use crate::generator::provider::SketchGenerator;

#[derive(Debug, Default, Clone)]
pub struct MockGenerator;

impl MockGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl SketchGenerator for MockGenerator {
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        let p_lower = prompt.to_lowercase();

        if p_lower.contains("tunnel") || p_lower.contains("polygon") || p_lower.contains("geometric") {
            Ok(format!(
                r#"// Grain Generated Sketch: Geometric Audio Tunnel [Seed: {}]
function setup(p) {{
  p.noStroke();
}}

function draw(p, ctx) {{
  p.background(10, 10, 18);
  const cx = ctx.width / 2;
  const cy = ctx.height / 2;
  const rings = 8;
  for (let i = 0; i < rings; i++) {{
    const t = (ctx.time * 0.8 + i / rings) % 1.0;
    const size = t * Math.min(ctx.width, ctx.height) * 0.9;
    const bass = ctx.audio.low;
    const treble = ctx.audio.high;
    p.fill(20 + 200 * t * bass, 50 + 150 * treble, 255 * (1 - t), 0.7);
    p.circle(cx, cy, size * (1.0 + 0.3 * bass));
  }}
}}
"#,
                seed
            ))
        } else if p_lower.contains("wave") || p_lower.contains("oscillator") || p_lower.contains("sine") {
            Ok(format!(
                r#"// Grain Generated Sketch: Oscillating Sine Wave [Seed: {}]
function setup(p) {{
  p.noStroke();
}}

function draw(p, ctx) {{
  p.background(8, 12, 24);
  const cy = ctx.height / 2;
  const count = 30;
  const step = ctx.width / count;
  for (let i = 0; i < count; i++) {{
    const x = i * step;
    const wave = Math.sin(ctx.time * 4.0 + i * 0.3) * (ctx.audio.amplitude * 120 + 20);
    const y = cy + wave * (1.0 + ctx.audio.mid);
    p.fill(50 + 205 * ctx.audio.high, 180, 255 * ctx.audio.low, 0.85);
    p.circle(x, y, 12 + 15 * ctx.audio.low);
  }}
}}
"#,
                seed
            ))
        } else if p_lower.contains("particle") || p_lower.contains("star") || p_lower.contains("orbit") {
            Ok(format!(
                r#"// Grain Generated Sketch: Orbital Particle Field [Seed: {}]
function setup(p) {{
  p.noStroke();
}}

function draw(p, ctx) {{
  p.background(12, 14, 26);
  const cx = ctx.width / 2;
  const cy = ctx.height / 2;
  const count = 18;
  for (let i = 0; i < count; i++) {{
    const angle = (i / count) * Math.PI * 2 + ctx.time * 1.8;
    const r = 60 + ctx.audio.low * 100 + Math.sin(ctx.time * 3.0 + i) * 30;
    const px = cx + Math.cos(angle) * r;
    const py = cy + Math.sin(angle) * r;
    p.fill(255 * ctx.audio.amplitude, 150 + 105 * ctx.audio.mid, 240, 0.9);
    p.circle(px, py, 10 + 20 * ctx.audio.high);
  }}
}}
"#,
                seed
            ))
        } else {
            Ok(format!(
                r#"// Grain Generated Sketch: Audio Reactive Core [Seed: {}]
function setup(p) {{
  p.noStroke();
}}

function draw(p, ctx) {{
  p.background(15, 18, 30);
  const cx = ctx.width / 2;
  const cy = ctx.height / 2;
  const pulse = 40 + ctx.audio.low * 130 + ctx.audio.amplitude * 60;
  p.fill(255 * ctx.audio.mid, 120, 255 * ctx.audio.high, 0.85);
  p.circle(cx, cy, pulse);
  p.fill(100, 200 * ctx.audio.low, 255, 0.4);
  p.circle(cx, cy, pulse * 1.5);
}}
"#,
                seed
            ))
        }
    }

    fn revise(&self, prompt: &str, current_sketch: &str, seed: u64) -> Result<String, String> {
        let p_lower = prompt.to_lowercase();
        let mut revised = current_sketch.to_string();

        if p_lower.contains("red") {
            revised = revised.replace("p.fill(100,", "p.fill(255,");
            revised = revised.replace("p.fill(15, 18, 30);", "p.fill(30, 10, 15);");
            revised = format!("// Revised: Red Accent Shift\n{}", revised);
        } else if p_lower.contains("blue") || p_lower.contains("cyan") {
            revised = revised.replace("p.fill(255,", "p.fill(80, 220, 255,");
            revised = format!("// Revised: Cyan Tint Shift\n{}", revised);
        } else if p_lower.contains("fast") || p_lower.contains("speed") {
            revised = revised.replace("ctx.time *", "ctx.time * 2.5 *");
            revised = format!("// Revised: High Speed Mode\n{}", revised);
        } else if p_lower.contains("bigger") || p_lower.contains("large") {
            revised = revised.replace("pulse * 1.5", "pulse * 2.2");
            revised = format!("// Revised: Enlarged Scale\n{}", revised);
        } else {
            // General revision
            revised = format!("// Revised for prompt: {}\n{}", prompt, revised);
        }

        // Add revision timestamp / seed comment
        revised = format!("// Grain Revision [Seed: {}]\n{}", seed, revised);
        Ok(revised)
    }
}
