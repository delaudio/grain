// Grain Audio-Reactive Sketch: Wave Particles
// Demonstrates p5.js runtime contract using amplitude, low, mid, high audio features.

function setup(p) {
  p.noStroke();
}

function draw(p, ctx) {
  p.background(10, 12, 22);

  const cx = ctx.width / 2;
  const cy = ctx.height / 2;
  const bass = ctx.audio.low;
  const mid = ctx.audio.mid;
  const treble = ctx.audio.high;
  const amp = ctx.audio.amplitude;

  // Pulsing reactive core
  const coreRadius = 40 + bass * 120 + amp * 60;
  p.fill(255, 80 + 175 * mid, 100 + 155 * treble, 0.85);
  p.circle(cx, cy, coreRadius);

  // Outer concentric halo
  p.fill(120, 200 * mid, 255 * treble, 0.35);
  p.circle(cx, cy, coreRadius * 1.6);

  // Audio-reactive orbiting rings
  const particleCount = 16;
  for (let i = 0; i < particleCount; i++) {
    const angle = (i / particleCount) * Math.PI * 2 + ctx.time * 2.5;
    const distance = coreRadius * 1.5 + Math.sin(ctx.time * 5.0 + i) * (25 * mid);
    const px = cx + Math.cos(angle) * distance;
    const py = cy + Math.sin(angle) * distance;

    p.fill(200 + 55 * treble, 240, 255, 0.9);
    p.circle(px, py, 8 + 14 * treble);
  }
}
