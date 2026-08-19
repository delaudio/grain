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
