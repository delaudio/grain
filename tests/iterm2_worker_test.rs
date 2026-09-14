use grain::audio::AudioFeatures;
use grain::preview::worker::{EngineCompletion, PreviewWorker, RenderRequest};
use grain::runtime::GrainContext;
use grain::runtime::engine::{EngineId, FrameOutput};
use ratatui::layout::Rect;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn request(source: &str, revision: u64) -> RenderRequest {
    RenderRequest {
        revision,
        lifecycle: 0,
        source: Arc::from(source),
        cols: 20,
        rows: 10,
        context: GrainContext {
            width: 800,
            height: 600,
            frame: 0,
            time: 0.0,
            seed: 42,
            params: Default::default(),
            audio: AudioFeatures {
                amplitude: 0.0,
                low: 0.0,
                mid: 0.0,
                high: 0.0,
            },
        },
    }
}

fn completed(worker: &PreviewWorker) -> EngineCompletion {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = worker.take_engine_completed() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker did not complete");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn worker_encodes_images_without_changing_engine_canvas() {
    let worker = PreviewWorker::new().unwrap();
    worker.submit_iterm2_for_engine(
        EngineId::P5,
        request("function draw(p) { p.background(255, 0, 0); }", 1),
        Rect::new(1, 2, 20, 10),
        Rect::new(0, 0, 80, 24),
    );
    let completion = completed(&worker);
    let packet = completion.image_packet.unwrap().unwrap();
    assert!(packet.starts_with(b"\x1b7\x1b[3;2H\x1b]1337;File="));
    assert!(packet.len() <= grain::preview::iterm2::MAX_PACKET_BYTES);
    let FrameOutput::Raster(raster) = completion.result.unwrap().output else {
        panic!("expected raster")
    };
    assert_eq!((raster.width, raster.height), (800, 600));
}

#[test]
fn native_cells_bypass_encoding_and_invalid_image_regions_have_a_cell_fallback() {
    let worker = PreviewWorker::new().unwrap();
    let screen = Rect::new(0, 0, 80, 24);
    worker.submit_iterm2_for_engine(
        EngineId::Ascii,
        request("function main() { return 'X'; }", 1),
        Rect::new(1, 2, 20, 10),
        screen,
    );
    let native = completed(&worker);
    assert!(native.image_packet.is_none());
    let native = native.into_terminal().result.unwrap();
    assert_eq!(native.cells.unwrap()[0][0].symbol, "X");
    assert!(native.raster.is_none());

    worker.submit_iterm2_for_engine(
        EngineId::P5,
        request("function draw(p) { p.background(0); }", 2),
        Rect::new(0, 23, 20, 1),
        screen,
    );
    let fallback = completed(&worker);
    assert!(fallback.image_packet.as_ref().unwrap().is_err());
    assert!(fallback.into_terminal().result.unwrap().cells.is_some());
}

#[test]
fn target_changes_discard_completed_image_packets() {
    let worker = PreviewWorker::new().unwrap();
    let source = "function draw(p) { p.background(0); }";
    worker.submit_iterm2_for_engine(
        EngineId::P5,
        request(source, 1),
        Rect::new(1, 2, 20, 10),
        Rect::new(0, 0, 80, 24),
    );
    worker.submit_for_engine(EngineId::P5, request(source, 2));
    let result = completed(&worker);
    assert_eq!(result.revision, 2);
    assert!(result.image_packet.is_none());
    assert!(result.into_terminal().result.is_ok());
}
