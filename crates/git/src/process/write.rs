//! Concurrent pipe draining for local writes. Hooks/signing have no automatic deadline.
use std::{
    io::{self, Read, Write},
    process::{ChildStderr, ChildStdout, Command, ExitStatus, Output, Stdio},
};

const TAIL_LIMIT: usize = 64 * 1024;

#[derive(Debug)]
struct Tail {
    bytes: Vec<u8>,
    truncated: bool,
}

fn drain(mut pipe: impl Read) -> io::Result<Tail> {
    let mut tail = Tail {
        bytes: Vec::with_capacity(TAIL_LIMIT),
        truncated: false,
    };
    let mut chunk = [0; 8192];
    loop {
        let count = match pipe.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(tail);
        }
        let discard = (tail.bytes.len() + count).saturating_sub(TAIL_LIMIT);
        if discard != 0 {
            tail.bytes.drain(..discard);
            tail.truncated = true;
        }
        tail.bytes.extend_from_slice(&chunk[..count]);
    }
}

impl Tail {
    fn display(&self, label: &str) -> String {
        let prefix = if self.truncated {
            format!("[{label} earlier output truncated]\n")
        } else {
            String::new()
        };
        format!("{prefix}{}", String::from_utf8_lossy(&self.bytes))
    }
}

#[derive(Debug)]
struct Captured<T> {
    status: ExitStatus,
    stdout: T,
    stderr: T,
    input: io::Result<()>,
}

/// Final status and bounded display diagnostics, including explicit tail markers.
#[derive(Debug)]
pub(crate) struct WriteReport {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    stderr: String,
}

impl WriteReport {
    pub(crate) fn diagnostics(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn capture<T: Send + 'static>(
    mut command: Command,
    input: Option<&str>,
    stdout: impl FnOnce(ChildStdout) -> io::Result<T> + Send + 'static,
    stderr: impl FnOnce(ChildStderr) -> io::Result<T> + Send + 'static,
) -> io::Result<Captured<T>> {
    let mut child = super::spawn_with_stdin(
        &mut command,
        if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        },
    )?;
    let readers = super::Readers::start(&mut child, stdout, stderr)?;
    let input_result = match input {
        Some(message) => match child.take_stdin() {
            Some(mut stdin) => stdin.write_all(message.as_bytes()),
            None => {
                super::terminate(&mut child);
                let _ = readers.join();
                return Err(io::Error::other("Git stdin pipe was not created"));
            }
        },
        None => Ok(()),
    };
    // stdin closes before waiting; both workers are joined on every return path.
    let status = child.wait();
    if status.is_err() {
        super::terminate(&mut child);
    }
    let captured = readers.join();
    let status = status?;
    let (stdout, stderr) = captured?;
    Ok(Captured {
        status,
        stdout,
        stderr,
        input: input_result,
    })
}

fn report(command: Command, input: Option<&str>) -> io::Result<WriteReport> {
    let captured = capture(command, input, drain, drain)?;
    // Prefer Git's final failure diagnostics to a secondary broken stdin pipe.
    if captured.status.success() {
        captured.input?;
    }
    Ok(WriteReport {
        status: captured.status,
        stdout: captured.stdout.display("Git stdout"),
        stderr: captured.stderr.display("Git stderr"),
    })
}

/// Preserve status and diagnostic tails for stateful commands such as rebase.
/// No deadline/cancellation is imposed on hooks, signing or inherited pipes.
pub(crate) fn diagnostic_output(command: Command) -> io::Result<WriteReport> {
    report(command, None)
}

/// Start draining before feeding stdin; success retains stdout diagnostics only.
/// No deadline/cancellation is imposed on configured hooks or signing programs.
pub(crate) fn write_output(command: Command, input: Option<&str>) -> io::Result<String> {
    let report = report(command, input)?;
    if !report.status.success() {
        return Err(io::Error::other(format!(
            "Git write failed ({}): {}",
            report.status,
            report.diagnostics()
        )));
    }
    Ok(report.stdout)
}

#[derive(Debug)]
struct Complete {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn drain_complete(mut pipe: impl Read, limit: usize) -> io::Result<Complete> {
    let mut captured = Complete {
        bytes: Vec::with_capacity(limit.min(8192)),
        exceeded: false,
    };
    let mut chunk = [0; 8192];
    loop {
        let count = match pipe.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(captured);
        }
        let keep = limit.saturating_sub(captured.bytes.len()).min(count);
        captured.bytes.extend_from_slice(&chunk[..keep]);
        captured.exceeded |= keep < count;
        // Discard excess while the object-writing command finishes; never parse a prefix.
    }
}

