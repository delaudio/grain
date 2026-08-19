pub const DEFAULT_SKETCH_TEMPLATE: &str = r#"// Grain Audio-Reactive p5.js Sketch
// Contract:
// setup(p) - called once on initialization
// draw(p, ctx) - called on every frame with:
//   ctx = { width, height, frame, time, seed, audio: { amplitude, low, mid, high } }

function setup(p) {
  p.noStroke();
}

function draw(p, ctx) {
  p.background(15, 18, 28);

  const cx = ctx.width / 2;
  const cy = ctx.height / 2;
  const baseSize = Math.min(ctx.width, ctx.height) * 0.2;
  const amp = ctx.audio.amplitude;
  const low = ctx.audio.low;
  const mid = ctx.audio.mid;
  const high = ctx.audio.high;

  // React to low-frequency energy (bass pulse)
  const radius = baseSize + low * 120 + amp * 50;

  // Outer reactive glow
  p.fill(200 * mid, 80, 255 * high, 0.4);
  p.circle(cx, cy, radius * 1.4);

  // Core visual element
  p.fill(255, 100 + 155 * mid, 50 + 200 * low, 0.9);
  p.circle(cx, cy, radius);

  // Orbital wave particles
  const numParticles = 12;
  for (let i = 0; i < numParticles; i++) {
    const angle = (i / numParticles) * Math.PI * 2 + ctx.time * 2.0;
    const dist = radius * 1.6 + Math.sin(ctx.time * 4.0 + i) * (20 * mid);
    const px = cx + Math.cos(angle) * dist;
    const py = cy + Math.sin(angle) * dist;

    p.fill(80 + 175 * high, 220, 255, 0.8);
    p.circle(px, py, 6 + 10 * high);
  }
}
"#;
