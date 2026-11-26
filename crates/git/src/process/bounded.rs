//! Bounded complete/prefix capture for read-only Unix commands.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverflowPolicy {
    Reject,
    Prefix,
}

/// Raw child output plus an explicit indication of a clipped stdout prefix.
#[derive(Debug)]
pub(crate) struct PrefixOutput {
    pub(crate) output: Output,
    pub(crate) stdout_truncated: bool,
}

fn drain(
    mut pipe: impl Read,
    limit: usize,
    overflow: &AtomicBool,
    policy: OverflowPolicy,
) -> io::Result<Capture> {
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
            if policy == OverflowPolicy::Reject {
                overflow.store(true, Ordering::Release);
            }
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
pub(crate) fn bounded_output(command: Command, limits: OutputLimits) -> io::Result<Output> {
    capture_output(command, limits, OverflowPolicy::Reject).map(|capture| capture.output)
}

/// Retain a bounded stdout prefix but drain the rest through EOF. Nonzero exit
/// status remains visible to the caller; stderr overflow and deadline still fail.
pub(crate) fn prefix_output(command: Command, limits: OutputLimits) -> io::Result<PrefixOutput> {
    capture_output(command, limits, OverflowPolicy::Prefix)
}

fn capture_output(
    mut command: Command,
    limits: OutputLimits,
    policy: OverflowPolicy,
) -> io::Result<PrefixOutput> {
    let started = Instant::now();
    let mut child = super::spawn(&mut command)?;
    let overflow = Arc::new(AtomicBool::new(false));
    let out_overflow = overflow.clone();
    let err_overflow = overflow.clone();
    let readers = super::Readers::start(
        &mut child,
        move |stdout| drain(stdout, limits.stdout, &out_overflow, policy),
        move |stderr| drain(stderr, limits.stderr, &err_overflow, OverflowPolicy::Reject),
    )?;
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
            if readers.is_finished() {
                break Ok(status);
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Reap/drain before returning, so retry does not leave pipe readers behind.
    let captured = readers.join();
    let status = outcome?;
    let (stdout, stderr) = captured?;
    // Catch overflow even when a short-lived command exits between controller polls.
    if (stdout.exceeded && policy == OverflowPolicy::Reject) || stderr.exceeded {
        return Err(limit_error());
    }
    Ok(PrefixOutput {
        stdout_truncated: stdout.exceeded,
        output: Output {
            status,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
        },
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
        let captured = drain(
            io::repeat(42).take(16 * 1024 * 1024),
            512,
            &overflow,
            OverflowPolicy::Reject,
        )
        .unwrap();
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
    #[test]
    fn prefix_capture_retains_only_the_prefix_and_observes_the_final_exit() {
        for exit in ["0", "7"] {
            let mut command = Command::new("sh");
            command.args([
                "-c",
                "head -c 100000 /dev/zero; printf 'finished' >&2; exit \"$1\"",
                "fixture",
                exit,
            ]);
            let captured = prefix_output(command, limits()).unwrap();
            assert_eq!(captured.output.stdout, vec![0; 512]);
            assert!(captured.stdout_truncated);
            assert_eq!(captured.output.status.code(), Some(exit.parse().unwrap()));
            assert_eq!(captured.output.stderr, b"finished");
        }
    }

    #[test]
    fn prefix_capture_marks_exact_boundary_without_false_truncation() {
        let captured = prefix_output(shell("head -c 512 /dev/zero"), limits()).unwrap();
        assert_eq!(captured.output.stdout.len(), 512);
        assert!(!captured.stdout_truncated);
    }

    #[test]
    fn prefix_capture_still_rejects_stderr_overflow_and_stalls() {
        assert_eq!(
            prefix_output(
                shell("head -c 100000 /dev/zero; head -c 1025 /dev/zero >&2"),
                limits()
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidData
        );
        let mut policy = limits();
        policy.timeout = Duration::from_millis(100);
        assert_eq!(
            prefix_output(shell("head -c 10000 /dev/zero; sleep 30"), policy)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }
}
