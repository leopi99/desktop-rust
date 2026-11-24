//! Local branch operations. Git owns overwrite and linked-worktree checks.
use super::{commit_basis, file_path, run_write, WRITE_LOCK};
use desktop_core::{CommitBasis, LocalBranch};
use std::{ffi::OsStr, io, path::Path};

pub(super) fn list(path: &Path) -> io::Result<Vec<LocalBranch>> {
    let bytes = super::read_output::complete(
        path,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)%00%(HEAD)",
            "refs/heads/",
        ],
        4 * 1024 * 1024,
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
///
/// # Errors
///
/// Returns an error for an invalid/occupied name, stale source state, active merge/rebase or Git rejection.
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
///
/// # Errors
///
/// Returns an error for a stale source/target, active operation, checkout collision or Git rejection.
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

/// Rename the observed current local branch without overwriting another ref.
/// Git moves its reflog and local branch configuration; index and files stay put.
/// Unborn/detached HEAD and active sequencers require a different workflow.
/// External clients can still race after these preflight checks.
///
/// # Errors
///
/// Returns an error for invalid or occupied names/configuration, stale state, active operation, worktree use or Git rejection.
pub fn rename_current_branch(
    path: &Path,
    name: &str,
    expected: &CommitBasis,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_name(path, name)?;
    validate_basis(path, expected)?;
    super::merge::ensure_idle(path)?;
    let branch = expected
        .branch
        .as_ref()
        .ok_or_else(|| io::Error::other("Select a committed local branch before renaming"))?;
    let source = branch
        .as_encoded_bytes()
        .strip_prefix(b"refs/heads/")
        .filter(|branch| !branch.is_empty())
        .ok_or_else(|| io::Error::other("Select a committed local branch before renaming"))?;
    if expected.head.is_none() {
        return Err(io::Error::other(
            "Create the first commit before renaming this branch",
        ));
    }
    let source = file_path(source)?.into_os_string();
    let selected = list(path)?
        .into_iter()
        .find(|branch| branch.name == source)
        .ok_or_else(|| io::Error::other("Current branch changed; refresh before renaming"))?;
    rename_observed_branch(path, &selected, name, expected)
}

/// Rename an observed non-current local branch without switching to it.
/// Git moves its reflog and local configuration; current HEAD, index and files
/// stay put. Branches used by any worktree are refused. No force or remote write
/// is performed; external clients can still race after preflight validation.
///
/// # Errors
///
/// Returns an error for stale source/target, invalid or occupied destinations,
/// active operations, detached/unborn HEAD, worktree use or Git rejection.
pub fn rename_local_branch(
    path: &Path,
    selected: &LocalBranch,
    name: &str,
    expected: &CommitBasis,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_name(path, name)?;
    validate_basis(path, expected)?;
    super::merge::ensure_idle(path)?;
    let mut full_ref = std::ffi::OsString::from("refs/heads/");
    full_ref.push(&selected.name);
    if selected.is_current || expected.branch.as_ref() == Some(&full_ref) {
        return Err(io::Error::other(
            "Use current-branch rename for the current branch",
        ));
    }
    rename_observed_branch(path, selected, name, expected)
}

