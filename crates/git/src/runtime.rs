//! Read-only engine diagnostics without opening a repository or changing config.
use desktop_core::GitRuntime;
use std::{
    io,
    process::{Command, Stdio},
};

/// Inspect the system Git currently resolved through PATH. No install/update is
/// performed, and the observed version is not a compatibility or security claim.
pub fn git_runtime() -> io::Result<GitRuntime> {
    inspect(super::base_git_command())
}

fn inspect(mut command: Command) -> io::Result<GitRuntime> {
    let output = command
        .arg("--version")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Unable to start Git: {error}. Install Git and ensure it is available on PATH."
                ),
            )
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
}
