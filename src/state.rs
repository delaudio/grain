use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    EditingPrompt,
    OpeningAudio,
    Help,
    Versions,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AudioStatus {
    #[default]
    None,
    Loading,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub path: Option<PathBuf>,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub status: AudioStatus,
}

impl Default for AudioInfo {
    fn default() -> Self {
        Self {
            path: None,
            duration_ms: 0,
            sample_rate: 44100,
            channels: 2,
            status: AudioStatus::None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PreviewStatus {
    #[default]
    Placeholder,
    Ready,
    Rendering,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewInfo {
    pub sketch_name: String,
    pub seed: u64,
    pub current_frame: usize,
    pub total_frames: usize,
    pub fps: u32,
    pub is_playing: bool,
    pub width: u32,
    pub height: u32,
    pub status: PreviewStatus,
}

impl Default for PreviewInfo {
    fn default() -> Self {
        Self {
            sketch_name: "initial_placeholder".to_string(),
            seed: 42,
            current_frame: 0,
            total_frames: 3600,
            fps: 60,
            is_playing: false,
            width: 800,
            height: 600,
            status: PreviewStatus::Placeholder,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GenerationStatus {
    #[default]
    Idle,
    Generating,
    Ready,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptInfo {
    pub active_prompt: String,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub generation_status: GenerationStatus,
    pub current_version: usize,
    pub total_versions: usize,
}

impl Default for PromptInfo {
    fn default() -> Self {
        Self {
            active_prompt: "geometric audio-reactive wave particles".to_string(),
            input_buffer: String::new(),
            cursor_position: 0,
            generation_status: GenerationStatus::Idle,
            current_version: 0,
            total_versions: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrainState {
    pub should_quit: bool,
    pub mode: InputMode,
    pub audio: AudioInfo,
    pub preview: PreviewInfo,
    pub prompt: PromptInfo,
    pub audio_input_buffer: String,
    pub status_message: Option<String>,
    pub terminal_size: (u16, u16),
}

impl Default for GrainState {
    fn default() -> Self {
        Self {
            should_quit: false,
            mode: InputMode::Normal,
            audio: AudioInfo::default(),
            preview: PreviewInfo::default(),
            prompt: PromptInfo::default(),
            audio_input_buffer: String::new(),
            status_message: Some("Ready. Press '?' for help.".to_string()),
            terminal_size: (80, 24),
        }
    }
}