fn rename_observed_branch(
    path: &Path,
    selected: &LocalBranch,
    name: &str,
    expected: &CommitBasis,
) -> io::Result<String> {
    if expected.head.is_none() || expected.branch.is_none() {
        return Err(io::Error::other(
            "Select a committed local branch before renaming",
        ));
    }
    let branches = list(path)?;
    if selected.name.as_encoded_bytes().starts_with(b"-")
        || selected.name == OsStr::new("HEAD")
        || !branches.contains(selected)
    {
        return Err(io::Error::other(
            "The selected branch changed or was deleted; refresh before renaming",
        ));
    }
    if selected.name == OsStr::new(name)
        || branches
            .iter()
            .any(|branch| branch.name == OsStr::new(name))
    {
        return Err(io::Error::other(
            "The destination branch already exists; choose a different name",
        ));
    }
    // An orphan configuration section can still contain valuable tracking settings.
    if super::read_output::complete(
        path,
        &["config", "--local", "--null", "--list"],
        4 * 1024 * 1024,
    )?
    .split(|byte| *byte == 0)
    .any(|entry| {
        let key = entry
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default();
        key.strip_prefix(b"branch.")
            .and_then(|key| key.rsplitn(2, |byte| *byte == b'.').nth(1))
            == Some(name.as_bytes())
    }) {
        return Err(io::Error::other(
            "The destination already has branch configuration; choose another name",
        ));
    }
    let mut full_ref = std::ffi::OsString::from("refs/heads/");
    full_ref.push(&selected.name);
    let is_current = expected.branch.as_ref() == Some(&full_ref);
    let full_ref = full_ref.as_encoded_bytes();
    if super::read_output::complete(
        path,
        &["worktree", "list", "--porcelain", "-z"],
        4 * 1024 * 1024,
    )?
    .split(|byte| *byte == 0)
    .filter(|entry| entry.strip_prefix(b"branch ") == Some(full_ref))
    .count()
        > usize::from(is_current)
    {
        return Err(io::Error::other(
            "This branch is also checked out in another worktree",
        ));
    }
    run_write(
        path,
        &[
            OsStr::new("branch"),
            OsStr::new("--move"),
            OsStr::new("--"),
            selected.name.as_os_str(),
            OsStr::new(name),
        ],
        None,
    )?;
    Ok(format!(
        "Renamed {} to {name}. Remote branch names are unchanged.",
        selected.name.to_string_lossy()
    ))
}

/// Delete an observed non-current local branch only when its tip is merged into
/// the current HEAD. Git also checks its upstream and linked-worktree use.
/// No forced deletion or remote operation is performed. External clients can
/// still race between preflight checks and Git acquiring its own ref locks.
///
/// # Errors
///
/// Returns an error for current/unmerged/stale branches, active operations, worktree use or Git rejection.
pub fn delete_local_branch(
    path: &Path,
    selected: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_basis(path, expected)?;
    super::merge::ensure_idle(path)?;
    let (Some(current), Some(head)) = (&expected.branch, &expected.head) else {
        return Err(io::Error::other(
            "Select a committed local branch before deleting another branch",
        ));
    };
    let mut full_ref = std::ffi::OsString::from("refs/heads/");
    full_ref.push(&selected.name);
    if selected.is_current || &full_ref == current {
        return Err(io::Error::other("Cannot delete the current branch"));
    }
    if selected.name.as_encoded_bytes().starts_with(b"-") || selected.name == OsStr::new("HEAD") {
        return Err(io::Error::other("Unsupported reserved branch name"));
    }
    if !list(path)?
        .iter()
        .any(|branch| branch.name == selected.name && branch.oid == selected.oid)
    {
        return Err(io::Error::other(
            "The selected branch changed or was deleted; refresh before deleting",
        ));
    }
    if !super::merge::ancestor(path, &selected.oid, head.trim())? {
        return Err(io::Error::other(
            "The selected branch contains commits not merged into the current branch",
        ));
    }
    run_write(
        path,
        &[
            OsStr::new("branch"),
            OsStr::new("--delete"),
            OsStr::new("--"),
            selected.name.as_os_str(),
        ],
        None,
    )?;
    Ok(format!(
        "Deleted local branch {}. Remote branches are unchanged.",
        selected.name.to_string_lossy()
    ))
}

/// Validate a literal local branch name without expanding previous-checkout shorthand.
pub(super) fn validate_name(path: &Path, name: &str) -> io::Result<()> {
    if name.is_empty() || name.starts_with('-') || name == "HEAD" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid local branch name",
        ));
    }
    super::read_output::complete(
        path,
        &["check-ref-format", &format!("refs/heads/{name}")],
        64 * 1024,
    )?;
    Ok(())
}
