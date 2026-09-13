use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use grain::audio::AudioFeatures;
use grain::preview::worker::{EngineCompletion, OutputTarget, PreviewWorker, RenderRequest};
use grain::runtime::engine::{
    CellFrame, EngineAdapter, EngineCapabilities, EngineFactory, EngineId, FrameOutput, OutputKind,
    ResetReason,
};
use grain::runtime::{GrainContext, RasterFrame, RuntimeDiagnostic, TerminalCell};

fn request(source: &str, revision: u64, frame: usize) -> RenderRequest {
    RenderRequest {
        revision,
        lifecycle: 0,
        source: Arc::from(source),
        cols: 2,
        rows: 1,
        context: GrainContext {
            width: 8,
            height: 4,
            frame,
            time: frame as f64 / 30.0,
            seed: 42,
            audio: AudioFeatures {
                amplitude: 0.5,
                low: 0.25,
                mid: 0.75,
                high: 1.0,
            },
        },
    }
}

fn completion(worker: &PreviewWorker) -> EngineCompletion {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = worker.take_engine_completed() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker did not finish");
        thread::sleep(Duration::from_millis(2));
    }
}

#[derive(Default)]
struct FakeFactory {
    resets: Arc<Mutex<Vec<(EngineId, ResetReason)>>>,
}

struct FakeAdapter {
    engine: EngineId,
    count: u8,
    invalid_output: bool,
}

impl EngineFactory for FakeFactory {
    fn create(
        &self,
        engine: EngineId,
        source: &str,
        _: &GrainContext,
        reason: ResetReason,
    ) -> Result<Box<dyn EngineAdapter>, RuntimeDiagnostic> {
        self.resets.lock().unwrap().push((engine, reason));
        Ok(Box::new(FakeAdapter {
            engine,
            count: 0,
            invalid_output: source == "invalid",
        }))
    }
}

impl EngineAdapter for FakeAdapter {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            output: if self.engine == EngineId::P5 {
                OutputKind::Raster
            } else {
                OutputKind::Cells
            },
            persistent_state: true,
        }
    }

    fn render(
        &mut self,
        context: &GrainContext,
        cols: u16,
        rows: u16,
    ) -> Result<FrameOutput, RuntimeDiagnostic> {
        self.count += 1;
        if self.engine == EngineId::P5 {
            return Ok(FrameOutput::Raster(RasterFrame {
                width: 1,
                height: 1,
                rgba: vec![self.count, 100, 200, 255],
            }));
        }
        let symbol = if self.invalid_output { "\x1b" } else { "#" };
        let cell = TerminalCell {
            symbol: symbol.into(),
            r: self.count,
            g: (context.audio.low * 100.0) as u8,
            b: 3,
            background: Some([4, 5, 6]),
        };
        Ok(FrameOutput::Cells(CellFrame {
            cols,
            rows,
            cells: vec![vec![cell; usize::from(cols)]; usize::from(rows)],
        }))
    }
}

#[test]
fn fake_dispatch_stamps_authoritative_headers_and_passes_native_cells_unchanged() {
    let worker = PreviewWorker::with_factory(Arc::new(FakeFactory::default())).unwrap();
    worker.submit_for_engine(EngineId::Ascii, request("cells", 17, 30));
    let done = completion(&worker);
    assert_eq!(done.target(), OutputTarget::Terminal);
    let frame = done.result.as_ref().unwrap();
    assert_eq!(frame.engine, EngineId::Ascii);
    assert_eq!(frame.contract_version, 1);
    assert_eq!(frame.revision, 17);
    assert_eq!(frame.frame, 30);
    assert_eq!(frame.time, 1.0);
    let FrameOutput::Cells(native) = &frame.output else {
        panic!("cells were rasterized")
    };
    let expected = native.cells.clone();
    assert_eq!(expected[0][0].g, 25);
    let terminal = done.into_terminal().result.unwrap();
    assert_eq!(terminal.cells.unwrap(), expected);
    assert!(terminal.raster.is_none());

    worker.submit_for_engine(EngineId::P5, request("raster", 18, 31));
    let raw = completion(&worker);
    let FrameOutput::Raster(raster) = &raw.result.as_ref().unwrap().output else {
        panic!("raster was converted eagerly")
    };
    assert_eq!(raster.rgba, [1, 100, 200, 255]);
    let terminal = raw.into_terminal().result.unwrap();
    assert!(terminal.raster.is_some());
    assert_eq!(terminal.cells.unwrap()[0].len(), 2);
}

#[test]
fn raw_target_never_falls_back_to_sampling_on_the_consuming_thread() {
    let worker = PreviewWorker::with_factory(Arc::new(FakeFactory::default())).unwrap();
    worker.submit_raw_for_engine(EngineId::P5, request("raster", 1, 0));
    let raw = completion(&worker);
    assert_eq!(raw.target(), OutputTarget::Raw);
    assert!(matches!(
        &raw.result.as_ref().unwrap().output,
        FrameOutput::Raster(_)
    ));
    assert!(
        raw.into_terminal()
            .result
            .unwrap_err()
            .message
            .contains("no terminal preparation")
    );

    worker.submit_for_engine(EngineId::P5, request("raster", 2, 0));
    let prepared = completion(&worker);
    assert_eq!(prepared.target(), OutputTarget::Terminal);
    let frame = prepared.into_terminal().result.unwrap();
    assert_eq!(frame.cells.unwrap()[0].len(), 2);
    assert!(frame.ascii_art.is_some());
}

