//! Fail-closed bounded capture for short read-only Unix commands.
use std::{
    io::{self, Read},
    process::{Command, Output},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct OutputLimits {
    pub(crate) timeout: Duration,
    pub(crate) stdout: usize,
    pub(crate) stderr: usize,
}

#[derive(Debug)]
struct Capture {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn drain(mut pipe: impl Read, limit: usize, overflow: &AtomicBool) -> io::Result<Capture> {
    let mut capture = Capture {
        bytes: Vec::with_capacity(limit.min(4096)),
        exceeded: false,
    };
    let mut chunk = [0; 4096];
    loop {
        let count = match pipe.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(capture);
        }
        let keep = limit.saturating_sub(capture.bytes.len()).min(count);
        capture.bytes.extend_from_slice(&chunk[..keep]);
        if keep < count {
            capture.exceeded = true;
            overflow.store(true, Ordering::Release);
        }
        // Keep draining until the controller stops the child; never grow the capture.
    }
}

fn limit_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Git command exceeded its output limit",
    )
}

/// Capture raw output, failing rather than accepting a truncated successful report.
/// Deadline monitoring continues after direct-child exit while inherited pipes remain open.
pub(crate) fn bounded_output(mut command: Command, limits: OutputLimits) -> io::Result<Output> {
    let started = Instant::now();
    let mut child = super::spawn(&mut command)?;
    let Some(stdout) = child.take_stdout() else {
        super::terminate(&mut child);
        return Err(io::Error::other("Git stdout pipe was not created"));
    };
    let Some(stderr) = child.take_stderr() else {
        super::terminate(&mut child);
        return Err(io::Error::other("Git stderr pipe was not created"));
    };
    let overflow = Arc::new(AtomicBool::new(false));
    let out_overflow = overflow.clone();
    let err_overflow = overflow.clone();
    let out = match thread::Builder::new()
        .name("git-probe-stdout".into())
        .spawn(move || drain(stdout, limits.stdout, &out_overflow))
    {
        Ok(reader) => reader,
        Err(error) => {
            super::terminate(&mut child);
            return Err(error);
        }
    };
    let err = match thread::Builder::new()
        .name("git-probe-stderr".into())
        .spawn(move || drain(stderr, limits.stderr, &err_overflow))
    {
        Ok(reader) => reader,
        Err(error) => {
            super::terminate(&mut child);
            let _ = out.join();
            return Err(error);
        }
    };
    let mut status = None;
    let outcome = loop {
        if overflow.load(Ordering::Acquire) {
            super::terminate(&mut child);
            break Err(limit_error());
        }
        if started.elapsed() >= limits.timeout {
            super::terminate(&mut child);
            break Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Git command exceeded its deadline",
            ));
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(error) => {
                    super::terminate(&mut child);
                    break Err(error);
                }
            }
        }
        if let Some(status) = status {
            if out.is_finished() && err.is_finished() {
                break Ok(status);
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Reap/drain before returning, so retry does not leave pipe readers behind.
    let stdout = out
        .join()
        .map_err(|_| io::Error::other("Git stdout reader failed"));
    let stderr = err
        .join()
        .map_err(|_| io::Error::other("Git stderr reader failed"));
    let status = outcome?;
    let stdout = stdout??;
    let stderr = stderr??;
    // Catch overflow even when a short-lived command exits between controller polls.
    if stdout.exceeded || stderr.exceeded {
        return Err(limit_error());
    }
    Ok(Output {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(body: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", body]);
        command
    }

    fn limits() -> OutputLimits {
        OutputLimits {
            timeout: Duration::from_secs(2),
            stdout: 512,
            stderr: 1024,
        }
    }

    #[test]
    fn exact_limits_and_raw_bytes_are_preserved() {
        let result = bounded_output(
            shell("head -c 512 /dev/zero; head -c 1024 /dev/zero >&2"),
            limits(),
        )
        .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, vec![0; 512]);
        assert_eq!(result.stderr, vec![0; 1024]);
    }

    #[test]
    fn exited_commands_cannot_hide_overflow_between_polls() {
        for body in ["head -c 513 /dev/zero", "head -c 1025 /dev/zero >&2"] {
            assert_eq!(
                bounded_output(shell(body), limits()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn streaming_stdout_and_stderr_are_stopped_before_the_deadline() {
        for body in [
            "while :; do printf '0123456789'; done",
            "while :; do printf '0123456789' >&2; done",
        ] {
            let started = Instant::now();
            assert_eq!(
                bounded_output(shell(body), limits()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert!(started.elapsed() < Duration::from_secs(5));
        }
    }

    #[test]
    fn drain_does_not_retain_a_large_input_stream() {
        let overflow = AtomicBool::new(false);
        let captured = drain(io::repeat(42).take(16 * 1024 * 1024), 512, &overflow).unwrap();
        assert_eq!(captured.bytes, vec![42; 512]);
        assert!(captured.exceeded);
        assert!(overflow.load(Ordering::Acquire));
    }

    #[test]
    fn stalled_child_and_inherited_pipes_are_terminated_on_timeout() {
        for body in ["sleep 30", "sleep 30 & exit 0"] {
            let mut policy = limits();
            policy.timeout = Duration::from_millis(100);
            let started = Instant::now();
            assert_eq!(
                bounded_output(shell(body), policy).unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
            assert!(started.elapsed() < Duration::from_secs(5));
        }
    }

    #[test]
    fn nonzero_exit_retains_status_and_error_output() {
        let result = bounded_output(shell("printf 'failure' >&2; exit 7"), limits()).unwrap();
        assert_eq!(result.status.code(), Some(7));
        assert_eq!(result.stderr, b"failure");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn timed_out_parent_is_reaped_and_its_helper_is_not_running() {
        use std::sync::atomic::AtomicUsize;
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = std::env::temp_dir().join(format!(
            "desktop-rust-probe-cleanup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let fixture = Fixture(directory);
        let mut command = shell("printf '%s' \"$$\" > \"$1/parent\"; sleep 30 & printf '%s' \"$!\" > \"$1/helper\"; wait");
        command.arg("fixture").arg(&fixture.0);
        let mut policy = limits();
        policy.timeout = Duration::from_secs(1);
        assert_eq!(
            bounded_output(command, policy).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        for name in ["parent", "helper"] {
            let pid: u32 = std::fs::read_to_string(fixture.0.join(name))
                .unwrap()
                .parse()
                .unwrap();
            match std::fs::read_to_string(format!("/proc/{pid}/status")) {
                Ok(status) => assert!(
                    name == "helper"
                        && status
                            .lines()
                            .any(|line| line.starts_with("State:") && line.contains('Z')),
                    "{name} still running: {status}"
                ),
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::NotFound),
            }
        }
    }
}
