//! CPAL caches a COM enumerator process-wide. Its creating STA must outlive
//! every caller, including short-lived test/UI threads (RustAudio/cpal#1302).
//! Keep every backend operation and destructor on one process-lifetime owner.
use super::backend::AudioPlayer as Backend;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

type Players = HashMap<u64, Backend>;
type Job = super::owner_request::Job<Players>;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

fn owner() -> Option<mpsc::Sender<Job>> {
    static OWNER: OnceLock<Option<mpsc::Sender<Job>>> = OnceLock::new();
    OWNER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("grain-wasapi-owner".into())
                .spawn(move || {
                    // Backends are created here, never transferred across threads.
                    let mut players = Players::new();
                    while let Ok(job) = receiver.recv() {
                        // Also contain construction/destruction failures. Do
                        // not terminate the thread owning CPAL's cached STA.
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            job(&mut players);
                        }));
                    }
                })
                .ok()?;
            // The retained sender deliberately keeps this one parked thread
            // and CPAL's first COM apartment alive until process termination.
            Some(sender)
        })
        .clone()
}

pub struct AudioPlayer {
    id: u64,
    sender: Option<mpsc::Sender<Job>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let sender = owner().filter(|sender| {
            sender
                .send(Box::new(move |players| {
                    players.insert(id, Backend::new());
                }))
                .is_ok()
        });
        Self { id, sender }
    }

    fn request<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Backend) -> T + Send + 'static,
    ) -> Result<T, String> {
        let sender = self.sender.as_ref().ok_or("Audio owner unavailable")?;
        let id = self.id;
        super::owner_request::request(sender, RESPONSE_TIMEOUT, move |players| {
            super::owner_request::isolated_operation(players, id, operation)
        })?
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let path = path.to_path_buf();
        self.request(move |backend| backend.load(&path))?
    }

    pub fn play(&mut self) {
        let _ = self.request(Backend::play);
    }

    pub fn pause(&mut self) {
        let _ = self.request(Backend::pause);
    }

    pub fn position(&self) -> Option<Duration> {
        self.request(|backend| backend.position()).ok().flatten()
    }

    pub fn generation(&self) -> u64 {
        self.request(|backend| backend.generation()).unwrap_or(0)
    }

    pub fn restart(&mut self) {
        let _ = self.request(Backend::restart);
    }

    pub fn seek_frame(&mut self, frame: usize, fps: u32) {
        if fps == 0 {
            return;
        }
        let _ = self.request(move |backend| backend.seek_frame(frame, fps));
    }

    pub fn is_playing(&self) -> bool {
        self.request(|backend| backend.is_playing())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_survives_failed_operations_and_unrelated_jobs() {
        let first = AudioPlayer::new();
        let mut second = AudioPlayer::new();
        assert!(
            first
                .request::<()>(|_| panic!("injected operation failure"))
                .is_err()
        );
        assert!(first.request(|_| ()).is_err());
        second.load(Path::new("fixtures/demo.wav")).unwrap();
        second.seek_frame(1, 0);
        assert!(second.request(|_| ()).is_ok());

        let sender = owner().unwrap();
        sender
            .send(Box::new(|_| panic!("injected owner job failure")))
            .unwrap();
        // FIFO acknowledgment proves the SAME owner handles work after unwind.
        assert!(second.request(|_| ()).is_ok());
        let mut third = AudioPlayer::new();
        third.load(Path::new("fixtures/demo.wav")).unwrap();
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        if let Some(sender) = &self.sender {
            let id = self.id;
            let (reply, response) = mpsc::channel();
            if sender
                .send(Box::new(move |players| {
                    // Stream, sink and COM-backed handles are released here,
                    // not by the thread dropping the public proxy.
                    players.remove(&id);
                    let _ = reply.send(());
                }))
                .is_ok()
            {
                let _ = response.recv_timeout(RESPONSE_TIMEOUT);
            }
        }
    }
}
