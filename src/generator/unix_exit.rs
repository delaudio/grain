//! Observe termination without releasing the process ID before group cleanup.
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};

pub(super) fn observe_exit(child: &Child) -> io::Result<Option<ExitStatus>> {
    // SAFETY: waitid initializes this valid, zeroed siginfo buffer. WNOWAIT
    // retains the zombie and its PID until OwnedGroup terminates the group and
    // calls Child::wait. WNOHANG keeps the generation control loop responsive.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id() as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::Interrupted {
            Ok(None)
        } else {
            Err(error)
        };
    }
    // SAFETY: successful waitid supplies the child fields, or leaves the
    // zero-initialized PID untouched when no exit is ready.
    if unsafe { info.si_pid() } == 0 {
        return Ok(None);
    }
    let status = unsafe { info.si_status() };
    let raw = match info.si_code {
        libc::CLD_EXITED => status << 8,
        libc::CLD_KILLED => status,
        libc::CLD_DUMPED => status | 0x80,
        _ => return Ok(None),
    };
    Ok(Some(ExitStatus::from_raw(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn observation_preserves_waitable_child_and_exit_status() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let observed = loop {
            if let Some(status) = observe_exit(&child).unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child did not exit");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        // A second successful observation proves the first did not reap it.
        let repeated = observe_exit(&child);
        let reaped = child.wait().unwrap();
        assert_eq!(observed.code(), Some(7));
        assert_eq!(repeated.unwrap(), Some(observed));
        assert_eq!(reaped, observed);
    }
}
