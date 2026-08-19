use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod analyzer;
pub mod decoder;
pub mod dsp;
pub mod player;

pub use dsp::{process_features, DspSettings};
pub use player::AudioPlayer;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioFeatures {
    pub amplitude: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAnalysis {
    pub fps: u32,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: Vec<AudioFeatures>,
}

impl AudioAnalysis {
    pub fn get_features_at_frame(&self, frame: usize) -> AudioFeatures {
        if self.frames.is_empty() {
            return AudioFeatures::default();
        }
        if frame < self.frames.len() {
            self.frames[frame]
        } else {
            *self.frames.last().unwrap_or(&AudioFeatures::default())
        }
    }

    pub fn peak_features(&self) -> AudioFeatures {
        let mut peak = AudioFeatures::default();
        for f in &self.frames {
            if f.amplitude > peak.amplitude { peak.amplitude = f.amplitude; }
            if f.low > peak.low { peak.low = f.low; }
            if f.mid > peak.mid { peak.mid = f.mid; }
            if f.high > peak.high { peak.high = f.high; }
        }
        peak
    }

    #[allow(dead_code)]
    pub fn get_features_at_time(&self, time_sec: f64) -> AudioFeatures {
        if self.frames.is_empty() || time_sec < 0.0 {
            return AudioFeatures::default();
        }
        let frame = (time_sec * self.fps as f64).round() as usize;
        self.get_features_at_frame(frame)
    }

    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let analysis: Self = serde_json::from_str(&content)?;
        Ok(analysis)
    }
}

pub fn get_cache_path(audio_path: &Path, fps: u32) -> Result<PathBuf> {
    let bytes = fs::read(audio_path).with_context(|| format!("Failed to read audio file: {}", audio_path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();

    let cache_dir = PathBuf::from(".grain").join("cache");
    Ok(cache_dir.join(format!("{}_{}fps.json", hash, fps)))
}

pub fn load_or_analyze(audio_path: &Path, fps: u32) -> Result<AudioAnalysis> {
    let cache_path = get_cache_path(audio_path, fps).ok();

    if let Some(ref cp) = cache_path {
        if cp.exists() {
            if let Ok(analysis) = AudioAnalysis::load_from_file(cp) {
                return Ok(analysis);
            }
        }
    }

    let decoded = decoder::decode_audio_file(audio_path)?;
    let analysis = analyzer::analyze_decoded_audio(&decoded, fps);

    if let Some(ref cp) = cache_path {
        let _ = analysis.save_to_file(cp);
    }

    Ok(analysis)
}
