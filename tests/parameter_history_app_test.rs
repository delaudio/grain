use grain::action::Action;
use grain::app::App;
use grain::history::HistoryManager;
use grain::state::InputMode;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "grain-parameter-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn app(&self) -> App {
        App::with_history_manager(HistoryManager::new(self.0.clone()))
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn edit(app: &mut App, command: &str) {
    app.update(Action::EnterParameters);
    for ch in command.chars() {
        app.update(Action::ParameterInputChar(ch));
    }
    app.update(Action::CommitParameter);
}

#[test]
fn parameters_survive_restart_and_rollback_without_changing_source_or_seed() {
    let workspace = Workspace::new();
    let mut app = workspace.app();
    let source = app.state.preview.sketch_source.clone();
    let seed = app.state.preview.seed;
    edit(&mut app, "speed=1.5");
    assert_eq!(app.state.mode, InputMode::Normal);
    let first = app.state.prompt.current_version;
    assert!(first > 0);
    let first_params = app.state.preview.params.clone();
    edit(&mut app, "speed=2.5");
    assert!(app.state.prompt.current_version > first);
    let second_params = app.state.preview.params.clone();
    assert_ne!(first_params, second_params);
    drop(app);

    let mut app = workspace.app();
    assert_eq!(app.state.preview.params, second_params);
    assert_eq!(app.state.preview.sketch_source, source);
    assert_eq!(app.state.preview.seed, seed);
    app.update(Action::RollbackToVersion(first));
    assert_eq!(app.state.preview.params, first_params);
    assert_eq!(app.state.preview.sketch_source, source);
    assert_eq!(app.state.preview.seed, seed);
}

#[test]
fn invalid_parameter_edits_preserve_the_active_version_and_removal_is_persisted() {
    let workspace = Workspace::new();
    let mut app = workspace.app();
    edit(&mut app, "speed=1");
    let version = app.state.prompt.current_version;
    let params = app.state.preview.params.clone();
    for command in [
        "speed=NaN",
        "speed=1000001",
        "__proto__=1",
        "not-an-assignment",
    ] {
        edit(&mut app, command);
        assert_eq!(app.state.mode, InputMode::EditingParameter);
        assert_eq!(app.state.preview.params, params);
        assert_eq!(app.state.prompt.current_version, version);
    }
    edit(&mut app, "-speed");
    assert_eq!(app.state.mode, InputMode::Normal);
    assert_eq!(app.state.preview.params.iter().count(), 0);
    assert!(app.state.prompt.current_version > version);
    drop(app);
    assert_eq!(workspace.app().state.preview.params.iter().count(), 0);
}

#[test]
fn parameter_panel_renders_at_normal_and_small_terminal_sizes() {
    use ratatui::{Terminal, backend::TestBackend};
    let workspace = Workspace::new();
    let mut app = workspace.app();
    app.update(Action::EnterParameters);
    for (width, height) in [(80, 24), (20, 8)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| grain::ui::render(frame, &app.state))
            .unwrap();
    }
}
