use std::process::Command;
use crate::generator::provider::SketchGenerator;

pub struct AgentCliGenerator {
    pub command: String,
    pub args: Vec<String>,
}

impl AgentCliGenerator {
    #[allow(dead_code)]
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self { command, args }
    }

    pub fn claude(model: Option<&str>) -> Self {
        let mut args = vec![
            "-p".to_string(),
            "--no-session-persistence".to_string(),
        ];
        if let Some(m) = model {
            let m_lower = m.trim().to_lowercase();
            // Only pass --model if it's a recognized Claude model
            if m_lower.contains("claude")
                || m_lower.contains("sonnet")
                || m_lower.contains("opus")
                || m_lower.contains("haiku")
            {
                args.push("--model".to_string());
                args.push(m.to_string());
            }
        }
        Self {
            command: "claude".to_string(),
            args,
        }
    }

    pub fn codex(model: Option<&str>) -> Self {
        let mut args = vec!["exec".to_string()];
        if let Some(m) = model {
            args.push("-m".to_string());
            args.push(m.to_string());
        }
        Self {
            command: "codex".to_string(),
            args,
        }
    }

    pub fn custom(custom_cmd: &str) -> Self {
        let parts: Vec<String> = custom_cmd
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        if parts.is_empty() {
            return Self::claude(None);
        }
        let prog = parts[0].clone();
        let args = parts[1..].to_vec();
        Self {
            command: prog,
            args,
        }
    }

    fn build_system_contract() -> &'static str {
        r#"You are a master creative coder and generative artist writing p5.js audio-reactive sketches for Grain.

CORE CONTRACT:
1. Implement `setup(p)` and `draw(p, ctx)`.
2. `ctx` provides:
   - ctx.width, ctx.height: canvas dimensions (e.g. 800x600)
   - ctx.frame: current frame number (integer)
   - ctx.time: elapsed time in seconds (float)
   - ctx.seed: deterministic seed (integer)
   - ctx.audio: { amplitude, low, mid, high } (all normalized 0.0 to 1.0)
3. Available drawing methods on `p`:
   `p.background()`, `p.fill()`, `p.stroke()`, `p.strokeWeight()`, `p.noFill()`, `p.noStroke()`,
   `p.circle()`, `p.rect()`, `p.line()`, `p.triangle()`, `p.point()`, `p.beginShape()`, `p.vertex()`, `p.endShape()`,
   `p.push()`, `p.pop()`, `p.translate()`, `p.rotate()`, `p.scale()`, `p.colorMode()`,
   `p.sin()`, `p.cos()`, `p.noise()`, `p.map()`, `p.createVector()`, `p.lerp()`, `p.dist()`, etc.

VISUAL DIVERSITY & STYLES (Match user prompt closely — avoid generic circular particles):
- Waveforms & Oscilloscopes: Horizontal flowing harmonic ribbons, Lissajous curves, frequency sweeps (`p.line`, `p.beginShape`).
- 3D Wireframe & Perspectives: Perspective horizon grids, flying synthwave terrain meshes, wireframe tunnels, rotating polyhedra.
- Matrix & Spectrograms: Segmented equalizer bar columns, modular digital matrices, geometric lattices (`p.rect`).
- Sacred Geometry & Mandalas: Radial symmetry, nested polygons, kaleidoscope reflections, spirograph loops.
- Flow Fields & Vector Streams: Perlin noise particle trails (`p.noise`), magnetic vector fields, fluid currents.
- Glitch & Brutalist: Sharp geometric scanlines, sliced strobe bands on transient kicks, high-contrast typography/glyphs.

COLOR & TRUECOLOR PALETTES:
- Set vibrant, expressive color palettes (`p.colorMode(p.HSB, 360, 100, 100, 1)` or RGB): Cyberpunk Neon, Sunset Gradient, Acid Matrix, Thermal Heatmap.
- Modulate colors and geometry with `audio.low` (bass punch/scale), `audio.mid` (morphing/wave), `audio.high` (electric sparks/flashes).

OUTPUT FORMAT:
Output ONLY executable JavaScript code in a ```javascript ... ``` codeblock. No conversational chat."#
    }

    fn strip_code_fences(code: &str) -> String {
        let trimmed = code.trim();
        if let Some(start_idx) = trimmed.find("```") {
            let after_fence = &trimmed[start_idx + 3..];
            let code_start = if let Some(newline_idx) = after_fence.find('\n') {
                &after_fence[newline_idx + 1..]
            } else {
                after_fence
            };

            if let Some(end_idx) = code_start.find("```") {
                return code_start[..end_idx].trim().to_string();
            }
        }
        trimmed.to_string()
    }
}

impl SketchGenerator for AgentCliGenerator {
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        let full_prompt = format!(
            "{}\n\nUSER PROMPT: {}\nDETERMINISTIC SEED: {}",
            Self::build_system_contract(),
            prompt,
            seed
        );

        let mut cmd = Command::new(&self.command);
        for arg in &self.args {
            cmd.arg(arg);
        }
        cmd.arg(&full_prompt);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute agent command '{}': {}", self.command, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Agent CLI '{}' failed (status {}): {}",
                self.command, output.status, stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let extracted = Self::strip_code_fences(&stdout);
        if extracted.trim().is_empty() {
            return Err("Agent CLI returned empty output".to_string());
        }

        Ok(extracted)
    }

    fn revise(&self, prompt: &str, current_sketch: &str, seed: u64) -> Result<String, String> {
        let full_prompt = format!(
            "{}\n\nREVISION REQUEST: {}\nSEED: {}\n\nCURRENT CODE:\n```javascript\n{}\n```",
            Self::build_system_contract(),
            prompt,
            seed,
            current_sketch
        );

        let mut cmd = Command::new(&self.command);
        for arg in &self.args {
            cmd.arg(arg);
        }
        cmd.arg(&full_prompt);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute agent command '{}': {}", self.command, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Agent CLI '{}' failed (status {}): {}",
                self.command, output.status, stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let extracted = Self::strip_code_fences(&stdout);
        if extracted.trim().is_empty() {
            return Err("Agent CLI returned empty output".to_string());
        }

        Ok(extracted)
    }
}
