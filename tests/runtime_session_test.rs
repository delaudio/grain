use std::sync::Arc;
use std::time::{Duration, Instant};

use grain::audio::AudioFeatures;
use grain::preview::clock::PlaybackClock;
use grain::preview::worker::{PreviewWorker, RenderCompletion, RenderRequest};
use grain::runtime::session::SketchSession;
use grain::runtime::{GrainContext, evaluate_frame};

fn context(frame: usize) -> GrainContext {
    GrainContext {
        params: Default::default(),
        width: 16,
        height: 16,
        frame,
        time: frame as f64 / 60.0,
        seed: 42,
        audio: AudioFeatures::default(),
    }
}

fn request(source: &str, frame: usize, revision: u64, lifecycle: u64) -> RenderRequest {
    RenderRequest {
        source: Arc::from(source),
        context: context(frame),
        revision,
        lifecycle,
        cols: 8,
        rows: 4,
    }
}

fn completed(worker: &PreviewWorker, revision: u64) -> RenderCompletion {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = worker.take_completed().filter(|r| r.revision == revision) {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "worker did not finish revision {revision}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn setup_context_and_synchronous_contract_match_both_evaluators() {
    let source = "function setup(p, ctx) { p.background(ctx.seed, 0, 0); } function draw() {}";
    let expected = evaluate_frame(source, &context(0), 8, 4).unwrap();
    let actual = SketchSession::new(source, &context(0))
        .unwrap()
        .render(&context(0), 8, 4)
        .unwrap();
    assert_eq!(actual.raster, expected.raster);
    assert_eq!(&actual.raster.unwrap().rgba[..4], &[42, 0, 0, 255]);
    for source in [
        "async function setup() { throw new Error('failed setup'); } function draw() {}",
        "async function setup(p) { await 0; p.background(255); } function draw() {}",
    ] {
        assert!(evaluate_frame(source, &context(0), 8, 4).is_err());
        assert!(SketchSession::new(source, &context(0)).is_err());
    }
}

#[test]
fn state_and_canvas_persist_and_paused_resize_does_not_draw_again() {
    let source = "let count = 0; function setup(p) { p.background(0); } function draw(p) { p.noStroke(); p.fill(++count, 0, 0); p.rect(count - 1, 0, 1, 16); }";
    let mut session = SketchSession::new(source, &context(0)).unwrap();
    let first = session.render(&context(0), 8, 4).unwrap();
    let resized = session.render(&context(0), 4, 2).unwrap();
    assert_eq!(first.raster, resized.raster);
    assert_eq!(resized.cells.unwrap().len(), 2);
    let next = session.render(&context(5), 8, 4).unwrap().raster.unwrap();
    assert_eq!(&next.rgba[..4], &[1, 0, 0, 255]);
    assert_eq!(&next.rgba[4..8], &[2, 0, 0, 255]);
}

#[test]
fn viewport_failure_does_not_poison_a_valid_session() {
    let worker = PreviewWorker::new().unwrap();
    let source = "let n = 0; function draw(p) { p.background(++n, 0, 0); }";
    worker.submit(request(source, 0, 1, 1));
    completed(&worker, 1).result.unwrap();
    let mut oversized = request(source, 1, 2, 1);
    oversized.cols = 400;
    oversized.rows = 100;
    worker.submit(oversized);
    assert!(completed(&worker, 2).result.is_err());
    worker.submit(request(source, 1, 3, 1));
    let next = completed(&worker, 3).result.unwrap().raster.unwrap();
    assert_eq!(&next.rgba[..4], &[2, 0, 0, 255]);
}

#[test]
fn lifecycle_epoch_resets_even_when_the_loop_boundary_request_was_skipped() {
    let worker = PreviewWorker::new().unwrap();
    let source = "let n = 0; function draw(p) { p.background(++n, 0, 0); }";
    worker.submit(request(source, 3, 1, 1));
    completed(&worker, 1).result.unwrap();
    worker.submit(request(source, 5, 1, 1));
    assert_eq!(
        completed(&worker, 1).result.unwrap().raster.unwrap().rgba[0],
        2
    );
    // Frame zero of the next loop never reaches the worker. Its epoch does.
    worker.submit(request(source, 10, 2, 2));
    let restarted = completed(&worker, 2).result.unwrap();
    assert_eq!(restarted.revision, 2);
    assert_eq!(restarted.raster.unwrap().rgba[0], 1);
}

#[test]
fn a_failed_replacement_retains_the_previous_session() {
    let worker = PreviewWorker::new().unwrap();
    let source = "let n = 0; function draw(p) { p.background(++n, 0, 0); }";
    worker.submit(request(source, 0, 1, 1));
    completed(&worker, 1).result.unwrap();
    worker.submit(request(
        "function draw() { throw new Error('bad edit'); }",
        1,
        2,
        1,
    ));
    assert!(completed(&worker, 2).result.is_err());
    worker.submit(request(source, 2, 3, 1));
    assert_eq!(
        completed(&worker, 3).result.unwrap().raster.unwrap().rgba[0],
        2
    );
}

#[test]
fn interrupted_session_is_poisoned_and_a_fresh_session_recovers() {
    let source = "function draw(p, ctx) { if(ctx.frame) while(true) {} p.background(255); }";
    let mut session = SketchSession::new(source, &context(0)).unwrap();
    session.render(&context(0), 8, 4).unwrap();
    assert!(
        session
            .render(&context(1), 8, 4)
            .unwrap_err()
            .message
            .contains("time limit")
    );
    assert!(
        session
            .render(&context(2), 8, 4)
            .unwrap_err()
            .message
            .contains("reset")
    );
    assert!(
        SketchSession::new(source, &context(0))
            .unwrap()
            .render(&context(0), 8, 4)
            .is_ok()
    );
}

#[test]
fn fake_clock_tracks_time_not_input_count_and_honors_audio_pause_and_loop() {
    let now = Instant::now();
    let mut clock = PlaybackClock::new(now);
    clock.update(now, true, None);
    for _ in 0..1000 {
        assert_eq!(clock.update(now, true, None), Duration::ZERO);
    }
    let second = now + Duration::from_secs(1);
    assert_eq!(
        PlaybackClock::frame(clock.update(second, false, None), 60, 100),
        60
    );
    assert_eq!(
        clock.update(second + Duration::from_secs(10), true, None),
        Duration::from_secs(1)
    );
    assert_eq!(
        PlaybackClock::frame(
            clock.update(second + Duration::from_secs(11), true, None),
            60,
            100
        ),
        20
    );
    assert_eq!(
        clock.update(
            second + Duration::from_secs(12),
            true,
            Some(Duration::from_millis(125))
        ),
        Duration::from_millis(125)
    );
    assert_eq!(PlaybackClock::frame(Duration::from_millis(125), 60, 100), 7);
}
