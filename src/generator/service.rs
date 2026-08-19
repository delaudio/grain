use std::sync::Arc;
use crate::audio::AudioFeatures;
use crate::generator::llm::LlmGenerator;
use crate::generator::mock::MockGenerator;
use crate::generator::provider::SketchGenerator;
use crate::runtime::{evaluate_frame, GrainContext};

pub struct GenerationService {
    generator: Arc<dyn SketchGenerator>,
}

impl GenerationService {
    pub fn new(generator: Arc<dyn SketchGenerator>) -> Self {
        Self { generator }
    }

    pub fn generate_and_validate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        let code = self.generator.generate(prompt, seed)?;
        self.validate_sketch(&code, seed)?;
        Ok(code)
    }

    pub fn revise_and_validate(
        &self,
        prompt: &str,
        current_sketch: &str,
        seed: u64,
    ) -> Result<String, String> {
        let code = self.generator.revise(prompt, current_sketch, seed)?;
        self.validate_sketch(&code, seed)?;
        Ok(code)
    }

    fn validate_sketch(&self, code: &str, seed: u64) -> Result<(), String> {
        let dummy_ctx = GrainContext {
            width: 800,
            height: 600,
            frame: 0,
            time: 0.0,
            seed,
            audio: AudioFeatures {
                amplitude: 0.5,
                low: 0.5,
                mid: 0.5,
                high: 0.5,
            },
        };

        match evaluate_frame(code, &dummy_ctx, 40, 10) {
            Ok(_) => Ok(()),
            Err(diag) => Err(format!("Generated sketch failed runtime validation: {}", diag)),
        }
    }
}

use crate::generator::agent::AgentCliGenerator;

pub fn create_default_generator() -> GenerationService {
    // 1. Direct custom CLI command (e.g. GRAIN_GENERATOR_CMD="claude -p" or "codex exec")
    if let Ok(cmd) = std::env::var("GRAIN_GENERATOR_CMD") {
        if !cmd.trim().is_empty() {
            return GenerationService::new(Arc::new(AgentCliGenerator::custom(&cmd)));
        }
    }

    // 2. Named agent provider (e.g. GRAIN_AI_PROVIDER="claude" or "codex")
    let provider = std::env::var("GRAIN_AI_PROVIDER").ok().map(|s| s.to_lowercase());
    let model = std::env::var("GRAIN_LLM_MODEL").ok();

    if let Some(ref p) = provider {
        if p == "claude" || p == "claude-code" {
            return GenerationService::new(Arc::new(AgentCliGenerator::claude(model.as_deref())));
        } else if p == "codex" {
            return GenerationService::new(Arc::new(AgentCliGenerator::codex(model.as_deref())));
        }
    }

    // 3. API Key based LLM (OpenAI / OpenRouter / Anthropic / Local API)
    let mut key = std::env::var("GRAIN_AI_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")).ok();

    if key.is_none() {
        if let Ok(content) = std::fs::read_to_string(".env") {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if (k == "OPENAI_API_KEY" || k == "GRAIN_AI_KEY") && !v.is_empty() {
                        key = Some(v.to_string());
                        break;
                    }
                }
            }
        }
    }

    if let Some(key_str) = key {
        if !key_str.trim().is_empty() {
            let base_url = std::env::var("OPENAI_BASE_URL").ok();
            let model = std::env::var("GRAIN_LLM_MODEL").ok();
            let llm = LlmGenerator::new(key_str, base_url, model);
            return GenerationService::new(Arc::new(llm));
        }
    }

    // 4. Default offline fixture generator
    GenerationService::new(Arc::new(MockGenerator::new()))
}
