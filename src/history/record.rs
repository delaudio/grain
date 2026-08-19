use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionMetadata {
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
