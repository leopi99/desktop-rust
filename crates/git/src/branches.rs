//! Local branch operations. Git owns overwrite and linked-worktree checks.
use super::{commit_basis, file_path, read_bytes, run_write, WRITE_LOCK};
use desktop_core::{CommitBasis, LocalBranch};
use std::{ffi::OsStr, io, path::Path};

pub(super) fn list(path: &Path) -> io::Result<Vec<LocalBranch>> {
    let bytes = read_bytes(
        path,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)%00%(HEAD)",
            "refs/heads/",
        ],
    )?;
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split(|b| *b == 0).collect();
            if fields.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid Git branch record",
                ));
            }
            let name = fields[0].strip_prefix(b"refs/heads/").ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Unexpected branch namespace")
            })?;
            let oid = std::str::from_utf8(fields[1])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_owned();
            Ok(LocalBranch {
                name: file_path(name)?.into_os_string(),
                oid,
                is_current: fields[2] == b"*",
            })
        })
        .collect()
}

fn validate_basis(path: &Path, expected: &CommitBasis) -> io::Result<()> {
    if expected.rebase.is_some() {
        return Err(io::Error::other(
            "Complete or abort the rebase before switching branches",
        ));
    }
    if expected.merge_heads.is_some() {
        return Err(io::Error::other(
            "Complete or abort the current merge before switching branches",
        ));
    }
    if &commit_basis(path)? != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh before changing branches",
        ));
    }
    Ok(())
}

/// Create and switch to a local branch at the current HEAD, without resetting an
/// existing branch. On an unborn branch this changes the initial branch name.
/// Configured checkout hooks run normally; Git preserves or rejects local edits.
pub fn create_branch(path: &Path, name: &str, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_name(path, name)?;
    validate_basis(path, expected)?;
    run_write(
        path,
        &["switch", "--no-guess", "--no-track", "--create", name],
        None,
    )?;
    Ok(format!("Created and switched to {name}"))
}

/// Switch only to an explicitly selected local branch at its observed target.
/// No force, automatic remote tracking, reset, or automatic stash is performed.
/// External Git clients can still race after the preflight checks.
pub fn switch_branch(
    path: &Path,
    selected: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if selected.name.as_encoded_bytes().starts_with(b"-") || selected.name == OsStr::new("HEAD") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported reserved branch name",
        ));
    }
    validate_basis(path, expected)?;
    if !list(path)?
        .iter()
        .any(|branch| branch.name == selected.name && branch.oid == selected.oid)
    {
        return Err(io::Error::other(
            "Target branch changed or was deleted; refresh before switching",
        ));
    }
    run_write(
        path,
        &[
            OsStr::new("switch"),
            OsStr::new("--no-guess"),
            OsStr::new("--"),
            selected.name.as_os_str(),
        ],
        None,
    )?;
    Ok(format!("Switched to {}", selected.name.to_string_lossy()))
}

/// Validate a literal local branch name without expanding previous-checkout shorthand.
pub(super) fn validate_name(path: &Path, name: &str) -> io::Result<()> {
    if name.is_empty() || name.starts_with('-') || name == "HEAD" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid local branch name",
        ));
    }
    read_bytes(path, &["check-ref-format", &format!("refs/heads/{name}")])?;
    Ok(())
}
