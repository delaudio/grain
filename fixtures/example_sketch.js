function setup(p) {
  p.noStroke();
}

function draw(p, ctx) {
  p.background(0);
  const r = 50 + ctx.audio.amplitude * 100;
  p.fill(255 * ctx.audio.mid, 120, 255 * ctx.audio.high);
  p.circle(ctx.width / 2, ctx.height / 2, r);
}
