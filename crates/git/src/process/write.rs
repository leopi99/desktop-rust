//! Concurrent pipe draining for local writes. Hooks/signing have no automatic deadline.
use std::{
    io::{self, Read, Write},
    process::{Command, Stdio},
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

/// Drain both streams before feeding stdin, then reap and join before returning.
/// Success retains stdout diagnostics only, matching the local-write API contract.
/// No deadline/cancellation is imposed on configured hooks or signing programs.
pub(crate) fn write_output(mut command: Command, input: Option<&str>) -> io::Result<String> {
    let mut child = super::spawn_with_stdin(
        &mut command,
        if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        },
    )?;
    let readers = super::Readers::start(&mut child, drain, drain)?;
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
    // stdin is closed before waiting, including after a failed/partial write.
    let status = child.wait();
    if status.is_err() {
        super::terminate(&mut child);
    }
    let captured = readers.join();
    let status = status?;
    let (stdout, stderr) = captured?;
    if !status.success() {
        // Prefer Git's final diagnostics to a secondary broken stdin pipe.
        return Err(io::Error::other(format!(
            "Git write failed ({status}): {}{}",
            stdout.display("Git stdout"),
            stderr.display("Git stderr")
        )));
    }
    input_result?;
    Ok(stdout.display("Git stdout"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(body: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", body]);
        command
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
