//! Windows CLI ownership: suspend, assign to a kill-on-close job, then resume.
use super::super::control::GenerationControl;
use std::io::Read;
#[cfg(test)]
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::ptr::{null, null_mut};
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

struct Handle(HANDLE);

impl Handle {
    fn checked(raw: HANDLE) -> Result<Self, String> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this uniquely owned handle was checked when constructed.
        unsafe { CloseHandle(self.0) };
    }
}

struct OwnedJob {
    child: Child,
    job: Handle,
}

impl Drop for OwnedJob {
    fn drop(&mut self) {
        // SAFETY: the job remains open throughout cleanup. Also kill the direct
        // child in case assigning it to the job failed while it was suspended.
        unsafe { TerminateJobObject(self.job.0, 1) };
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn resume(pid: u32, control: &GenerationControl) -> Result<(), String> {
    // SAFETY: ToolHelp returns an owned snapshot; THREADENTRY32 has its required
    // size initialized, and the process cannot create threads while suspended.
    let snapshot = Handle::checked(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut found = unsafe { Thread32First(snapshot.0, &mut entry) };
    while found != 0 {
        control.check()?;
        if entry.th32OwnerProcessID == pid {
            let thread = Handle::checked(unsafe {
                OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID)
            })?;
            if unsafe { ResumeThread(thread.0) } == u32::MAX {
                return Err(std::io::Error::last_os_error().to_string());
            }
            return Ok(());
        }
        found = unsafe { Thread32Next(snapshot.0, &mut entry) };
    }
    Err("Cannot locate the suspended provider thread".into())
}

fn drain(pipe: &mut (impl Read + AsRawHandle), buffer: &mut Vec<u8>) -> Result<bool, String> {
    let mut available = 0;
    // SAFETY: the pipe is live and exclusively read here. Peek does not consume
    // bytes; reading no more than the available count avoids a blocking reader.
    let ok = unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            &mut available,
            null_mut(),
        )
    };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
            Ok(true)
        } else {
            Err(error.to_string())
        };
    }
    if available == 0 {
        return Ok(false);
    }
    let mut bytes = [0; 16_384];
    let count = (available as usize).min(bytes.len());
    let count = pipe.read(&mut bytes[..count]).map_err(|e| e.to_string())?;
    if buffer.len() + count > 1_048_576 {
        return Err("Provider output exceeds 1 MiB per stream".into());
    }
    buffer.extend_from_slice(&bytes[..count]);
    Ok(count == 0)
}

pub fn run(command: &mut Command, control: &GenerationControl) -> Result<Output, String> {
    control.check()?;
    // SAFETY: null attributes create a non-inheritable unnamed job. Zeroed
    // limits are valid; only the documented kill-on-close flag is enabled.
    let job = Handle::checked(unsafe { CreateJobObjectW(null(), null()) })?;
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_SUSPENDED);
    let child = command.spawn().map_err(|e| e.to_string())?;
    let mut owned = OwnedJob { child, job };
    // SAFETY: both handles are owned and live. No provider instructions have
    // executed yet, so descendants cannot escape the assignment window.
    if unsafe { AssignProcessToJobObject(owned.job.0, owned.child.as_raw_handle()) } == 0 {
        return Err(format!(
            "Cannot contain provider process: {}",
            std::io::Error::last_os_error()
        ));
    }
    control.check()?;
    resume(owned.child.id(), control)?;
    let mut stdout = owned.child.stdout.take().ok_or("Missing provider stdout")?;
    let mut stderr = owned.child.stderr.take().ok_or("Missing provider stderr")?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let (mut out_done, mut err_done) = (false, false);
    let mut status = None;
    loop {
        control.check()?;
        if !out_done {
            out_done = drain(&mut stdout, &mut out)?;
        }
        if !err_done {
            err_done = drain(&mut stderr, &mut err)?;
        }
        if status.is_none() {
            status = owned.child.try_wait().map_err(|e| e.to_string())?;
        }
        if out_done
            && err_done
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    fn helper_command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "generator::process::windows::tests::helper",
            "--nocapture",
        ]);
        command.env("GRAIN_WINDOWS_PROCESS_HELPER", mode);
        command
    }

    #[test]
    fn helper() {
        let Ok(mode) = std::env::var("GRAIN_WINDOWS_PROCESS_HELPER") else {
            return;
        };
        match mode.as_str() {
            "output" => {
                println!("provider-source");
                eprintln!("provider-diagnostic");
                std::process::exit(7);
            }
            "overflow" => loop {
                std::io::stdout().write_all(&[b'x'; 16_384]).unwrap();
            },
            "descendant" => {
                let mut child = helper_command("sleep").spawn().unwrap();
                std::fs::write(
                    std::env::var_os("GRAIN_HELPER_PID_FILE").unwrap(),
                    child.id().to_string(),
                )
                .unwrap();
                let _ = child.wait();
            }
            _ => std::thread::sleep(Duration::from_secs(60)),
        }
    }

    #[test]
    fn captures_output_and_failure_status() {
        let output = run(&mut helper_command("output"), &GenerationControl::default()).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stdout).contains("provider-source"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("provider-diagnostic"));
    }

    #[test]
    fn bounds_output_and_checks_before_spawn() {
        let error = run(
            &mut helper_command("overflow"),
            &GenerationControl::default(),
        )
        .unwrap_err();
        assert!(error.contains("1 MiB"), "{error}");
        let control = GenerationControl::default();
        control.cancel();
        assert!(
            run(&mut Command::new("grain-nonexistent-provider"), &control)
                .unwrap_err()
                .contains("cancelled")
        );
    }

    #[test]
    fn deadline_is_bounded() {
        let start = Instant::now();
        let error = run(
            &mut helper_command("sleep"),
            &GenerationControl::new(Duration::from_millis(100)),
        )
        .unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancellation_terminates_descendants() {
        let path = std::env::temp_dir().join(format!(
            "grain-child-{}-{}.pid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let control = GenerationControl::default();
        let cancel = control.clone();
        let watched = path.clone();
        let observer = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let process = loop {
                if let Ok(text) = std::fs::read_to_string(&watched)
                    && let Ok(pid) = text.parse::<u32>()
                {
                    // Open before cancellation to retain the exact process
                    // identity even if its PID is subsequently recycled.
                    break Handle::checked(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) })
                        .unwrap();
                }
                if Instant::now() >= deadline {
                    cancel.cancel();
                    panic!("descendant did not start");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            cancel.cancel();
            assert_eq!(unsafe { WaitForSingleObject(process.0, 5_000) }, 0);
        });
        let error = run(
            helper_command("descendant").env("GRAIN_HELPER_PID_FILE", &path),
            &control,
        )
        .unwrap_err();
        observer.join().unwrap();
        let _ = std::fs::remove_file(path);
        assert!(error.contains("cancelled"), "{error}");
    }
}
