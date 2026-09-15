//! Bounded provider process execution; no reader threads can outlive a request.
use super::control::GenerationControl;
use std::process::{Command, Output};

#[cfg(not(any(unix, windows)))]
pub fn run(_command: &mut Command, _control: &GenerationControl) -> Result<Output, String> {
    Err("Bounded CLI providers currently require macOS or Linux".into())
}

#[cfg(unix)]
pub fn run(command: &mut Command, control: &GenerationControl) -> Result<Output, String> {
    use std::io::Read;
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    use std::process::{Child, Stdio};
    use std::time::Duration;

    struct OwnedGroup(Child);
    impl Drop for OwnedGroup {
        fn drop(&mut self) {
            // The command is spawned into its own process group. Kill descendants
            // as well as the immediate child, including after a successful exit.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn nonblocking(fd: std::os::fd::RawFd) -> Result<(), String> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }

    fn drain(reader: &mut impl Read, bytes: &mut Vec<u8>) -> Result<bool, String> {
        let mut chunk = [0u8; 16_384];
        match reader.read(&mut chunk) {
            Ok(0) => Ok(true),
            Ok(count) => {
                if bytes.len() + count > 1_048_576 {
                    return Err("Provider output exceeds 1 MiB per stream".into());
                }
                bytes.extend_from_slice(&chunk[..count]);
                Ok(false)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error.to_string()),
        }
    }

    control.check()?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = OwnedGroup(command.spawn().map_err(|error| error.to_string())?);
    let mut stdout = child.0.stdout.take().ok_or("Missing provider stdout")?;
    let mut stderr = child.0.stderr.take().ok_or("Missing provider stderr")?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let (mut out_eof, mut err_eof) = (false, false);
    let mut status = None;
    loop {
        control.check()?;
        if !out_eof {
            out_eof = drain(&mut stdout, &mut out)?;
        }
        if !err_eof {
            err_eof = drain(&mut stderr, &mut err)?;
        }
        if status.is_none() {
            status = observe_exit(&child.0).map_err(|error| error.to_string())?;
        }
        if out_eof
            && err_eof
            && let Some(status) = status
        {
            return Ok(Output {
                status,
                stdout: out,
                stderr: err,
            });
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn captures_both_streams_and_exit_status() {
        let result = run(
            &mut shell("printf source; printf diagnostic >&2; exit 7"),
            &GenerationControl::default(),
        )
        .unwrap();
        assert_eq!(result.stdout, b"source");
        assert_eq!(result.stderr, b"diagnostic");
        assert_eq!(result.status.code(), Some(7));
    }

    #[test]
    fn deadline_and_cancellation_interrupt_slow_providers() {
        let start = Instant::now();
        let result = run(
            &mut shell("sleep 30 & wait"),
            &GenerationControl::new(Duration::from_millis(100)),
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(2));
        let control = GenerationControl::default();
        let cancel = control.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            cancel.cancel();
        });
        let result = run(&mut shell("sleep 30 & wait"), &control);
        thread.join().unwrap();
        assert!(result.unwrap_err().contains("cancelled"));
    }

    #[test]
    fn excessive_output_and_precancelled_requests_are_rejected() {
        let result = run(
            &mut shell("while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done"),
            &GenerationControl::default(),
        );
        assert!(result.unwrap_err().contains("1 MiB"));
        let control = GenerationControl::default();
        control.cancel();
        assert!(
            run(&mut Command::new("/does/not/exist"), &control)
                .unwrap_err()
                .contains("cancelled")
        );
    }
}

#[cfg(windows)]
#[path = "windows.rs"]
mod windows;
#[cfg(windows)]
pub use windows::run;

#[cfg(unix)]
#[path = "unix_exit.rs"]
mod unix_exit;
#[cfg(unix)]
use unix_exit::observe_exit;
