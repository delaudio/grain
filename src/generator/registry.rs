use std::sync::Arc;
use crate::generator::agent::AgentCliGenerator;
use crate::generator::llm::LlmGenerator;
use crate::generator::mock::MockGenerator;
use crate::generator::provider::SketchGenerator;
use crate::generator::service::GenerationService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineKind {
    ClaudeCli,
    CodexCli,
    OpenAiApi,
    CustomCmd(String),
    OfflineMock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOption {
    pub id: String,
    pub label: String,
    pub kind: EngineKind,
    pub model: Option<String>,
    pub is_available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSelectionState {
    pub options: Vec<EngineOption>,
    pub selected_index: usize,
    pub active_index: usize,
}

impl Default for EngineSelectionState {
    fn default() -> Self {
        Self::discover()
    }
}

impl EngineSelectionState {
    pub fn discover() -> Self {
        // Read .env if present
        let mut env_openai_key = std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("GRAIN_AI_KEY")).ok();
        let mut env_provider = std::env::var("GRAIN_AI_PROVIDER").ok().map(|s| s.to_lowercase());
        let mut env_custom_cmd = std::env::var("GRAIN_GENERATOR_CMD").ok();

        if let Ok(content) = std::fs::read_to_string(".env") {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    if env_openai_key.is_none() && (k == "OPENAI_API_KEY" || k == "GRAIN_AI_KEY") && !v.is_empty() {
                        env_openai_key = Some(v.clone());
                    }
                    if env_provider.is_none() && k == "GRAIN_AI_PROVIDER" && !v.is_empty() {
                        env_provider = Some(v.to_lowercase());
                    }
                    if env_custom_cmd.is_none() && k == "GRAIN_GENERATOR_CMD" && !v.is_empty() {
                        env_custom_cmd = Some(v);
                    }
                }
            }
        }

        let has_claude_cli = check_command_exists("claude")
            || std::env::var("HOME").map(|h| std::path::Path::new(&h).join(".local/bin/claude").exists()).unwrap_or(false);
        let has_codex_cli = check_command_exists("codex");
        let has_openai_key = env_openai_key.as_ref().map(|k| !k.trim().is_empty()).unwrap_or(false);

        let mut options = Vec::new();

        // 1. Claude Code CLI options
        options.push(EngineOption {
            id: "claude-default".to_string(),
            label: "Claude Code CLI (Default)".to_string(),
            kind: EngineKind::ClaudeCli,
            model: None,
            is_available: has_claude_cli,
            detail: "Local agent CLI via `claude -p`".to_string(),
        });
        options.push(EngineOption {
            id: "claude-sonnet".to_string(),
            label: "Claude Code CLI (Sonnet)".to_string(),
            kind: EngineKind::ClaudeCli,
            model: Some("sonnet".to_string()),
            is_available: has_claude_cli,
            detail: "Claude 3.7 / 3.5 Sonnet via CLI".to_string(),
        });
        options.push(EngineOption {
            id: "claude-opus".to_string(),
            label: "Claude Code CLI (Opus)".to_string(),
            kind: EngineKind::ClaudeCli,
            model: Some("opus".to_string()),
            is_available: has_claude_cli,
            detail: "Claude 3 Opus via CLI".to_string(),
        });
        options.push(EngineOption {
            id: "claude-haiku".to_string(),
            label: "Claude Code CLI (Haiku)".to_string(),
            kind: EngineKind::ClaudeCli,
            model: Some("haiku".to_string()),
            is_available: has_claude_cli,
            detail: "Fast lightweight Claude model".to_string(),
        });

        // 2. Codex CLI options
        options.push(EngineOption {
            id: "codex-default".to_string(),
            label: "Codex CLI (Default)".to_string(),
            kind: EngineKind::CodexCli,
            model: None,
            is_available: has_codex_cli,
            detail: "Local agent CLI via `codex exec`".to_string(),
        });
        options.push(EngineOption {
            id: "codex-o3-mini".to_string(),
            label: "Codex CLI (o3-mini)".to_string(),
            kind: EngineKind::CodexCli,
            model: Some("o3-mini".to_string()),
            is_available: has_codex_cli,
            detail: "OpenAI reasoning model via Codex CLI".to_string(),
        });

        // 3. OpenAI API options
        let openai_key_detail = if has_openai_key {
            "API Key detected in .env"
        } else {
            "Requires OPENAI_API_KEY in .env"
        };
        options.push(EngineOption {
            id: "openai-gpt-4o-mini".to_string(),
            label: "OpenAI API (gpt-4o-mini)".to_string(),
            kind: EngineKind::OpenAiApi,
            model: Some("gpt-4o-mini".to_string()),
            is_available: has_openai_key,
            detail: format!("Fast remote API ({})", openai_key_detail),
        });
        options.push(EngineOption {
            id: "openai-gpt-4o".to_string(),
            label: "OpenAI API (gpt-4o)".to_string(),
            kind: EngineKind::OpenAiApi,
            model: Some("gpt-4o".to_string()),
            is_available: has_openai_key,
            detail: format!("High-capability remote API ({})", openai_key_detail),
        });
        options.push(EngineOption {
            id: "openai-o3-mini".to_string(),
            label: "OpenAI API (o3-mini)".to_string(),
            kind: EngineKind::OpenAiApi,
            model: Some("o3-mini".to_string()),
            is_available: has_openai_key,
            detail: format!("Reasoning remote API ({})", openai_key_detail),
        });

        // 4. Custom command if set
        if let Some(cmd) = env_custom_cmd {
            if !cmd.trim().is_empty() {
                options.push(EngineOption {
                    id: "custom-cmd".to_string(),
                    label: format!("Custom Command ({})", cmd),
                    kind: EngineKind::CustomCmd(cmd),
                    model: None,
                    is_available: true,
                    detail: "User configured CLI generator command".to_string(),
                });
            }
        }

        // 5. Deterministic offline mock
        options.push(EngineOption {
            id: "offline-mock".to_string(),
            label: "Offline Mock Generator".to_string(),
            kind: EngineKind::OfflineMock,
            model: None,
            is_available: true,
            detail: "Instant deterministic templates (zero latency / no keys)".to_string(),
        });

        // Determine active engine index based on .env / config
        let active_index = if let Some(ref p) = env_provider {
            if p == "claude" || p == "claude-code" {
                0
            } else if p == "codex" {
                4
            } else {
                options.iter().position(|o| o.is_available).unwrap_or(options.len() - 1)
            }
        } else if has_openai_key {
            options.iter().position(|o| o.id == "openai-gpt-4o-mini").unwrap_or(0)
        } else if has_claude_cli {
            0
        } else {
            options.len() - 1
        };

        Self {
            selected_index: active_index,
            active_index,
            options,
        }
    }

    pub fn active_option(&self) -> &EngineOption {
        &self.options[self.active_index.min(self.options.len() - 1)]
    }

    pub fn active_label(&self) -> &str {
        &self.active_option().label
    }

    pub fn create_service_for_active(&self) -> GenerationService {
        self.create_service_for_option(self.active_option())
    }

    pub fn create_service_for_option(&self, option: &EngineOption) -> GenerationService {
        let generator: Arc<dyn SketchGenerator> = match &option.kind {
            EngineKind::ClaudeCli => {
                Arc::new(AgentCliGenerator::claude(option.model.as_deref()))
            }
            EngineKind::CodexCli => {
                Arc::new(AgentCliGenerator::codex(option.model.as_deref()))
            }
            EngineKind::CustomCmd(cmd) => {
                Arc::new(AgentCliGenerator::custom(cmd))
            }
            EngineKind::OpenAiApi => {
                let mut key = std::env::var("GRAIN_AI_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")).ok();
                if key.is_none() {
                    if let Ok(content) = std::fs::read_to_string(".env") {
                        for line in content.lines() {
                            if let Some((k, v)) = line.trim().split_once('=') {
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
                    let base_url = std::env::var("OPENAI_BASE_URL").ok();
                    let model = option.model.clone().or_else(|| std::env::var("GRAIN_LLM_MODEL").ok());
                    Arc::new(LlmGenerator::new(key_str, base_url, model))
                } else {
                    Arc::new(MockGenerator::new())
                }
            }
            EngineKind::OfflineMock => Arc::new(MockGenerator::new()),
        };

        GenerationService::new(generator)
    }
}

fn check_command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
