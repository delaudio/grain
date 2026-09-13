use crate::audio::AudioFeatures;
use crate::generator::llm::LlmGenerator;
use crate::generator::mock::MockGenerator;
use crate::generator::provider::SketchGenerator;
use crate::runtime::GrainContext;
use crate::runtime::engine::{EngineFactory, EngineId, ResetReason};
use std::sync::Arc;

pub struct GenerationService {
    generator: Arc<dyn SketchGenerator>,
}

impl GenerationService {
    pub fn new(generator: Arc<dyn SketchGenerator>) -> Self {
        Self { generator }
    }

    pub fn generate_and_validate(&self, prompt: &str, seed: u64) -> Result<String, String> {
        self.generate_for_engine(EngineId::P5, prompt, seed)
    }

    pub fn generate_for_engine(
        &self,
        engine: EngineId,
        prompt: &str,
        seed: u64,
    ) -> Result<String, String> {
        let code = self.generator.generate_for_engine(engine, prompt, seed)?;
        Self::validate_for_engine(engine, &code, seed)?;
        Ok(code)
    }

    pub fn revise_and_validate(
        &self,
        prompt: &str,
        current_sketch: &str,
        seed: u64,
    ) -> Result<String, String> {
        self.revise_for_engine(EngineId::P5, prompt, current_sketch, seed)
    }

    pub fn revise_for_engine(
        &self,
        engine: EngineId,
        prompt: &str,
        current_sketch: &str,
        seed: u64,
    ) -> Result<String, String> {
        let code = self
            .generator
            .revise_for_engine(engine, prompt, current_sketch, seed)?;
        Self::validate_for_engine(engine, &code, seed)?;
        Ok(code)
    }

    pub fn validate_for_engine(engine: EngineId, code: &str, seed: u64) -> Result<(), String> {
        let dummy_ctx = GrainContext {
            params: Default::default(),
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

        let result = crate::runtime::builtin::BuiltinEngineFactory
            .create(engine, code, &dummy_ctx, ResetReason::SourceChanged)
            .and_then(|mut adapter| adapter.render(&dummy_ctx, 40, 10))
            .and_then(|output| output.validate());
        match result {
            Ok(_) => Ok(()),
            Err(diag) => Err(format!(
                "Generated sketch failed runtime validation: {}",
                diag
            )),
        }
    }
}

use crate::generator::agent::AgentCliGenerator;

pub fn create_default_generator() -> GenerationService {
    if std::env::var("GRAIN_OFFLINE").is_ok_and(|value| value == "1") {
        return GenerationService::new(Arc::new(MockGenerator::new()));
    }
    // 1. Direct custom CLI command (e.g. GRAIN_GENERATOR_CMD="claude -p" or "codex exec")
    if let Ok(cmd) = std::env::var("GRAIN_GENERATOR_CMD")
        && !cmd.trim().is_empty()
    {
        return GenerationService::new(Arc::new(AgentCliGenerator::custom(&cmd)));
    }

    // 2. Named agent provider (e.g. GRAIN_AI_PROVIDER="claude" or "codex")
    let provider = std::env::var("GRAIN_AI_PROVIDER")
        .ok()
        .map(|s| s.to_lowercase());

    if let Some(ref p) = provider {
        if p == "claude" || p == "claude-code" {
            let claude_model = std::env::var("GRAIN_CLAUDE_MODEL")
                .or_else(|_| std::env::var("GRAIN_LLM_MODEL"))
                .ok();
            return GenerationService::new(Arc::new(AgentCliGenerator::claude(
                claude_model.as_deref(),
            )));
        } else if p == "codex" {
            let codex_model = std::env::var("GRAIN_CODEX_MODEL")
                .or_else(|_| std::env::var("GRAIN_LLM_MODEL"))
                .ok();
            return GenerationService::new(Arc::new(AgentCliGenerator::codex(
                codex_model.as_deref(),
            )));
        }
    }

    // 3. API Key based LLM (OpenAI / OpenRouter / Anthropic / Local API)
    let mut key = std::env::var("GRAIN_AI_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok();

    if key.is_none()
        && let Ok(content) = std::fs::read_to_string(".env")
    {
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

    if let Some(key_str) = key
        && !key_str.trim().is_empty()
    {
        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        let model = std::env::var("GRAIN_LLM_MODEL").ok();
        let llm = LlmGenerator::new(key_str, base_url, model);
        return GenerationService::new(Arc::new(llm));
    }

    // 4. Default offline fixture generator
    GenerationService::new(Arc::new(MockGenerator::new()))
}