#[test]
fn failed_engine_switch_retains_previous_session_and_seek_resets_state() {
    let factory = Arc::new(FakeFactory::default());
    let worker = PreviewWorker::with_factory(factory.clone()).unwrap();
    worker.submit(request("raster", 1, 10));
    assert!(completion(&worker).result.is_ok());
    worker.submit_for_engine(EngineId::Ascii, request("invalid", 2, 11));
    assert!(completion(&worker).result.is_err());
    worker.submit(request("raster", 3, 12));
    let FrameOutput::Raster(raster) = completion(&worker).result.unwrap().output else {
        panic!()
    };
    assert_eq!(
        raster.rgba[0], 2,
        "failed replacement discarded the previous state"
    );
    worker.submit(request("raster", 4, 0));
    let FrameOutput::Raster(raster) = completion(&worker).result.unwrap().output else {
        panic!()
    };
    assert_eq!(raster.rgba[0], 1);
    assert_eq!(
        *factory.resets.lock().unwrap(),
        [
            (EngineId::P5, ResetReason::SourceChanged),
            (EngineId::Ascii, ResetReason::EngineChanged),
            (EngineId::P5, ResetReason::Seek),
        ]
    );
}

#[test]
fn explicit_lifecycle_resets_but_terminal_resize_keeps_session() {
    let factory = Arc::new(FakeFactory::default());
    let worker = PreviewWorker::with_factory(factory.clone()).unwrap();
    worker.submit_for_engine(EngineId::Ascii, request("cells", 1, 0));
    assert!(completion(&worker).result.is_ok());
    let mut resized = request("cells", 2, 0);
    resized.cols = 3;
    worker.submit_for_engine(EngineId::Ascii, resized.clone());
    let FrameOutput::Cells(cells) = completion(&worker).result.unwrap().output else {
        panic!()
    };
    assert_eq!(cells.cells[0][0].r, 2);
    resized.lifecycle = 1;
    resized.revision = 3;
    worker.submit_for_engine(EngineId::Ascii, resized);
    let FrameOutput::Cells(cells) = completion(&worker).result.unwrap().output else {
        panic!()
    };
    assert_eq!(cells.cells[0][0].r, 1);
    assert_eq!(
        factory.resets.lock().unwrap().last().unwrap().1,
        ResetReason::Restart
    );
}

#[test]
fn builtins_share_worker_without_a_native_pixel_roundtrip() {
    let worker = PreviewWorker::new().unwrap();
    worker.submit_for_engine(
        EngineId::Ascii,
        request("export function main(c) { return c.x ? 'B' : 'A'; }", 1, 0),
    );
    let terminal = completion(&worker).into_terminal();
    assert_eq!(terminal.engine, EngineId::Ascii);
    let result = terminal.result.unwrap();
    assert_eq!(result.ascii_art.as_deref(), Some("AB"));
    assert!(result.raster.is_none());
    worker.submit(request(
        "function setup(p) { p.createCanvas(8, 4); } function draw(p) { p.background(255); }",
        2,
        0,
    ));
    let raw = completion(&worker);
    assert_eq!(raw.engine, EngineId::P5);
    assert!(raw.draw_commands_count > 0);
    assert!(matches!(raw.result.unwrap().output, FrameOutput::Raster(_)));
}

#[test]
fn in_flight_result_is_discarded_after_engine_switch_and_pending_slot_is_latest_only() {
    use std::sync::mpsc::{self, Receiver, SyncSender};
    struct GateFactory {
        started: SyncSender<()>,
        release: Mutex<Option<Receiver<()>>>,
    }
    struct GateAdapter {
        started: SyncSender<()>,
        release: Option<Receiver<()>>,
        inner: FakeAdapter,
    }
    impl EngineFactory for GateFactory {
        fn create(
            &self,
            engine: EngineId,
            _: &str,
            _: &GrainContext,
            _: ResetReason,
        ) -> Result<Box<dyn EngineAdapter>, RuntimeDiagnostic> {
            Ok(Box::new(GateAdapter {
                started: self.started.clone(),
                release: self.release.lock().unwrap().take(),
                inner: FakeAdapter {
                    engine,
                    count: 0,
                    invalid_output: false,
                },
            }))
        }
    }
    impl EngineAdapter for GateAdapter {
        fn capabilities(&self) -> EngineCapabilities {
            self.inner.capabilities()
        }
        fn render(
            &mut self,
            ctx: &GrainContext,
            cols: u16,
            rows: u16,
        ) -> Result<FrameOutput, RuntimeDiagnostic> {
            if let Some(release) = self.release.take() {
                self.started.send(()).unwrap();
                release.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            self.inner.render(ctx, cols, rows)
        }
    }
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let worker = PreviewWorker::with_factory(Arc::new(GateFactory {
        started: started_tx,
        release: Mutex::new(Some(release_rx)),
    }))
    .unwrap();
    worker.submit(request("slow", 1, 0));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.submit_for_engine(EngineId::Ascii, request("superseded", 2, 1));
    worker.submit_for_engine(EngineId::Ascii, request("latest", 3, 2));
    release_tx.send(()).unwrap();
    let done = completion(&worker);
    assert_eq!(done.revision, 3);
    assert_eq!(done.engine, EngineId::Ascii);
    assert_eq!(done.result.unwrap().frame, 2);
}
