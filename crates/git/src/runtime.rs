//! Read-only engine diagnostics without opening a repository or changing config.
use desktop_core::GitRuntime;
use std::{
    io,
    process::{Command, Stdio},
    time::Duration,
};

/// Inspect the system Git currently resolved through PATH. No install/update is
/// performed, and the observed version is not a compatibility or security claim.
///
/// # Errors
///
/// Returns an error if Git cannot start, exits unsuccessfully or reports a malformed version.
/// On Unix, a five-second deadline and 512/1024-byte stdout/stderr limits also apply.
pub fn git_runtime() -> io::Result<GitRuntime> {
    inspect(super::base_git_command())
}

fn inspect(command: Command) -> io::Result<GitRuntime> {
    inspect_with_timeout(command, Duration::from_secs(5))
}

fn inspect_with_timeout(mut command: Command, timeout: Duration) -> io::Result<GitRuntime> {
    command
        .arg("--version")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    #[cfg(unix)]
    let result = super::process::bounded_output(
        command,
        super::process::OutputLimits {
            timeout,
            stdout: 512,
            stderr: 1024,
        },
    );
    #[cfg(not(unix))]
    let result = {
        // Native Windows process-tree/pipe cancellation is still tracked in PLAT-01.
        let _ = timeout;
        command.output()
    };
    let output = result.map_err(|error| {
        let message = match error.kind() {
            io::ErrorKind::NotFound => format!(
                "Unable to start Git: {error}. Install Git and ensure it is available on PATH."
            ),
            io::ErrorKind::TimedOut => String::from(
                "Git version check timed out. Recheck Git after fixing the executable on PATH.",
            ),
            _ => format!("Git version check failed: {error}"),
        };
        io::Error::new(error.kind(), message)
    })?;
    if !output.status.success() {
        let end = output.stderr.len().min(1024);
        return Err(io::Error::other(format!(
            "Git version check failed: {}",
            String::from_utf8_lossy(&output.stderr[..end])
        )));
    }
    parse_version(&output.stdout)
}

fn parse_version(bytes: &[u8]) -> io::Result<GitRuntime> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Unexpected Git version output");
    if bytes.len() > 512 {
        return Err(invalid());
    }
    let line = std::str::from_utf8(bytes).map_err(|_| invalid())?.trim();
    let version = line.strip_prefix("git version ").ok_or_else(invalid)?;
    let components: Vec<_> = version
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .split('.')
        .collect();
    if components.len() < 2
        || components[..2]
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
        || version.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(GitRuntime {
        version: version.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_vendor_prerelease_and_development_versions_without_guessing_compatibility() {
        for version in [
            "2.56.1",
            "2.56.0.windows.4",
            "2.39.5 (Apple Git-154)",
            "2.57.0.rc0",
            "2.56.GIT",
        ] {
            assert_eq!(
                parse_version(format!("git version {version}\n").as_bytes())
                    .unwrap()
                    .version,
                version
            );
        }
    }

    #[test]
    fn rejects_malformed_multiline_non_utf8_and_oversized_reports() {
        for bytes in [
            b"".as_slice(),
            b"git version unknown",
            b"git version 2",
            b"git version 2.56.1\nextra",
            b"git version 2.56.1\x1b[0m",
            b"git version 2.56.1\xff",
            &[b'x'; 513],
        ] {
            assert!(parse_version(bytes).is_err());
        }
    }

    #[test]
    fn missing_executable_reports_an_actionable_error() {
        let path = std::env::temp_dir()
            .join(format!("desktop-rust-missing-git-{}", std::process::id()))
            .join("git");
        let error = inspect(Command::new(path)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("Install Git"));
    }

    #[test]
    fn installed_git_can_be_inspected_without_a_repository_path() {
        let runtime = git_runtime().unwrap();
        assert!(!runtime.version.is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn a_failing_version_command_reports_its_error() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'version query unavailable' >&2; exit 7"]);
        let error = inspect(command).unwrap_err();
        assert!(error.to_string().contains("version query unavailable"));
    }
    #[cfg(unix)]
    #[test]
    fn timeout_is_actionable_and_a_following_probe_can_succeed() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let error = inspect_with_timeout(command, Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("timed out"));
        assert!(!git_runtime().unwrap().version.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn noisy_and_non_utf8_commands_do_not_produce_a_valid_version() {
        for body in [
            "printf 'git version 2.56.1'; head -c 10000 /dev/zero",
            "printf 'git version 2.56.1\\377'",
        ] {
            let mut command = Command::new("sh");
            command.args(["-c", body]);
            assert_eq!(
                inspect(command).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
}
