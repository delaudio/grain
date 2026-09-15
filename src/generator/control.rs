use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Shared cancellation and a deadline measured from request creation.
#[derive(Clone)]
pub struct GenerationControl {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl Default for GenerationControl {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl GenerationControl {
    pub fn new(timeout: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + timeout.min(Duration::from_secs(30)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err("Generation cancelled".into());
        }
        if Instant::now() >= self.deadline {
            return Err("Generation timed out".into());
        }
        Ok(())
    }
}
