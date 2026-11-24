//! Bounded read-only Git output with distinct complete-record and display policies.
use std::{ffi::OsStr, io, path::Path};
#[cfg(unix)]
use std::{process::Output, time::Duration};

#[cfg(unix)]
use crate::process::{bounded_output, prefix_output, OutputLimits, PrefixOutput};

#[cfg(unix)]
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

#[cfg(unix)]
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

pub(super) fn complete<S: AsRef<OsStr>>(
    path: &Path,
    args: &[S],
    limit: usize,
) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        successful(bounded_output(command(path, args), limits(limit))?)
    }
    #[cfg(not(unix))]
    {
        // Native process-tree and pipe cancellation remains tracked in PLAT-01.
        let bytes = crate::read_bytes(path, args)?;
        if bytes.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Git metadata exceeded its output limit",
            ));
        }
        Ok(bytes)
    }
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
        let bytes = crate::read_bytes(path, args)?;
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
    fn invalid_utf8_at_a_clipped_boundary_is_only_lossy_display_data() {
        assert_eq!(
            display(b"A\xc3", true, "Message"),
            "A\u{fffd}\n[Message preview truncated]"
        );
    }
}
