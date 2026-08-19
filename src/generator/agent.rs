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
        r#"You are writing a p5.js audio-reactive sketch for Grain.
CONTRACT:
1. Implement `setup(p)` and `draw(p, ctx)`.
2. `ctx` provides `{ width, height, frame, time, seed, audio: { amplitude, low, mid, high } }`.
3. Use `p` methods: `p.background()`, `p.fill()`, `p.stroke()`, `p.circle()`, `p.rect()`, `p.line()`, etc.
4. Output ONLY the executable JavaScript code wrapped in a single ```javascript ... ``` codeblock."#
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