/// Complete raw protocol output from an object-writing command. On overflow,
/// drain/reap/join and fail without parsing partial data or interrupting drivers.
/// This is an output bound, not a process deadline or a rollback guarantee.
pub(crate) fn complete_write_output(
    command: Command,
    stdout: usize,
    stderr: usize,
) -> io::Result<Output> {
    let captured = capture(
        command,
        None,
        move |pipe| drain_complete(pipe, stdout),
        move |pipe| drain_complete(pipe, stderr),
    )?;
    if captured.stdout.exceeded || captured.stderr.exceeded {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git structured write exceeded its output limit",
        ));
    }
    Ok(Output {
        status: captured.status,
        stdout: captured.stdout.bytes,
        stderr: captured.stderr.bytes,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(body: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", body]);
        command
    }

    #[test]
    fn complete_protocol_keeps_raw_bytes_exact_limits_and_conflict_status() {
        let output =
            complete_write_output(shell("printf 'a\\000b'; printf 'err' >&2; exit 1"), 3, 3)
                .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, b"a\0b");
        assert_eq!(output.stderr, b"err");
    }

    #[test]
    fn complete_overflow_drains_and_reaps_before_returning_or_retrying() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "desktop-structured-write-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        for pipe in ["stdout", "stderr"] {
            let marker = fixture.0.join(pipe);
            let body = if pipe == "stdout" {
                "head -c 100000 /dev/zero; printf 'finished' > \"$1\"; exit 7"
            } else {
                "head -c 100000 /dev/zero >&2; printf 'finished' > \"$1\"; exit 0"
            };
            let mut command = shell(body);
            command.arg("fixture").arg(&marker);
            assert_eq!(
                complete_write_output(command, 8, 8).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(std::fs::read(&marker).unwrap(), b"finished");
        }
        let retry = complete_write_output(shell("printf 'valid'"), 8, 8).unwrap();
        assert!(retry.status.success());
        assert_eq!(retry.stdout, b"valid");
    }

    #[test]
    fn diagnostic_report_keeps_final_status_and_both_marked_tails() {
        let report = diagnostic_output(shell("head -c 100000 /dev/zero; printf 'stdout tail'; head -c 100000 /dev/zero >&2; printf 'stderr tail' >&2; exit 7")).unwrap();
        assert_eq!(report.status.code(), Some(7));
        let text = report.diagnostics();
        assert!(text.contains("Git stdout earlier output truncated"));
        assert!(text.contains("Git stderr earlier output truncated"));
        assert!(text.contains("stdout tail"));
        assert!(text.ends_with("stderr tail"));
        assert!(text.len() < 2 * TAIL_LIMIT + 200);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn large_input_and_simultaneous_output_complete_without_pipe_deadlock() {
        // Coreutils timeout makes a regression fail instead of hanging the test process.
        let mut command = Command::new("timeout");
        command.args(["5", "sh", "-c", "head -c 2097152 /dev/zero; head -c 2097152 /dev/zero >&2; count=$(wc -c); printf 'input=%s' \"$count\""]);
        let input = "message\n".repeat(400_000);
        let result = write_output(command, Some(&input)).unwrap();
        assert!(result.starts_with("[Git stdout earlier output truncated]\n"));
        assert!(result.ends_with(&format!("input={}", input.len())));
        assert!(result.len() < TAIL_LIMIT + 100);
    }

    #[test]
    fn failed_child_preserves_both_tails_and_overrides_broken_input_pipe() {
        let input = "message".repeat(400_000);
        let error = write_output(shell("exec 0<&-; head -c 100000 /dev/zero; printf 'stdout tail'; head -c 100000 /dev/zero >&2; printf 'hook rejected commit' >&2; exit 7"), Some(&input)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exit status: 7"));
        assert!(message.contains("[Git stdout earlier output truncated]"));
        assert!(message.contains("stdout tail"));
        assert!(message.contains("[Git stderr earlier output truncated]"));
        assert!(message.ends_with("hook rejected commit"));
        assert!(message.len() < 2 * TAIL_LIMIT + 200);
    }

    #[test]
    fn successful_child_cannot_hide_incomplete_stdin() {
        let input = "message".repeat(400_000);
        let error = write_output(shell("exec 0<&-; exit 0"), Some(&input)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn absent_input_is_eof_and_success_keeps_stdout_contract() {
        let result = write_output(shell("cat; printf 'ok'; printf 'warning' >&2"), None).unwrap();
        assert_eq!(result, "ok");
        let result = write_output(shell("wc -c"), Some("")).unwrap();
        assert_eq!(result.trim(), "0");
    }

    #[test]
    fn exact_tail_boundary_preserves_raw_bytes_without_a_truncation_marker() {
        let tail = drain(io::repeat(255).take(TAIL_LIMIT as u64)).unwrap();
        assert_eq!(tail.bytes, vec![255; TAIL_LIMIT]);
        assert!(!tail.truncated);
        let tail = drain(io::repeat(255).take((TAIL_LIMIT + 1) as u64)).unwrap();
        assert_eq!(tail.bytes, vec![255; TAIL_LIMIT]);
        assert!(tail.truncated);
    }
}
