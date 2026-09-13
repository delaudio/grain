use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct AudioPlayer {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    current_path: Option<PathBuf>,
    generation: u64,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        match OutputStream::try_default() {
            Ok((stream, stream_handle)) => {
                let sink = Sink::try_new(&stream_handle).ok();
                Self {
                    _stream: Some(stream),
                    stream_handle: Some(stream_handle),
                    sink,
                    current_path: None,
                    generation: 0,
                }
            }
            Err(_) => {
                // Headless or no audio hardware available
                Self {
                    _stream: None,
                    stream_handle: None,
                    sink: None,
                    current_path: None,
                    generation: 0,
                }
            }
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        self.current_path = Some(path.to_path_buf());
        self.reset_sink()?;
        Ok(())
    }

    fn reset_sink(&mut self) -> Result<(), String> {
        if let Some(ref handle) = self.stream_handle
            && let Some(ref path) = self.current_path
        {
            let file = File::open(path).map_err(|e| format!("Failed to open audio: {}", e))?;
            let reader = BufReader::new(file);
            let source =
                Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {}", e))?;

            let new_sink =
                Sink::try_new(handle).map_err(|e| format!("Failed to create sink: {}", e))?;
            new_sink.append(source);
            new_sink.pause();
            self.sink = Some(new_sink);
            self.generation = self.generation.wrapping_add(1);
        }
        Ok(())
    }

    pub fn play(&mut self) {
        if let Some(ref sink) = self.sink {
            if sink.empty() {
                // If it reached the end, reset and play again
                let _ = self.reset_sink();
                if let Some(ref s) = self.sink {
                    s.play();
                }
            } else {
                sink.play();
            }
        }
    }

    pub fn pause(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.pause();
        }
    }

    /// Position reported by the audio sink, including pauses and seeks.
    pub fn position(&self) -> Option<Duration> {
        self.current_path.as_ref()?;
        self.sink.as_ref().map(Sink::get_pos)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn restart(&mut self) {
        let _ = self.reset_sink();
        if let Some(ref sink) = self.sink {
            sink.play();
        }
    }

    #[allow(dead_code)]
    pub fn seek_frame(&mut self, frame: usize, fps: u32) {
        let seconds = frame as f64 / fps as f64;
        let _ = self.reset_sink();
        if let Some(ref sink) = self.sink {
            let _ = sink.try_seek(Duration::from_secs_f64(seconds));
        }
    }

    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.sink
            .as_ref()
            .map(|s| !s.is_paused() && !s.empty())
            .unwrap_or(false)
    }
}
