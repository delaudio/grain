//! Worker throughput only, not terminal FPS or keyboard latency.
use grain::audio::AudioFeatures;
use grain::preview::worker::{PreviewWorker, RenderRequest};
use grain::runtime::GrainContext;
use grain::runtime::engine::EngineId;
use ratatui::layout::Rect;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = PreviewWorker::new()?;
    let source: Arc<str> = Arc::from(
        r#"
function setup(p) { p.noStroke(); }
function draw(p, ctx) {
  p.background(8, 12, 20);
  for (let i = 0; i < 64; i++) {
    p.fill((i * 37) % 256, (i * 71) % 256, (i * 113) % 256);
    p.rect((i % 8) * 100, Math.floor(i / 8) * 75, 85, 60);
  }
  p.fill(255, 255, 255);
  p.circle(400 + Math.sin(ctx.time) * 260, 300, 80);
}
"#,
    );
    let start = Instant::now();
    let deadline = start + Duration::from_secs(15);
    let mut times = Vec::new();
    let mut packet_bytes = 0usize;
    for frame in 0..90 {
        let submitted = Instant::now();
        worker.submit_iterm2_for_engine(
            EngineId::P5,
            RenderRequest {
                revision: 1,
                lifecycle: 0,
                source: Arc::clone(&source),
                cols: 80,
                rows: 24,
                context: GrainContext {
                    width: 800,
                    height: 600,
                    frame,
                    time: frame as f64 / 30.0,
                    seed: 42,
                    params: Default::default(),
                    audio: AudioFeatures {
                        amplitude: 0.5,
                        low: 0.5,
                        mid: 0.5,
                        high: 0.5,
                    },
                },
            },
            Rect::new(1, 7, 80, 24),
            Rect::new(0, 0, 100, 40),
        );
        loop {
            if let Some(completion) = worker.take_engine_completed() {
                let packet = completion
                    .image_packet
                    .ok_or("missing image packet")?
                    .map_err(|error| error.to_string())?;
                packet_bytes += packet.len();
                times.push(submitted.elapsed().as_secs_f64() * 1000.0);
                break;
            }
            if Instant::now() >= deadline {
                return Err("benchmark exceeded 15 seconds".into());
            }
            std::thread::sleep(Duration::from_micros(250));
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    times.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({
            "scope": "worker-only; excludes terminal writes and input latency",
            "source_canvas": [800, 600],
            "max_presentation_pixels": grain::preview::iterm2::MAX_IMAGE_PIXELS,
            "frames": times.len(),
            "elapsed_seconds": elapsed,
            "worker_frames_per_second": times.len() as f64 / elapsed,
            "completion_p50_ms": times[times.len() / 2],
            "completion_p95_ms": times[(times.len() * 95 / 100).min(times.len() - 1)],
            "mean_packet_bytes": packet_bytes / times.len(),
        })
    );
    Ok(())
}
