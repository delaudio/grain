use std::sync::Arc;
use ratatui::layout::Rect;
use grain::audio::AudioFeatures;
use grain::preview::{AnsiPreviewBackend, PreviewEngine, RattyTerminalBackend};

#[test]
fn test_preview_engine_renders_ansi_backend() {
    let engine = PreviewEngine::with_backend(Arc::new(AnsiPreviewBackend::new()));
    let source = include_str!("../examples/wave_particles.js");
    let audio = AudioFeatures {
        amplitude: 0.7,
        low: 0.9,
        mid: 0.4,
        high: 0.2,
    };
    let area = Rect::new(0, 0, 60, 20);

    let (res, lines) = engine
        .render_frame(source, 10, 60, 42, audio, area)
        .expect("Rendering should succeed");

    assert_eq!(res.frame, 10);
    assert!(!lines.is_empty());
    assert!(res.cells.is_some());
    assert!(!res.cells.unwrap().is_empty());
}

#[test]
fn test_preview_engine_renders_ratty_backend() {
    let engine = PreviewEngine::with_backend(Arc::new(RattyTerminalBackend::new()));
    let source = include_str!("../examples/wave_particles.js");
    let audio = AudioFeatures {
        amplitude: 0.5,
        low: 0.5,
        mid: 0.5,
        high: 0.5,
    };
    let area = Rect::new(0, 0, 80, 24);

    let (res, lines) = engine
        .render_frame(source, 0, 60, 100, audio, area)
        .expect("Rendering should succeed");

    assert_eq!(res.frame, 0);
    assert!(!lines.is_empty());
}

#[test]
fn test_preview_engine_handles_small_area() {
    let engine = PreviewEngine::new();
    let source = include_str!("../fixtures/example_sketch.js");
    let audio = AudioFeatures::default();
    let area = Rect::new(0, 0, 15, 6);

    let (res, lines) = engine
        .render_frame(source, 0, 60, 42, audio, area)
        .expect("Rendering should succeed on small area");

    assert_eq!(res.frame, 0);
    assert!(!lines.is_empty());
}
