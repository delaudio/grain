use crate::runtime::engine::{EngineId, default_contract_version};
use crate::runtime::parameters::Parameters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionInputs {
    pub engine: EngineId,
    pub seed: u64,
    pub params: Parameters,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionMetadata {
    #[serde(default)]
    pub params: Parameters,
    #[serde(default)]
    pub engine: EngineId,
    #[serde(default = "default_contract_version")]
    pub contract_version: u32,
    pub version: usize,
    pub timestamp: u64,
    pub prompt: String,
    pub seed: u64,
    pub provider: String,
    pub runtime_contract: String,
    pub audio_source_hash: Option<String>,
    pub sketch_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct GenerationHistory {
    pub active_version: usize,
    pub versions: Vec<VersionMetadata>,
}
