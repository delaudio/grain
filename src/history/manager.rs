use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Context, Result};
use crate::history::record::{GenerationHistory, VersionMetadata};

pub struct HistoryManager {
    grain_dir: PathBuf,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new(PathBuf::from(".grain"))
    }
}

impl HistoryManager {
    pub fn new(grain_dir: PathBuf) -> Self {
        Self { grain_dir }
    }

    fn generations_file(&self) -> PathBuf {
        self.grain_dir.join("generations.json")
    }

    fn sketches_dir(&self) -> PathBuf {
        self.grain_dir.join("sketches")
    }

    pub fn init_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.sketches_dir())?;
        Ok(())
    }

    pub fn load_history(&self) -> Result<GenerationHistory> {
        let path = self.generations_file();
        if !path.exists() {
            return Ok(GenerationHistory::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read generations file: {}", path.display()))?;
        let history: GenerationHistory = serde_json::from_str(&content)
            .with_context(|| "Failed to parse generations JSON")?;
        Ok(history)
    }

    pub fn save_history(&self, history: &GenerationHistory) -> Result<()> {
        self.init_dirs()?;
        let json = serde_json::to_string_pretty(history)?;
        fs::write(self.generations_file(), json)?;
        Ok(())
    }

    pub fn record_new_version(
        &self,
        prompt: &str,
        source: &str,
        seed: u64,
        provider: &str,
        audio_hash: Option<&str>,
    ) -> Result<VersionMetadata> {
        self.init_dirs()?;
        let mut history = self.load_history().unwrap_or_default();

        let next_version = history.versions.len() + 1;
        let file_name = format!("{:03}.js", next_version);
        let sketch_path = self.sketches_dir().join(&file_name);

        fs::write(&sketch_path, source)
            .with_context(|| format!("Failed to write sketch file: {}", sketch_path.display()))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let meta = VersionMetadata {
            version: next_version,
            timestamp,
            prompt: prompt.to_string(),
            seed,
            provider: provider.to_string(),
            runtime_contract: "grain-p5-v1".to_string(),
            audio_source_hash: audio_hash.map(|s| s.to_string()),
            sketch_file: file_name,
        };

        history.versions.push(meta.clone());
        history.active_version = next_version;
        self.save_history(&history)?;

        Ok(meta)
    }

    pub fn load_sketch_content(&self, version_file: &str) -> Result<String> {
        let path = self.sketches_dir().join(version_file);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read sketch file: {}", path.display()))?;
        Ok(content)
    }
}
