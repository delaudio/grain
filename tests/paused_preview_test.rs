use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use grain::app::App;
use grain::audio::AudioFeatures;
use grain::history::HistoryManager;
use grain::runtime::GrainContext;
use grain::runtime::session::SketchSession;

const SOURCE: &str =
    "let draws = 0; function draw(p, ctx) { p.background(ctx.audio.low * 255, ++draws, 0); }";

#[test]
fn same_position_audio_refreshes_but_resize_alone_preserves_state() {
    let mut context = GrainContext {
        params: Default::default(),
        width: 16,
        height: 16,
        frame: 0,
        time: 0.0,
        seed: 42,
        audio: AudioFeatures::default(),
    };
    let mut session = SketchSession::new(SOURCE, &context).unwrap();
    let first = session.render(&context, 8, 4).unwrap().raster.unwrap();
    assert_eq!(&first.rgba[..4], &[0, 1, 0, 255]);
    context.audio.low = 1.0;
    let refreshed = session.render(&context, 8, 4).unwrap().raster.unwrap();
    assert_eq!(&refreshed.rgba[..4], &[255, 2, 0, 255]);
    assert_eq!(
        session.render(&context, 4, 2).unwrap().raster.unwrap(),
        refreshed
    );
}

fn await_color(app: &mut App, red: u8, green: u8) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.service_preview();
        if app
            .state
            .preview
            .active_frame_result
            .as_ref()
            .and_then(|frame| frame.raster.as_ref())
            .is_some_and(|raster| raster.rgba[..4] == [red, green, 0, 255])
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "paused preview did not refresh: {:?}",
            app.state.preview.runtime_error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn coordinator_submits_changed_audio_while_playback_remains_paused() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "grain-paused-preview-{}-{nonce}",
        std::process::id()
    ));
    let mut app = App::with_history_manager(HistoryManager::new(path));
    app.state.preview.sketch_source = SOURCE.into();
    app.state.preview.width = 16;
    app.state.preview.height = 16;
    assert!(!app.state.preview.is_playing);
    await_color(&mut app, 0, 1);
    let revision = app
        .state
        .preview
        .active_frame_result
        .as_ref()
        .unwrap()
        .revision;
    app.state.live_audio_features.low = 1.0;
    await_color(&mut app, 255, 2);
    assert_eq!(app.state.preview.current_frame, 0);
    assert_eq!(
        app.state
            .preview
            .active_frame_result
            .as_ref()
            .unwrap()
            .revision,
        revision
    );
    assert!(!app.state.preview.is_playing);
}
