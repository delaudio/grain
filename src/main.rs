mod action;
mod app;
mod audio;
mod cli;
mod generator;
mod history;
mod preview;
mod runtime;
mod state;
mod terminal;
mod ui;

use std::time::Duration;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event};

use action::Action;
use app::App;
use cli::Cli;
use terminal::{init_terminal, install_panic_hook, restore_terminal};
fn load_dotenv_if_exists() {
    if let Ok(content) = std::fs::read_to_string(".env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim().trim_matches('"').trim_matches('\'');
                if std::env::var(k).is_err() {
                    unsafe {
                        std::env::set_var(k, v);
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    load_dotenv_if_exists();
    let args = Cli::parse();

    install_panic_hook();
    let mut terminal = init_terminal()?;

    let mut app = App::new();
    app.state.preview.fps = args.fps;

    if let Some(audio_path) = args.audio_file {
        app.load_audio(audio_path);
    }

    let tick_rate = Duration::from_millis(1000 / args.fps.max(1) as u64);

    let result = run_app(&mut terminal, &mut app, tick_rate);

    restore_terminal()?;

    if let Err(err) = result {
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut terminal::Tui, app: &mut App, tick_rate: Duration) -> Result<()> {
    let size = terminal.size()?;
    app.update(Action::Resize(size.width, size.height));

    while !app.state.should_quit {
        terminal.draw(|f| ui::render(f, &app.state))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process key press events (ignore release/repeat if supported by platform)
                    if key.kind == event::KeyEventKind::Press {
                        if let Some(action) = app.handle_key_event(key) {
                            if action == Action::OpenInEditor {
                                handle_open_in_editor(terminal, app)?;
                            } else {
                                let mut next_action = app.update(action);
                                while let Some(chained) = next_action {
                                    if chained == Action::OpenInEditor {
                                        handle_open_in_editor(terminal, app)?;
                                        break;
                                    }
                                    next_action = app.update(chained);
                                }
                            }
                        }
                    }
                }
                Event::Resize(w, h) => {
                    app.update(Action::Resize(w, h));
                }
                _ => {}
            }
        } else {
            app.update(Action::Tick);
        }
    }

    Ok(())
}

fn handle_open_in_editor(terminal: &mut terminal::Tui, app: &mut App) -> Result<()> {
    // Ensure active sketch exists as file on disk
    let sketch_path = if let Ok(Some(path)) = app.history_manager.get_active_sketch_path() {
        path
    } else {
        // Record initial version if none exists
        if let Ok(meta) = app.history_manager.record_new_version(
            &app.state.prompt.active_prompt,
            &app.state.preview.sketch_source,
            app.state.preview.seed,
            "Template",
            None,
        ) {
            app.history_manager.get_sketch_path(&meta.sketch_file)
        } else {
            return Ok(());
        }
    };

    // Restore terminal to normal console mode
    restore_terminal()?;

    let editor_env = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| if cfg!(windows) { "notepad".to_string() } else { "nano".to_string() });

    let parts: Vec<&str> = editor_env.split_whitespace().collect();
    let prog = parts.first().copied().unwrap_or("nano");
    let mut cmd = std::process::Command::new(prog);
    for arg in &parts[1..] {
        cmd.arg(arg);
    }
    cmd.arg(&sketch_path);

    let _ = cmd.status();

    // Re-initialize terminal TUI mode
    *terminal = init_terminal()?;
    let size = terminal.size()?;
    app.update(Action::Resize(size.width, size.height));

    // Reload modified code
    if let Ok(content) = std::fs::read_to_string(&sketch_path) {
        app.state.preview.sketch_source = content;
        if let Ok(meta) = std::fs::metadata(&sketch_path) {
            app.last_watched_mtime = meta.modified().ok();
        }
        app.state.status_message = Some(format!("Updated sketch from editor: {}", sketch_path.display()));
    }

    Ok(())
}
