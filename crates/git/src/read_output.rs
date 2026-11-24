//! Bounded read-only Git output with distinct complete-record and display policies.
#[cfg(unix)]
use std::time::Duration;
use std::{ffi::OsStr, io, path::Path, process::Output};

#[cfg(unix)]
use crate::process::{bounded_output, prefix_output, OutputLimits, PrefixOutput};

fn command<S: AsRef<OsStr>>(path: &Path, args: &[S]) -> std::process::Command {
    let mut command = crate::git_command(path);
    command.arg("--no-optional-locks").args(args);
    command
}

#[cfg(unix)]
fn limits(stdout: usize) -> OutputLimits {
    OutputLimits {
        timeout: Duration::from_secs(30),
        stdout,
        stderr: 16 * 1024,
    }
}

fn successful(output: Output) -> io::Result<Vec<u8>> {
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Git read failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

fn complete_output<S: AsRef<OsStr>>(path: &Path, args: &[S], limit: usize) -> io::Result<Output> {
    #[cfg(unix)]
    {
        bounded_output(command(path, args), limits(limit))
    }
    #[cfg(not(unix))]
    {
        // Post-buffer checks only until native tree/pipe cancellation (PLAT-01).
        let output = command(path, args).output()?;
        if output.stdout.len() > limit || output.stderr.len() > 16 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git metadata exceeded its output limit",
            ));
        }
        Ok(output)
    }
}

pub(super) fn complete<S: AsRef<OsStr>>(
    path: &Path,
    args: &[S],
    limit: usize,
) -> io::Result<Vec<u8>> {
    successful(complete_output(path, args, limit)?)
}

fn optional_output(output: Output) -> io::Result<Option<Vec<u8>>> {
    // Only a quiet, empty exit 1 denotes absence for the selected Git commands.
    if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        Ok(None)
    } else {
        successful(output).map(Some)
    }
}

/// For Git commands whose documented quiet exit 1 specifically denotes absence.
/// Capture errors, signals, other statuses and diagnostics are never absence.
pub(super) fn optional<S: AsRef<OsStr>>(
    path: &Path,
    args: &[S],
    limit: usize,
) -> io::Result<Option<Vec<u8>>> {
    optional_output(complete_output(path, args, limit)?)
}

fn display(bytes: &[u8], truncated: bool, label: &str) -> String {
    let mut result = String::from_utf8_lossy(bytes).into_owned();
    if truncated {
        result.push_str(&format!("\n[{label} preview truncated]"));
    }
    result
}

#[cfg(unix)]
fn display_output(captured: PrefixOutput, label: &str) -> io::Result<String> {
    let bytes = successful(captured.output)?;
    Ok(display(&bytes, captured.stdout_truncated, label))
}

pub(super) fn preview<S: AsRef<OsStr>>(
    path: &Path,
    args: &[S],
    limit: usize,
    label: &str,
) -> io::Result<String> {
    #[cfg(unix)]
    {
        display_output(prefix_output(command(path, args), limits(limit))?, label)
    }
    #[cfg(not(unix))]
    {
        let bytes = successful(command(path, args).output()?)?;
        Ok(display(
            &bytes[..bytes.len().min(limit)],
            bytes.len() > limit,
            label,
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn failure_after_a_large_prefix_is_not_reported_as_a_valid_preview() {
        let mut command = std::process::Command::new("sh");
        command.args([
            "-c",
            "printf 'prefix'; head -c 100000 /dev/zero; printf 'late failure' >&2; exit 7",
        ]);
        let captured = prefix_output(command, limits(512)).unwrap();
        assert!(captured.stdout_truncated);
        let error = display_output(captured, "Patch").unwrap_err();
        assert!(error.to_string().contains("late failure"));
    }

    #[test]
    fn optional_reads_require_an_empty_quiet_exit_one() {
        for (body, absent) in [
            ("exit 1", true),
            ("exit 128", false),
            ("printf 'failure' >&2; exit 1", false),
            ("printf 'partial'; exit 1", false),
            ("kill -TERM $$", false),
        ] {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", body]);
            let output = bounded_output(command, limits(512)).unwrap();
            let result = optional_output(output);
            if absent {
                assert!(result.unwrap().is_none());
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn invalid_utf8_at_a_clipped_boundary_is_only_lossy_display_data() {
        assert_eq!(
            display(b"A\xc3", true, "Message"),
            "A\u{fffd}\n[Message preview truncated]"
        );
    }
}
