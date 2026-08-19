use serde_json::json;
use crate::generator::provider::SketchGenerator;

pub struct LlmGenerator {
    api_key: String,
    base_url: String,
    model: String,
}

impl LlmGenerator {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
        }
    }

    fn build_system_prompt() -> &'static str {
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

VISUAL DIVERSITY & STYLES (Match the user's prompt closely — DO NOT just draw standard central particle circles):
- Waveforms & Oscilloscopes: Horizontal flowing harmonic ribbons, Lissajous curves, frequency sweeps (`p.line`, `p.beginShape`).
- 3D Wireframe & Perspectives: Perspective horizon grids, flying synthwave terrain meshes, wireframe tunnels, rotating polyhedra.
- Matrix & Spectrograms: Segmented equalizer bar columns, modular digital matrices, geometric lattices (`p.rect`).
- Sacred Geometry & Mandalas: Radial symmetry, nested polygons, kaleidoscope reflections, spirograph loops.
- Flow Fields & Vector Streams: Perlin noise particle trails (`p.noise`), magnetic vector fields, fluid currents.
- Glitch & Brutalist: Sharp geometric scanlines, sliced strobe bands on transient kicks, high-contrast typography/glyphs.

COLOR & TRUECOLOR PALETTES:
- Use vibrant, purposeful color schemes (e.g. `p.colorMode(p.HSB, 360, 100, 100, 1)` or RGB):
  * Cyberpunk: Neon Cyan (180°), Hot Magenta (320°), Deep Indigo
  * Sunset Gradient: Warm Orange/Amber to Violet
  * Matrix / Bioluminescent: Neon Emerald to Lime with dark background
  * Thermal Heatmap: Deep Blue -> Red -> Yellow -> White
- Drive colors with audio:
  * `audio.low` (Kick/Bass): Background pulse, scale explosion, contrast shifts.
  * `audio.mid` (Synths/Vocals): Morphing geometry, wave amplitude, rotation.
  * `audio.high` (Hi-hats/Snares): Electric sparks, strobe flashes, radiating line bursts.

OUTPUT FORMAT:
Output ONLY executable JavaScript code in a ```javascript ... ``` codeblock. No conversational text."#
    }

    fn strip_code_fences(code: &str) -> String {
        let trimmed = code.trim();
        
        // If there's a code block anywhere in the output, extract it
        if let Some(start_idx) = trimmed.find("```") {
            let after_fence = &trimmed[start_idx + 3..];
            // Skip language tag (e.g. javascript, js) until newline
            let code_start = if let Some(newline_idx) = after_fence.find('\n') {
                &after_fence[newline_idx + 1..]
            } else {
                after_fence
            };

            if let Some(end_idx) = code_start.rfind("```") {
                return code_start[..end_idx].trim().to_string();
            }
        }

        trimmed.to_string()
    }
}

impl SketchGenerator for LlmGenerator {
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        let user_prompt = format!("Generate a new p5.js audio-reactive sketch for the prompt: \"{}\". Deterministic seed: {}.", prompt, seed);
        
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let payload = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": Self::build_system_prompt() },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.7
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key.trim()))
            .json(&payload)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("LLM API returned error {}: {}", status, error_body));
        }

        let json_body: serde_json::Value = response
            .json()
            .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

        let content = json_body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "Missing content in LLM response".to_string())?;

        Ok(Self::strip_code_fences(content))
    }

    fn revise(&self, prompt: &str, current_sketch: &str, seed: u64) -> Result<String, String> {
        let user_prompt = format!(
            "Revise the following existing p5.js sketch based on user request: \"{}\". Seed: {}.\n\nExisting code:\n```javascript\n{}\n```",
            prompt, seed, current_sketch
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let payload = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": Self::build_system_prompt() },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.7
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key.trim()))
            .json(&payload)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("LLM API returned error {}: {}", status, error_body));
        }

        let json_body: serde_json::Value = response
            .json()
            .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

        let content = json_body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "Missing content in LLM response".to_string())?;

        Ok(Self::strip_code_fences(content))
    }
}
