//! Explicit stash save/apply. Applying never drops the saved entry.
use super::{changes, commit_basis, file_path, read_bytes, run_write, WRITE_LOCK};
use desktop_core::{CommitBasis, StashEntry};
use std::{ffi::OsStr, io, path::Path};

pub(super) fn list(path: &Path) -> io::Result<Vec<StashEntry>> {
    let bytes = read_bytes(path, &["stash", "list", "--format=%H%x00%gs%x00"])?;
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split(|b| *b == 0).collect();
            if fields.len() != 3
                || !fields[2].is_empty()
                || !matches!(fields[0].len(), 40 | 64)
                || !fields[0].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid Git stash record",
                ));
            }
            Ok(StashEntry {
                oid: String::from_utf8_lossy(fields[0]).into_owned(),
                subject: String::from_utf8_lossy(fields[1]).into_owned(),
            })
        })
        .collect()
}

fn validate_basis(path: &Path, expected: &CommitBasis) -> io::Result<()> {
    if &commit_basis(path)? != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh before using saved changes",
        ));
    }
    if expected.rebase.is_some() {
        return Err(io::Error::other(
            "Complete or abort the rebase before using saved changes",
        ));
    }
    if expected.merge_heads.is_some() {
        return Err(io::Error::other(
            "Complete or abort the current merge before using saved changes",
        ));
    }
    if expected.head.is_none() {
        return Err(io::Error::other(
            "Create the initial commit before using saved changes",
        ));
    }
    Ok(())
}

/// Save staged/unstaged tracked changes, leaving unstaged new files in place.
/// The current working-tree contents are saved; external edits are not fenced.
pub fn save_stash(path: &Path, message: &str, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_basis(path, expected)?;
    let changes = changes(path)?;
    if changes.iter().any(|change| change.is_conflicted()) {
        return Err(io::Error::other(
            "Resolve existing conflicts before saving changes",
        ));
    }
    if !changes.iter().any(|change| change.index != '?') {
        return Err(io::Error::other(
            "No tracked changes to save; new files must be staged first",
        ));
    }
    let message = if message.trim().is_empty() {
        "Desktop Rust saved changes"
    } else {
        message
    };
    run_write(path, &["stash", "push", "--message", message, "--"], None)?;
    Ok(String::from(
        "Tracked changes saved; unstaged new files were kept in the working tree",
    ))
}

fn check_collisions(path: &Path, stash: &StashEntry) -> io::Result<()> {
    let mut paths = read_bytes(
        path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            "--no-renames",
            "-z",
            &format!("{}^1", stash.oid),
            &stash.oid,
            "--",
        ],
    )?;
    // Stashes made outside the app may also contain untracked/ignored files.
    if read_bytes(
        path,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{}^3", stash.oid),
        ],
    )
    .is_ok()
    {
        paths.extend(read_bytes(
            path,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                "-z",
                &format!("{}^3", stash.oid),
                "--",
            ],
        )?);
    }
    for bytes in paths.split(|b| *b == 0).filter(|name| !name.is_empty()) {
        let relative = file_path(bytes)?;
        let mut parent = relative.parent();
        while let Some(component) = parent.filter(|p| !p.as_os_str().is_empty()) {
            match std::fs::symlink_metadata(path.join(component)) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(io::Error::other(format!(
                        "Path blocks restore: {}",
                        component.display()
                    )))
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            parent = component.parent();
        }
        match std::fs::symlink_metadata(path.join(&relative)) {
            Ok(_) => {
                if read_bytes(
                    path,
                    &[
                        OsStr::new("ls-files"),
                        OsStr::new("--error-unmatch"),
                        OsStr::new("--"),
                        relative.as_os_str(),
                    ],
                )
                .is_err()
                {
                    return Err(io::Error::other(format!(
                        "Untracked or ignored path would be overwritten: {}",
                        relative.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Restore a selected stash's staged and unstaged changes, retaining the stash.
/// Require a clean working tree; conflicts are reported without dropping the copy.
pub fn apply_stash(
    path: &Path,
    selected: &StashEntry,
    expected: &CommitBasis,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_basis(path, expected)?;
    if !list(path)?.iter().any(|stash| stash.oid == selected.oid) {
        return Err(io::Error::other(
            "Saved changes were removed; refresh before restoring",
        ));
    }
    if !changes(path)?.is_empty() {
        return Err(io::Error::other(
            "A clean working tree is required before restoring saved changes",
        ));
    }
    check_collisions(path, selected)?;
    run_write(
        path,
        &["stash", "apply", "--index", "--", &selected.oid],
        None,
    )?;
    Ok(String::from(
        "Saved changes restored; the stash entry was retained",
    ))
}
