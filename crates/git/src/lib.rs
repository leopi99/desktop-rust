//! Git adapter with literal path arguments and serialized local writes.
mod basis;
use basis::commit_basis;
mod branches;
mod conflicts;
pub use conflicts::{
    apply_conflict_side_resolution, inspect_conflict, inspect_conflict_side, resolve_conflict,
    review_conflict_side_resolution,
};
mod process;
mod read_output;
mod runtime;
pub use runtime::git_runtime;
mod hunks;
pub use hunks::{review_hunks, review_staged_hunks, stage_hunk, unstage_hunk};
mod history;
mod init;
pub use history::{commit_details, commit_file_diff, commit_history};
mod merge;
pub use init::{init_repository, init_repository_with_default_branch};
pub use merge::{abort_merge, execute_merge, preview_merge};
mod rebase;
mod remotes;
pub use rebase::{abort_rebase, continue_rebase, execute_rebase, preview_rebase};
mod stash;
pub use stash::{apply_stash, save_stash};
mod transfer;
pub use branches::{
    create_branch, create_branch_at_commit, delete_local_branch, rename_current_branch,
    rename_local_branch, switch_branch,
};
use desktop_core::{CommitBasis, FileChange, FileDiff, Repository, RepositorySnapshot};
pub use remotes::{
    add_remote, clone_repository, clone_repository_with_control, remote_operation,
    remote_operation_with_control, remove_remote, set_remote_url,
};
use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
pub use transfer::TransferControl;

/// Build Git processes with inherited repository-routing variables removed.
fn base_git_command() -> Command {
    let mut command = Command::new("git");
    command.env("GIT_TERMINAL_PROMPT", "0");
    for key in [
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
        "GIT_SHALLOW_FILE",
    ] {
        command.env_remove(key);
    }
    command
}

/// Bind repository operations to the requested path, independently of Git's cwd.
fn git_command(path: &Path) -> Command {
    let mut command = base_git_command();
    command.arg("--literal-pathspecs").arg("-C").arg(path);
    command
}

fn file_path(bytes: &[u8]) -> io::Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.to_vec())
            .map(PathBuf::from)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

fn changes(path: &Path) -> io::Result<Vec<FileChange>> {
    let output = read_output::complete(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        8 * 1024 * 1024,
    )?;
    let mut records = output.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut changes = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Git status record",
            ));
        }
        let original_path = if record[..2].iter().any(|b| matches!(b, b'R' | b'C')) {
            Some(file_path(records.next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Missing rename source")
            })?)?)
        } else {
            None
        };
        changes.push(FileChange {
            path: file_path(&record[3..])?,
            original_path,
            index: char::from(record[0]),
            worktree: char::from(record[1]),
        });
    }
    Ok(changes)
}

