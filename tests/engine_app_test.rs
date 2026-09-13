use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use grain::action::Action;
use grain::app::App;
use grain::generator::EngineKind;
use grain::history::HistoryManager;
use grain::runtime::engine::EngineId;
use grain::state::{GrainState, InputMode};

struct TempHistory(std::path::PathBuf);
impl TempHistory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "grain-engine-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn app(&self) -> App {
        let mut app = App::with_history_manager(HistoryManager::new(self.0.clone()));
        let mock = app
            .state
            .engine
            .options
            .iter()
            .position(|option| option.kind == EngineKind::OfflineMock)
            .unwrap();
        app.state.engine.active_index = mock;
        app.state.engine.selected_index = mock;
        app
    }
}
impl Drop for TempHistory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn switch(app: &mut App, engine: EngineId) {
    app.update(Action::ToggleSketchEngine);
    app.update(Action::SelectSketchEngine(engine));
    app.update(Action::ActivateSketchEngine);
    assert_eq!(
        app.state.preview.engine, engine,
        "{:?}",
        app.state.status_message
    );
}

#[test]
fn sketch_selector_is_independent_and_native_preview_reaches_the_app() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    let provider = app.state.engine.active_index;
    assert_eq!(
        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        Some(Action::ToggleSketchEngine)
    );
    assert_eq!(
        app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)),
        Some(Action::ToggleSelectModel)
    );
    switch(&mut app, EngineId::Ascii);
    assert_eq!(app.state.engine.active_index, provider);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.service_preview();
        if let Some(frame) = &app.state.preview.active_frame_result {
            assert!(
                frame.raster.is_none(),
                "native output took a pixel round-trip"
            );
            assert!(frame.cells.as_ref().is_some_and(|rows| !rows.is_empty()));
            break;
        }
        assert!(Instant::now() < deadline, "{:?}", app.state.status_message);
        thread::sleep(Duration::from_millis(2));
    }
    app.update(Action::OpenInBrowser);
    assert!(
        app.state
            .status_message
            .as_deref()
            .unwrap()
            .contains("terminal")
    );
}

#[test]
fn switches_generation_rollback_and_restart_preserve_engine_source_and_seed() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    let p5_source = app.state.preview.sketch_source.clone();
    let p5_seed = app.state.preview.seed;
    switch(&mut app, EngineId::Ascii);
    app.state.prompt.active_prompt = "typographic interference".into();
    let completion = app.update(Action::TriggerGenerate).unwrap();
    app.update(completion);
    let ascii_source = app.state.preview.sketch_source.clone();
    let ascii_seed = app.state.preview.seed;
    let ascii_version = app.state.prompt.current_version;
    assert_eq!(ascii_seed, p5_seed + 1);
    assert_eq!(
        app.state.preview.sketch_name,
        format!("sketch_v{ascii_version}")
    );
    let recorded = app.history_manager.load_history().unwrap();
    assert_eq!(recorded.versions.last().unwrap().engine, EngineId::Ascii);

    switch(&mut app, EngineId::P5);
    assert_eq!(app.state.preview.sketch_source, p5_source);
    assert_eq!(app.state.preview.seed, p5_seed);
    app.update(Action::RollbackToVersion(ascii_version));
    assert_eq!(app.state.preview.engine, EngineId::Ascii);
    assert_eq!(app.state.selected_sketch_engine, EngineId::Ascii);
    assert_eq!(app.state.preview.sketch_source, ascii_source);
    assert_eq!(app.state.preview.seed, ascii_seed);
    drop(app);
    let restored = temp.app();
    assert_eq!(restored.state.preview.engine, EngineId::Ascii);
    assert_eq!(restored.state.preview.sketch_source, ascii_source);
    assert_eq!(restored.state.preview.seed, ascii_seed);
}

#[test]
fn invalid_edits_failed_generation_and_inactive_engine_results_preserve_source() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    switch(&mut app, EngineId::Ascii);
    let source = app.state.preview.sketch_source.clone();
    let seed = app.state.preview.seed;
    let version = app.state.prompt.current_version;
    assert!(!app.replace_edited_source("export function main() { return '\\x1b'; }".into()));
    app.update(Action::GenerationCompleted {
        result: Err("provider failed".into()),
        prompt: "failed".into(),
        engine: EngineId::Ascii,
        seed: seed + 1,
    });
    app.update(Action::GenerationCompleted {
        result: Ok("wrong engine".into()),
        prompt: "stale".into(),
        engine: EngineId::P5,
        seed: seed + 1,
    });
    assert_eq!(app.state.preview.sketch_source, source);
    assert_eq!(app.state.preview.seed, seed);
    assert_eq!(app.state.prompt.current_version, version);
    assert_eq!(app.state.preview.engine, EngineId::Ascii);
}

