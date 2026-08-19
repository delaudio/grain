use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "grain", author, version, about = "Terminal-first audio-reactive creative coding instrument")]
pub struct Cli {
    /// Path to an audio file (WAV or MP3)
    #[arg(value_name = "AUDIO_FILE")]
    pub audio_file: Option<PathBuf>,

    /// Target FPS for frame analysis and playback
    #[arg(short, long, default_value_t = 60)]
    pub fps: u32,
}
