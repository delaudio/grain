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
        r#"You are an expert creative coder writing p5.js sketches for Grain, an audio-reactive terminal instrument.

CONTRACT RULES:
1. Write pure JavaScript implementing `setup(p)` and `draw(p, ctx)`.
2. `ctx` provides:
   - ctx.width: number (canvas width)
   - ctx.height: number (canvas height)
   - ctx.frame: number (current frame index)
   - ctx.time: number (elapsed time in seconds)
   - ctx.seed: number (random seed)
   - ctx.audio: { amplitude, low, mid, high } (all normalized 0.0 to 1.0)
3. Use the `p` instance for drawing: `p.background()`, `p.fill()`, `p.stroke()`, `p.circle()`, `p.rect()`, `p.line()`, `p.push()`, `p.pop()`, etc.
4. Output ONLY valid executable JavaScript code. Do not wrap in markdown or backticks."#
    }

    fn strip_code_fences(code: &str) -> String {
        let trimmed = code.trim();
        let stripped = if trimmed.starts_with("```javascript") {
            trimmed.trim_start_matches("```javascript")
        } else if trimmed.starts_with("```js") {
            trimmed.trim_start_matches("```js")
        } else if trimmed.starts_with("```") {
            trimmed.trim_start_matches("```")
        } else {
            trimmed
        };
        stripped.trim_end_matches("```").trim().to_string()
    }
}

impl SketchGenerator for LlmGenerator {
    fn generate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        let user_prompt = format!("Generate a new p5.js audio-reactive sketch for the prompt: \"{}\". Deterministic seed: {}.", prompt, seed);
        
        let client = reqwest::blocking::Client::new();
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
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("LLM API returned error status: {}", response.status()));
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

        let client = reqwest::blocking::Client::new();
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
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("LLM API returned error status: {}", response.status()));
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