#[test]
fn corrupt_history_prevents_switch_without_replacing_unversioned_source() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    let original = app.state.preview.sketch_source.clone();
    fs::write(temp.0.join("generations.json"), "{broken").unwrap();
    app.update(Action::SelectSketchEngine(EngineId::Ascii));
    app.update(Action::ActivateSketchEngine);
    assert_eq!(app.state.preview.engine, EngineId::P5);
    assert_eq!(app.state.preview.sketch_source, original);
    assert_eq!(
        fs::read_to_string(temp.0.join("generations.json")).unwrap(),
        "{broken"
    );
}

#[test]
fn selector_renders_with_existing_style_at_normal_and_small_sizes() {
    for (width, height) in [(80, 24), (20, 8)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        let state = GrainState {
            mode: InputMode::SelectSketchEngine,
            ..Default::default()
        };
        terminal
            .draw(|frame| grain::ui::render(frame, &state))
            .unwrap();
        if width == 80 {
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Sketch Engine"));
            assert!(text.contains("ASCII"));
            assert!(text.contains("p5"));
        }
    }
}

#[test]
fn cross_engine_rollbacks_refresh_the_draft_restored_by_selection() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    let first = "function setup(p) {} function draw(p) { p.background(10); }";
    let second = "function setup(p) {} function draw(p) { p.background(20); }";
    app.update(Action::GenerationCompleted {
        result: Ok(first.into()),
        prompt: "first".into(),
        engine: EngineId::P5,
        seed: 101,
    });
    let first_version = app.state.prompt.current_version;
    app.update(Action::GenerationCompleted {
        result: Ok(second.into()),
        prompt: "second".into(),
        engine: EngineId::P5,
        seed: 102,
    });
    switch(&mut app, EngineId::Ascii);
    let ascii_version = app.state.prompt.current_version;
    app.update(Action::RollbackToVersion(first_version));
    assert_eq!(app.state.preview.sketch_source, first);
    app.update(Action::RollbackToVersion(ascii_version));
    assert_eq!(app.state.preview.engine, EngineId::Ascii);
    switch(&mut app, EngineId::P5);
    assert_eq!(app.state.preview.sketch_source, first);
    assert_eq!(app.state.preview.seed, 101);
    assert_eq!(app.state.prompt.active_prompt, "first");
}

#[test]
fn restart_rejects_invalid_disk_edits_without_overwriting_the_repairable_file() {
    use grain::generator::GenerationService;
    for engine in [EngineId::P5, EngineId::Ascii] {
        let temp = TempHistory::new();
        let mut app = temp.app();
        if engine == EngineId::Ascii {
            switch(&mut app, engine);
        } else {
            let completion = app.update(Action::TriggerGenerate).unwrap();
            app.update(completion);
        }
        let seed = app.state.preview.seed;
        let original = app.state.preview.sketch_source.clone();
        let path = app
            .history_manager
            .get_active_sketch_path()
            .unwrap()
            .unwrap();
        let history_path = temp.0.join("generations.json");
        let history_before = fs::read(&history_path).unwrap();
        let invalid = "function broken( {";
        fs::write(&path, invalid).unwrap();
        assert!(!app.replace_edited_source(invalid.into()));
        assert_eq!(app.state.preview.sketch_source, original);
        drop(app);

        let mut restored = temp.app();
        assert_eq!(restored.state.preview.engine, engine);
        assert_eq!(restored.state.preview.seed, seed);
        assert_eq!(restored.state.prompt.current_version, 0);
        assert!(restored.state.preview.sketch_name.starts_with("recovery_"));
        assert!(
            restored
                .state
                .status_message
                .as_deref()
                .unwrap()
                .contains("Cannot restore sketch")
        );
        GenerationService::validate_for_engine(engine, &restored.state.preview.sketch_source, seed)
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        assert_eq!(fs::read(&history_path).unwrap(), history_before);

        fs::write(&path, &original).unwrap();
        assert!(restored.replace_edited_source(original.clone()));
        assert_eq!(restored.state.preview.sketch_source, original);
        drop(restored);
        assert_eq!(temp.app().state.preview.sketch_source, original);
    }
}

#[test]
fn accepted_edits_survive_cross_engine_rollback_and_reselection() {
    let temp = TempHistory::new();
    let mut app = temp.app();
    let completion = app.update(Action::TriggerGenerate).unwrap();
    app.update(completion);
    let p5_version = app.state.prompt.current_version;
    let seed = app.state.preview.seed;
    switch(&mut app, EngineId::Ascii);
    let ascii_version = app.state.prompt.current_version;
    app.update(Action::RollbackToVersion(p5_version));
    let edited = "function setup(p) {} function draw(p) { p.background(99); }";
    let path = app
        .history_manager
        .get_active_sketch_path()
        .unwrap()
        .unwrap();
    fs::write(&path, edited).unwrap();
    assert!(app.replace_edited_source(edited.into()));
    app.update(Action::RollbackToVersion(ascii_version));
    switch(&mut app, EngineId::P5);
    assert_eq!(app.state.preview.sketch_source, edited);
    assert_eq!(app.state.preview.seed, seed);
    let active_path = app
        .history_manager
        .get_active_sketch_path()
        .unwrap()
        .unwrap();
    assert_eq!(fs::read_to_string(active_path).unwrap(), edited);
}
