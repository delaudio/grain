use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// A backend that unwinds may be inconsistent. Remove it before attempting
/// destruction, and contain destructor unwinding without touching other players.
pub(super) fn isolated_operation<S, T>(
    players: &mut std::collections::HashMap<u64, S>,
    id: u64,
    operation: impl FnOnce(&mut S) -> T,
) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        players
            .get_mut(&id)
            .ok_or_else(|| "Audio player unavailable".to_string())
            .map(operation)
    })) {
        Ok(result) => result,
        Err(_) => {
            let retired = players.remove(&id);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(retired)));
            Err("Audio operation panicked; affected player retired".into())
        }
    }
}

pub(super) type Job<S> = Box<dyn FnOnce(&mut S) + Send>;

pub(super) fn request<S: 'static, T: Send + 'static>(
    sender: &mpsc::Sender<Job<S>>,
    timeout: Duration,
    operation: impl FnOnce(&mut S) -> T + Send + 'static,
) -> Result<T, String> {
    const QUEUED: u8 = 0;
    const RUNNING: u8 = 1;
    const CANCELLED: u8 = 2;
    let phase = Arc::new(AtomicU8::new(QUEUED));
    let owner_phase = phase.clone();
    let deadline = Instant::now() + timeout;
    let (reply, response) = mpsc::channel();
    sender
        .send(Box::new(move |state| {
            if Instant::now() >= deadline
                || owner_phase
                    .compare_exchange(QUEUED, RUNNING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
            {
                let _ = reply.send(Err("Audio request expired before execution".into()));
                return;
            }
            let _ = reply.send(Ok(operation(state)));
        }))
        .map_err(|_| "Audio owner stopped".to_string())?;
    match response.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Audio owner stopped".into()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if phase
                .compare_exchange(QUEUED, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Err("Audio request expired before execution".into());
            }
            // Native operations cannot safely be interrupted midway. Once
            // claimed, preserve the synchronous backend contract: do not
            // return an error while a mutation can still complete later.
            response
                .recv()
                .map_err(|_| "Audio owner stopped".to_string())?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_panic_retires_only_the_affected_backend() {
        let mut players = std::collections::HashMap::from([(1, 0_u32), (2, 10)]);
        let result = isolated_operation(&mut players, 1, |value| {
            *value = 99;
            panic!("backend failure");
        });
        assert!(result.is_err());
        assert!(!players.contains_key(&1));
        assert_eq!(
            isolated_operation(&mut players, 2, |value| {
                *value += 1;
                *value
            }),
            Ok(11)
        );
        players.insert(3, 20);
        assert_eq!(isolated_operation(&mut players, 3, |value| *value), Ok(20));
    }

    #[test]
    fn retirement_also_contains_a_destructor_panic() {
        struct Broken;
        impl Drop for Broken {
            fn drop(&mut self) {
                panic!("destructor failure");
            }
        }
        let mut players = std::collections::HashMap::from([(1, Broken)]);
        let result = isolated_operation(&mut players, 1, |_| panic!("operation failure"));
        assert!(result.is_err());
        assert!(players.is_empty());
    }

    #[test]
    fn expired_queued_work_cannot_mutate_the_backend() {
        let (sender, receiver) = mpsc::channel();
        let result = request(&sender, Duration::from_millis(10), |value: &mut u32| {
            *value = 7
        });
        assert!(result.is_err());
        let mut value = 0;
        receiver.recv().unwrap()(&mut value);
        assert_eq!(value, 0);
    }

    #[test]
    fn started_work_returns_its_actual_outcome_not_a_false_timeout() {
        let (sender, receiver) = mpsc::channel::<Job<u32>>();
        let (started, start) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (result, outcome) = mpsc::channel();
        let owner = std::thread::spawn(move || {
            let mut value = 0;
            receiver.recv().unwrap()(&mut value);
            value
        });
        let caller = std::thread::spawn(move || {
            let value = request(&sender, Duration::from_millis(100), move |value| {
                started.send(()).unwrap();
                released.recv().unwrap();
                *value = 7;
                *value
            });
            result.send(value).unwrap();
        });
        start.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            outcome.recv_timeout(Duration::from_millis(200)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        release.send(()).unwrap();
        assert_eq!(
            outcome
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap(),
            7
        );
        caller.join().unwrap();
        assert_eq!(owner.join().unwrap(), 7);
    }
}
