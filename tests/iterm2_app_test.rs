use grain::{action::Action, app::App, history::HistoryManager};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

struct Workspace(std::path::PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn image(app: &mut App) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.service_preview();
        if let Some(packet) = app.take_image_packet() {
            return packet;
        }
        assert!(Instant::now() < deadline, "image preview did not complete");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn paused_backend_transitions_invalidate_images_and_restore_cell_preview() {
    let workspace =
        Workspace(std::env::temp_dir().join(format!("grain-iterm2-app-{}", std::process::id())));
    let mut app = App::with_history_manager(HistoryManager::new(workspace.0.clone()));
    app.state.preview.is_playing = false;
    app.update(Action::Resize(80, 24));
    let screen = Rect::new(0, 0, 80, 24);
    let target = Some((grain::ui::preview_content_rect(screen), screen));
    app.set_image_target(target);
    assert!(image(&mut app).starts_with(b"\x1b7"));
    assert!(app.image_active());
    let position = app.state.preview.current_frame;

    app.set_image_target(None);
    assert!(!app.image_active());
    assert!(app.take_image_packet().is_none());
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.state.preview.active_frame_result.is_none() {
        app.service_preview();
        assert!(Instant::now() < deadline, "cell fallback did not complete");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        app.state
            .preview
            .active_frame_result
            .as_ref()
            .unwrap()
            .cells
            .is_some()
    );
    app.set_image_target(target);
    assert!(!image(&mut app).is_empty());
    assert_eq!(app.state.preview.current_frame, position);
    assert!(app.image_active());

    let smaller = Rect::new(0, 0, 60, 20);
    app.update(Action::Resize(60, 20));
    app.set_image_target(Some((grain::ui::preview_content_rect(smaller), smaller)));
    assert!(!app.image_active());
    assert!(app.take_image_packet().is_none());
    assert!(!image(&mut app).is_empty());
}
