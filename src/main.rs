use grain::{action, app, cli, terminal, ui, web};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event};
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

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
    app.state.preview.fps = args.fps.clamp(1, 120);

    if let Some(audio_path) = args.audio_file {
        app.load_audio(audio_path);
    }

    let web_state = std::sync::Arc::new(std::sync::RwLock::new(web::WebBridgeState::default()));
    let web_server = web::WebServer::start(3333, std::sync::Arc::clone(&web_state));

    let tick_rate = Duration::from_secs_f64(1.0 / f64::from(app.state.preview.fps));

    let result = run_app(&mut terminal, &mut app, &web_server, tick_rate);

    restore_terminal()?;

    if let Err(err) = result {
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut terminal::Tui,
    app: &mut App,
    web_server: &web::WebServer,
    tick_rate: Duration,
) -> Result<()> {
    let size = terminal.size()?;
    app.update(Action::Resize(size.width, size.height));

    let image_backend = grain::preview::selection::iterm2_enabled(
        std::env::var("GRAIN_PREVIEW_BACKEND").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("TERM_PROGRAM_VERSION").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
        std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some(),
        std::io::stdout().is_terminal(),
    )
    .map_err(anyhow::Error::msg)?;
    let mut shown_image_area = None;

    let mut next_tick = Instant::now();
    while !app.state.should_quit {
        let now = Instant::now();
        if now >= next_tick {
            app.update(Action::Tick);
            // Skip missed deadlines instead of replaying obsolete UI ticks.
            next_tick = now + tick_rate;
        }
        let size = terminal.size()?;
        let screen = ratatui::layout::Rect::new(0, 0, size.width, size.height);
        let area = ui::preview_content_rect(screen);
        let image_target = (image_backend
            && app.state.mode == grain::state::InputMode::Normal
            && app.state.preview.engine == grain::runtime::engine::EngineId::P5
            && area.width > 0
            && area.height > 0
            && area.bottom() < screen.bottom())
        .then_some((area, screen));
        app.set_image_target(image_target);
        app.service_preview();
        let packet = app.take_image_packet();
        // Sync state with web browser live server
        web_server.update_state(
            app.state.prompt.current_version,
            &app.state.preview.sketch_source,
            app.state.audio.path.clone(),
            app.state.live_audio_features,
            app.state.preview.is_playing,
            app.state.preview.current_frame,
        );

        if shown_image_area.is_some()
            && (shown_image_area != image_target.map(|(area, _)| area) || !app.image_active())
        {
            terminal.clear()?;
            shown_image_area = None;
        }
        if packet.is_some()
            && let Some(old_area) = shown_image_area
        {
            let clear = grain::preview::iterm2::clear_packet(old_area, screen)
                .map_err(anyhow::Error::msg)?;
            terminal.backend_mut().write_all(&clear)?;
        }
        let mut drawn_screen = screen;
        terminal.draw(|f| {
            drawn_screen = f.area();
            ui::render(f, &app.state);
            if app.image_active() && image_target.is_some() && drawn_screen == screen {
                f.render_widget(
                    ratatui::widgets::Block::default()
                        .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black)),
                    area,
                );
            }
        })?;
        if drawn_screen != screen {
            app.set_image_target(None);
            terminal.clear()?;
            shown_image_area = None;
        } else if let Some(packet) = packet {
            terminal.backend_mut().write_all(&packet)?;
            terminal.backend_mut().flush()?;
            shown_image_area = Some(area);
        }

        if event::poll(next_tick.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process key press events (ignore release/repeat if supported by platform)
                    if key.kind == event::KeyEventKind::Press
                        && let Some(action) = app.handle_key_event(key)
                    {
                        if action == Action::OpenInEditor {
                            terminal.clear()?;
                            shown_image_area = None;
                            app.set_image_target(None);
                            handle_open_in_editor(terminal, app)?;
                        } else {
                            let mut next_action = app.update(action);
                            while let Some(chained) = next_action {
                                if chained == Action::OpenInEditor {
                                    terminal.clear()?;
                                    shown_image_area = None;
                                    app.set_image_target(None);
                                    handle_open_in_editor(terminal, app)?;
                                    break;
                                }
                                next_action = app.update(chained);
                            }
                        }
                    }
                }
                Event::Resize(w, h) => {
                    app.update(Action::Resize(w, h));
                }
                _ => {}
            }
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
        if let Ok(meta) = app.history_manager.record_version(
            &app.state.prompt.active_prompt,
            &app.state.preview.sketch_source,
            "Template",
            None,
            &grain::history::record::VersionInputs {
                engine: app.state.preview.engine,
                seed: app.state.preview.seed,
                params: app.state.preview.params.clone(),
            },
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
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        });

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
        let accepted = app.replace_edited_source(content);
        if let Ok(meta) = std::fs::metadata(&sketch_path) {
            app.last_watched_mtime = meta.modified().ok();
        }
        if accepted {
            app.state.status_message = Some(format!(
                "Updated sketch from editor: {}",
                sketch_path.display()
            ));
        }
    }

    Ok(())
}
