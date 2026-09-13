use std::time::{Duration, Instant};

/// Monotonic fallback clock; an available audio position always takes priority.
pub struct PlaybackClock {
    last: Instant,
    position: Duration,
    playing: bool,
}

impl Default for PlaybackClock {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl PlaybackClock {
    pub fn new(now: Instant) -> Self {
        Self {
            last: now,
            position: Duration::ZERO,
            playing: false,
        }
    }

    pub fn update(&mut self, now: Instant, playing: bool, audio: Option<Duration>) -> Duration {
        if let Some(position) = audio {
            self.position = position;
        } else if self.playing {
            self.position = self
                .position
                .saturating_add(now.saturating_duration_since(self.last));
        }
        self.last = now;
        self.playing = playing;
        self.position
    }

    pub fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }

    pub fn frame(position: Duration, fps: u32, total_frames: usize) -> usize {
        let frame = (position.as_secs_f64() * f64::from(fps.max(1))).floor() as usize;
        if total_frames > 0 {
            frame % total_frames
        } else {
            frame
        }
    }
}
