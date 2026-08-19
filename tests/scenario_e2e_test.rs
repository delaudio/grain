use std::fs;
use std::path::PathBuf;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use grain::app::App;
use grain::history::HistoryManager;
use grain::state::InputMode;
use grain::ui;

fn send_key(app: &mut App, code: KeyCode) {
    let event = KeyEvent::new(code, KeyModifiers::NONE);
    let mut next = app.handle_key_event(event).and_then(|act| app.update(act));
    while let Some(act) = next {
        next = app.update(act);
    }
}

fn type_str(app: &mut App, text: &str) {
    for ch in text.chars() {
        send_key(app, KeyCode::Char(ch));
    }
}

fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut output = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            output.push_str(cell.symbol());
        }
        output.push('\n');
    }
    output
}

#[test]
fn test_mvp_e2e_scenario() {
    let test_dir = PathBuf::from("target/test_grain_e2e_scenario");
    if test_dir.exists() {
        let _ = fs::remove_dir_all(&test_dir);
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let history_manager = HistoryManager::new(test_dir.clone());
    let mut app = App::with_history_manager(history_manager);
    if let Some(idx) = app.state.engine.options.iter().position(|o| matches!(o.kind, grain::generator::EngineKind::OfflineMock)) {
        app.state.engine.active_index = idx;
        app.state.engine.selected_index = idx;
    }

    // 1. Load Audio Fixture
    let fixture_path = PathBuf::from("fixtures/demo.wav");
    app.load_audio(fixture_path);

    terminal.draw(|f| ui::render(f, &app.state)).unwrap();
    let screen = buffer_to_string(&terminal);
    assert!(screen.contains("demo.wav"), "Expected audio track in UI: {}", screen);
    assert!(screen.contains("READY") || screen.contains("Ready"), "Expected Ready status: {}", screen);

    // 2. Enter Prompt Editing Mode
    send_key(&mut app, KeyCode::Char('p'));
    assert_eq!(app.state.mode, InputMode::EditingPrompt);
    terminal.draw(|f| ui::render(f, &app.state)).unwrap();
    let screen = buffer_to_string(&terminal);
    assert!(screen.contains("EDITING PROMPT"), "Expected editing mode title");

    // Clear and enter new prompt
    app.state.prompt.input_buffer.clear();
    app.state.prompt.cursor_position = 0;
    type_str(&mut app, "geometric audio tunnel");
    assert_eq!(app.state.prompt.input_buffer, "geometric audio tunnel");

    // 3. Commit Generation
    send_key(&mut app, KeyCode::Enter);
    assert_eq!(app.state.mode, InputMode::Normal);
    assert_eq!(app.state.prompt.current_version, 1);
    assert_eq!(app.state.preview.sketch_name, "sketch_v1");

    terminal.draw(|f| ui::render(f, &app.state)).unwrap();
    let screen = buffer_to_string(&terminal);
    assert!(screen.contains("sketch_v1"), "Expected sketch_v1 on screen");
    assert!(screen.contains("Visual Preview"), "Expected Visual Preview block");

    // 4. Revise Sketch
    send_key(&mut app, KeyCode::Char('p'));
    app.state.prompt.input_buffer.clear();
    app.state.prompt.cursor_position = 0;
    type_str(&mut app, "make it red with high speed");
    send_key(&mut app, KeyCode::Enter);

    assert_eq!(app.state.prompt.current_version, 2);
    assert_eq!(app.state.preview.sketch_name, "sketch_v2");

    terminal.draw(|f| ui::render(f, &app.state)).unwrap();
    let screen = buffer_to_string(&terminal);
    assert!(screen.contains("sketch_v2"), "Expected sketch_v2 on screen");

    // 5. Open Version History & Check Entries
    send_key(&mut app, KeyCode::Char('v'));
    assert_eq!(app.state.mode, InputMode::Versions);

    terminal.draw(|f| ui::render(f, &app.state)).unwrap();
    let screen = buffer_to_string(&terminal);
    assert!(screen.contains("Sketch Version History"), "Expected versions modal");
    assert!(screen.contains("001 • sketch_v1"), "Expected v1 listed in modal");
    assert!(screen.contains("002 • sketch_v2"), "Expected v2 listed in modal");

    // Close modal
    send_key(&mut app, KeyCode::Esc);
    assert_eq!(app.state.mode, InputMode::Normal);

    // 6. Clean Exit
    send_key(&mut app, KeyCode::Char('q'));
    assert!(app.state.should_quit, "Expected app to quit on 'q'");

    let _ = fs::remove_dir_all(&test_dir);
}
