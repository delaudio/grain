use grain::preview::iterm2::InlineImage;
use grain::runtime::RasterFrame;
use grain::{action::Action, app::App, history::HistoryManager};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

struct Workspace(std::path::PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn next_packet(app: &mut App) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.service_preview();
        if let Some(packet) = app.take_image_packet() {
            return packet;
        }
        assert!(Instant::now() < deadline, "image did not complete");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn pausing_restores_full_quality_without_advancing_or_repeatedly_rendering() {
    let workspace = Workspace(
        std::env::temp_dir().join(format!("grain-paused-quality-{}", std::process::id())),
    );
    let mut app = App::with_history_manager(HistoryManager::new(workspace.0.clone()));
    app.update(Action::Resize(80, 24));
    assert!(app.replace_edited_source("function draw(p) { p.background(12, 20, 28); }".into()));
    let screen = Rect::new(0, 0, 80, 24);
    let area = grain::ui::preview_content_rect(screen);
    app.set_image_target(Some((area, screen)));
    app.state.preview.is_playing = true;
    app.set_image_pacing(10_000, Duration::from_secs(1));
    let low_quality = next_packet(&mut app);
    let position = app.state.preview.current_frame;
    let source = app.state.preview.sketch_source.clone();
    app.update(Action::TogglePlayback);
    app.set_image_pacing(10_000, Duration::from_secs(1));
    let restored = next_packet(&mut app);
    let canvas = RasterFrame {
        width: 800,
        height: 600,
        rgba: [12, 20, 28, 255].repeat(800 * 600),
    };
    let expected = InlineImage::from_canvas(&canvas)
        .unwrap()
        .packet(area, screen)
        .unwrap();
    assert_eq!(restored, expected);
    assert_ne!(restored, low_quality);
    assert_eq!(app.state.preview.current_frame, position);
    assert_eq!(app.state.preview.sketch_source, source);
    for _ in 0..5 {
        app.set_image_pacing(10_000, Duration::from_secs(1));
        app.service_preview();
        assert!(app.take_image_packet().is_none());
    }
}