/// Read a repository without changing its index or working tree.
///
/// # Errors
///
/// Returns an error if Git cannot inspect the repository or its metadata is malformed.
pub fn snapshot(path: &Path) -> io::Result<RepositorySnapshot> {
    let root = read_output::complete(path, &["rev-parse", "--show-toplevel"], 64 * 1024)?;
    // Git terminates this path with one newline; filenames may themselves contain newlines.
    let root = file_path(root.strip_suffix(b"\n").unwrap_or(&root))?;
    let status = String::from_utf8_lossy(&read_output::complete(
        &root,
        &["status", "--short", "--branch"],
        8 * 1024 * 1024,
    )?)
    .into_owned();
    let commit_basis = commit_basis(&root)?;
    let commits = if let Some(head) = &commit_basis.head {
        history::commit_history(&root, head.trim(), 0)?
    } else {
        Vec::new()
    };
    let history = if commit_basis.head.is_none() {
        String::from("No commits yet")
    } else {
        commits
            .iter()
            .map(|commit| format!("{} {}", &commit.oid[..7], commit.subject))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let changes = changes(&root)?;
    let branches = branches::list(&root)?;
    let remotes = remotes::list(&root)?;
    let stashes = stash::list(&root)?;
    let merge_message = if commit_basis.merge_heads.is_some() {
        Some(
            merge::metadata(&root, "MERGE_MSG")?
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_else(|| String::from("Merge branches")),
        )
    } else {
        None
    };
    Ok(RepositorySnapshot {
        repository: Repository { path: root },
        status,
        history,
        commits,
        changes,
        branches,
        remotes,
        stashes,
        merge_message,
        commit_basis,
    })
}

/// Show index and working-tree differences separately, including unborn branches.
/// The selected path must still occur in the repository status. Tracked patches
/// retain at most 256 KiB of raw output per side, with an explicit truncation marker.
///
/// # Errors
///
/// Returns an error if the selected change is stale, unsupported or cannot be read by Git.
/// Unix patch subprocesses also fail on a thirty-second deadline or oversized stderr.
pub fn diff(path: &Path, selected: &FileChange) -> io::Result<FileDiff> {
    if !changes(path)?.contains(selected) {
        return Err(io::Error::other(
            "File status changed; refresh the repository",
        ));
    }
    if selected.index == '?' {
        let full = path.join(&selected.path);
        let metadata = std::fs::symlink_metadata(&full)?;
        let preview = if metadata.file_type().is_symlink() {
            format!("Symbolic link → {}", std::fs::read_link(full)?.display())
        } else if !metadata.is_file() || metadata.len() > 1024 * 1024 {
            String::from("Preview unavailable: non-regular file or file larger than 1 MiB")
        } else {
            let bytes = std::fs::read(full)?;
            if bytes.contains(&0) {
                String::from("Binary file")
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            }
        };
        return Ok(FileDiff {
            staged: String::new(),
            unstaged: format!("Untracked file\n{preview}"),
        });
    }
    let read_diff = |cached: bool| -> io::Result<String> {
        let mut args = vec![
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-textconv"),
            OsStr::new("--no-color"),
            OsStr::new("--find-renames"),
        ];
        if cached {
            args.push(OsStr::new("--cached"));
        }
        args.push(OsStr::new("--"));
        args.push(selected.path.as_os_str());
        if let Some(original) = &selected.original_path {
            args.push(original.as_os_str());
        }
        read_output::preview(
            path,
            &args,
            256 * 1024,
            if cached {
                "Staged diff"
            } else {
                "Unstaged diff"
            },
        )
    };
    Ok(FileDiff {
        staged: read_diff(true)?,
        unstaged: read_diff(false)?,
    })
}

// Serialize writes issued by this process. Git retains its own mandatory locks
// for coordination with other clients; this mutex does not lock external editors.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

fn run_write<S: AsRef<OsStr>>(path: &Path, args: &[S], input: Option<&str>) -> io::Result<String> {
    let mut command = git_command(path);
    command.args(args);
    process::write_output(command, input)
}

fn validate_selection(path: &Path, selected: &FileChange) -> io::Result<()> {
    if !changes(path)?.contains(selected) {
        return Err(io::Error::other(
            "File status changed; refresh the repository",
        ));
    }
    if selected.is_conflicted() {
        return Err(io::Error::other(
            "Use conflict review to mark the edited file as resolved",
        ));
    }
    Ok(())
}

/// Stage the selected file's current working-tree contents, including deletion.
/// Status is revalidated, but concurrent external edits to file contents are not fenced.
///
/// # Errors
///
/// Returns an error for a stale/conflicted selection, Git rejection or filesystem failure.
pub fn stage(path: &Path, selected: &FileChange) -> io::Result<()> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_selection(path, selected)?;
    if !selected.can_stage() {
        return Err(io::Error::other("No working-tree change to stage"));
    }
    let mut args = vec![
        OsStr::new("add"),
        OsStr::new("--all"),
        OsStr::new("--"),
        selected.path.as_os_str(),
    ];
    if matches!(selected.worktree, 'R' | 'C') {
        if let Some(original) = &selected.original_path {
            args.push(original.as_os_str());
        }
    }
    run_write(path, &args, None)?;
    Ok(())
}

/// Remove a selected change from the index without changing working-tree files.
///
/// # Errors
///
/// Returns an error for a stale/conflicted selection, Git rejection or filesystem failure.
pub fn unstage(path: &Path, selected: &FileChange) -> io::Result<()> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_selection(path, selected)?;
    if !selected.can_unstage() {
        return Err(io::Error::other("No index change to unstage"));
    }
    // `reset -- <paths>` resets the index to HEAD, or the empty tree on an unborn branch.
    let mut args = vec![
        OsStr::new("reset"),
        OsStr::new("--quiet"),
        OsStr::new("--"),
        selected.path.as_os_str(),
    ];
    if selected.index == 'R' {
        if let Some(original) = &selected.original_path {
            args.push(original.as_os_str());
        }
    }
    run_write(path, &args, None)?;
    Ok(())
}

/// Commit the entire staged index, preserving unstaged changes and configured hooks.
/// Reject an index changed since the snapshot. External clients can still race
/// between this check and Git acquiring its own locks.
///
/// # Errors
///
/// Returns an error for a stale basis, unresolved operation, invalid message, hook/signing failure or Git rejection.
pub fn commit(path: &Path, message: &str, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if message.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Commit message cannot be empty",
        ));
    }
    let current = commit_basis(path)?;
    if &current != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh and review before committing",
        ));
    }
    if current.rebase.is_some() {
        return Err(io::Error::other(
            "Use Continue rebase to finish the current replayed commit",
        ));
    }
    let entries = changes(path)?;
    if entries.iter().any(FileChange::is_conflicted) {
        return Err(io::Error::other("Cannot commit with unresolved conflicts"));
    }
    if !entries.iter().any(FileChange::can_unstage) && current.merge_heads.is_none() {
        return Err(io::Error::other("No staged changes to commit"));
    }
    run_write(path, &["commit", "--file=-"], Some(message))
}
