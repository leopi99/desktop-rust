//! Local branch merge previews and explicit merge/abort operations.
use super::{commit_basis, file_path, git_command, run_write, snapshot, WRITE_LOCK};
use desktop_core::{CommitBasis, LocalBranch, MergeKind, MergePreview};
use std::{
    ffi::OsString,
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(super) fn metadata_path(root: &Path, name: &str) -> io::Result<PathBuf> {
    let bytes = super::read_output::complete(root, &["rev-parse", "--git-path", name], 64 * 1024)?;
    let path = file_path(bytes.strip_suffix(b"\n").unwrap_or(&bytes))?;
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

pub(super) fn metadata(root: &Path, name: &str) -> io::Result<Option<Vec<u8>>> {
    let path = metadata_path(root, name)?;
    read_metadata_file(&path)
}

pub(super) fn read_metadata_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() && meta.len() <= 1024 * 1024 => {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 1024 * 1024 {
                return Err(io::Error::other("Oversized Git operation metadata"));
            }
            Ok(Some(bytes))
        }
        Ok(_) => Err(io::Error::other("Unsupported Git operation metadata")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn ensure_idle(root: &Path) -> io::Result<()> {
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        match std::fs::symlink_metadata(metadata_path(root, name)?) {
            Ok(_) => {
                return Err(io::Error::other(
                    "Finish or abort the existing Git operation before starting another",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn ancestor(root: &Path, first: &str, second: &str) -> io::Result<bool> {
    let output = git_command(root)
        .args(["merge-base", "--is-ancestor", first, second])
        .output()?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )),
    }
}

fn build_preview(
    path: &Path,
    target: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<MergePreview> {
    let state = snapshot(path)?;
    if &state.commit_basis != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh before previewing the merge",
        ));
    }
    ensure_idle(&state.repository.path)?;
    if expected.branch.is_none() || expected.head.is_none() {
        return Err(io::Error::other(
            "Select a committed local branch before merging",
        ));
    }
    if !state.changes.is_empty() {
        return Err(io::Error::other(
            "Save or commit local changes before merging",
        ));
    }
    if !state
        .branches
        .iter()
        .any(|branch| branch.name == target.name && branch.oid == target.oid)
    {
        return Err(io::Error::other(
            "Target branch changed or was removed; refresh the merge preview",
        ));
    }
    let head = expected
        .head
        .as_deref()
        .ok_or_else(|| io::Error::other("A committed HEAD is required"))?
        .trim();
    let mut preview = MergePreview {
        repository: state.repository,
        target: target.clone(),
        basis: expected.clone(),
        kind: MergeKind::AlreadyUpToDate,
        tree_oid: None,
        conflicts: Vec::new(),
    };
    if ancestor(&preview.repository.path, &target.oid, head)? {
        return Ok(preview);
    }
    if ancestor(&preview.repository.path, head, &target.oid)? {
        preview.kind = MergeKind::FastForward;
        return Ok(preview);
    }
    let common = git_command(&preview.repository.path)
        .args(["merge-base", "--", head, &target.oid])
        .output()?;
    if common.status.code() == Some(1) {
        return Err(io::Error::other("Branches have unrelated histories"));
    }
    if !common.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&common.stderr).into_owned(),
        ));
    }
    let output = git_command(&preview.repository.path)
        .args([
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            head,
            &target.oid,
        ])
        .output()?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let mut fields = output.stdout.split(|b| *b == 0);
    let tree = fields.next().unwrap_or_default();
    if !matches!(tree.len(), 40 | 64) || !tree.iter().all(u8::is_ascii_hexdigit) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid merge-tree result",
        ));
    }
    preview.tree_oid = Some(String::from_utf8_lossy(tree).into_owned());
    preview.conflicts = fields
        .filter(|name| !name.is_empty())
        .map(file_path)
        .collect::<io::Result<Vec<_>>>()?;
    preview.kind = if output.status.success() {
        MergeKind::CleanMerge
    } else {
        MergeKind::Conflicts
    };
    if (preview.kind == MergeKind::Conflicts) == preview.conflicts.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Inconsistent merge-tree conflict list",
        ));
    }
    Ok(preview)
}

/// Predict mergeability without changing HEAD, index or working-tree files.
/// Divergent previews write temporary Git objects and can invoke configured merge drivers.
///
/// # Errors
///
/// Returns an error for stale/dirty/unsupported source state, changed targets, unrelated histories or Git failure.
pub fn preview_merge(
    path: &Path,
    target: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<MergePreview> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    build_preview(path, target, expected)
}

/// Merge exactly the reviewed commit. Fast-forwards finish immediately; divergent
/// merges stop before commit so the user can review or resolve the resulting index.
///
/// # Errors
///
/// Returns an error if the preview is no longer valid, checkout collisions are found or Git rejects execution.
pub fn execute_merge(preview: &MergePreview) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if &build_preview(&preview.repository.path, &preview.target, &preview.basis)? != preview {
        return Err(io::Error::other(
            "Merge result changed; preview the merge again",
        ));
    }
    if preview.kind == MergeKind::AlreadyUpToDate {
        return Ok(String::from("Already up to date"));
    }
    // branch.<name>.mergeOptions can add strategies (not just defaults), so clear
    // them before supplying the exact, previewed merge policy.
    let branch = Path::new(
        preview
            .basis
            .branch
            .as_ref()
            .ok_or_else(|| io::Error::other("A local branch is required"))?,
    )
    .strip_prefix("refs/heads")
    .map_err(|_| io::Error::other("Invalid local branch ref"))?;
    let mut options = OsString::from("branch.");
    options.push(branch.as_os_str());
    options.push(".mergeOptions=");
    let mut args: Vec<OsString> = ["-c", "merge.autoStash=false", "-c"]
        .map(OsString::from)
        .to_vec();
    args.push(options);
    args.extend(
        [
            "merge",
            "--ff",
            "--no-squash",
            "--no-commit",
            "--no-edit",
            "--no-autostash",
            "--no-overwrite-ignore",
            "--strategy=ort",
            "--",
        ]
        .map(OsString::from),
    );
    args.push(preview.target.oid.clone().into());
    let result = run_write(&preview.repository.path, &args, None);
    if let Err(error) = result {
        let state = snapshot(&preview.repository.path)?;
        if state.commit_basis.merge_heads.is_some()
            && state.changes.iter().any(|change| change.is_conflicted())
        {
            return Ok(String::from(
                "Merge paused with conflicts. Review and resolve the files, then commit the merge.",
            ));
        }
        return Err(error);
    }
    Ok(match preview.kind {
        MergeKind::FastForward => String::from("Fast-forward merge completed"),
        _ => String::from("Merge prepared. Review staged changes, then complete the merge commit."),
    })
}

/// Abort an in-progress merge after explicit user confirmation. Git discards merge
/// results/resolution edits; no stash or unrelated untracked files are deleted.
///
/// # Errors
///
/// Returns an error for stale/non-merge state or Git rejection; abort can discard merge resolution edits.
pub fn abort_merge(path: &Path, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if &commit_basis(path)? != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh before aborting",
        ));
    }
    if expected.merge_heads.is_none() {
        return Err(io::Error::other("No merge is in progress"));
    }
    run_write(path, &["merge", "--abort"], None)?;
    Ok(String::from("Merge aborted"))
}
