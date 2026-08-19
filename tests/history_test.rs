use std::fs;
use std::path::PathBuf;
use grain::action::Action;
use grain::app::App;
use grain::history::HistoryManager;

#[test]
fn test_history_manager_records_and_loads_versions() {
    let test_dir = PathBuf::from("target/test_grain_history");
    if test_dir.exists() {
        let _ = fs::remove_dir_all(&test_dir);
    }

    let manager = HistoryManager::new(test_dir.clone());
    let v1 = manager
        .record_new_version(
            "particle wave",
            "function setup(p) {} function draw(p, ctx) { p.circle(0, 0, 10); }",
            42,
            "mock",
            None,
        )
        .expect("v1 record failed");
    assert_eq!(v1.version, 1);
    assert_eq!(v1.sketch_file, "001.js");

    let v2 = manager
        .record_new_version(
            "particle wave red",
            "function setup(p) {} function draw(p, ctx) { p.circle(0, 0, 20); }",
            43,
            "mock",
            None,
        )
        .expect("v2 record failed");
    assert_eq!(v2.version, 2);
    assert_eq!(v2.sketch_file, "002.js");

    let loaded = manager.load_history().expect("load failed");
    assert_eq!(loaded.versions.len(), 2);
    assert_eq!(loaded.active_version, 2);

    let v1_content = manager.load_sketch_content("001.js").expect("load v1 failed");
    assert!(v1_content.contains("p.circle(0, 0, 10)"));

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_app_rollback_action() {
    let test_dir = PathBuf::from("target/test_grain_app_rollback");
    if test_dir.exists() {
        let _ = fs::remove_dir_all(&test_dir);
    }

    let mut app = App::with_history_manager(HistoryManager::new(test_dir.clone()));

    // Trigger generation v1
    app.state.prompt.active_prompt = "first sketch".to_string();
    let mut next = app.update(Action::TriggerGenerate);
    while let Some(act) = next {
        next = app.update(act);
    }
    let v1_code = app.state.preview.sketch_source.clone();

    // Trigger generation v2
    app.state.prompt.active_prompt = "second sketch".to_string();
    let mut next2 = app.update(Action::TriggerGenerate);
    while let Some(act) = next2 {
        next2 = app.update(act);
    }
    let v2_code = app.state.preview.sketch_source.clone();
    assert_ne!(v1_code, v2_code);
    assert_eq!(app.state.prompt.current_version, 2);

    // Rollback to v1
    app.update(Action::RollbackToVersion(1));
    assert_eq!(app.state.prompt.current_version, 1);
    assert_eq!(app.state.preview.sketch_source, v1_code);
    assert_eq!(app.state.prompt.active_prompt, "first sketch");

    let _ = fs::remove_dir_all(&test_dir);
}
