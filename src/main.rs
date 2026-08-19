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

fn main() -> Result<()> {
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
                            let mut next_action = app.update(action);
                            while let Some(chained) = next_action {
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
        } else {
            app.update(Action::Tick);
        }
    }

    Ok(())
}
