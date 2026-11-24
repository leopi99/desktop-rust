//! Explicit non-interactive rebases of linear local history.
use super::{commit_basis, file_path, git_command, merge, snapshot, WRITE_LOCK};
use desktop_core::{
    CommitBasis, LocalBranch, RebaseCommit, RebaseKind, RebasePreview, RebaseState,
};
use std::{collections::HashSet, io, path::Path};

const COMMIT_LIMIT: usize = 1000;
const PATH_BYTES_LIMIT: usize = 8 * 1024 * 1024;
const PATH_COUNT_LIMIT: usize = 10_000;

fn read_metadata<S: AsRef<std::ffi::OsStr>>(root: &Path, args: &[S]) -> io::Result<Vec<u8>> {
    super::read_output::complete(root, args, 4 * 1024 * 1024)
}

fn hash(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

pub(super) fn state(root: &Path) -> io::Result<Option<RebaseState>> {
    for backend in ["rebase-merge", "rebase-apply"] {
        let directory = merge::metadata_path(root, backend)?;
        match std::fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
            Ok(meta) if meta.file_type().is_dir() => {}
            Ok(_) => return Err(io::Error::other("Unsupported rebase metadata directory")),
        }
        let mut metadata = Vec::new();
        for key in [
            "head-name",
            "orig-head",
            "onto",
            "stopped-sha",
            "git-rebase-todo",
            "done",
            "msgnum",
            "end",
            "autostash",
            "update-refs",
            "strategy",
            "strategy_opts",
            "rewritten-list",
            "message",
            "author-script",
            "amend",
            "git-rebase-todo.backup",
            "gpg_sign_opt",
        ] {
            if let Some(bytes) = merge::read_metadata_file(&directory.join(key))? {
                metadata.push((key.to_owned(), bytes));
            }
        }
        let get = |name: &str| {
            metadata
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, bytes)| bytes.as_slice())
        };
        let branch = get("head-name")
            .map(|raw| file_path(raw.strip_suffix(b"\n").unwrap_or(raw)))
            .transpose()?
            .map(|path| path.into_os_string())
            .unwrap_or_default();
        let original_head = get("orig-head").map(text).unwrap_or_default();
        let onto = get("onto").map(text).unwrap_or_default();
        let mut supported = backend == "rebase-merge"
            && branch.as_encoded_bytes().starts_with(b"refs/heads/")
            && hash(&original_head)
            && hash(&onto)
            && get("autostash").is_none()
            && get("update-refs").is_none_or(|bytes| text(bytes).is_empty())
            && get("strategy").is_none_or(|bytes| matches!(text(bytes).as_str(), "" | "ort"))
            && get("strategy_opts").is_none_or(|bytes| text(bytes).is_empty());
        let mut commits = Vec::new();
        for key in ["done", "git-rebase-todo"] {
            for line in get(key).unwrap_or_default().split(|b| *b == b'\n') {
                let line = line.trim_ascii();
                if line.is_empty() || line.starts_with(b"#") {
                    continue;
                }
                let mut words = line
                    .split(u8::is_ascii_whitespace)
                    .filter(|w| !w.is_empty());
                let command = words.next().unwrap_or_default();
                let oid = words.next().unwrap_or_default();
                if !matches!(command, b"pick" | b"p")
                    || !(4..=64).contains(&oid.len())
                    || !oid.iter().all(u8::is_ascii_hexdigit)
                {
                    supported = false;
                } else {
                    if commits.len() == COMMIT_LIMIT {
                        supported = false;
                        break;
                    }
                    commits.push(String::from_utf8_lossy(oid).into_owned());
                }
            }
        }
        let step = get("msgnum")
            .and_then(|v| text(v).parse().ok())
            .unwrap_or(1);
        let total = get("end")
            .and_then(|v| text(v).parse().ok())
            .unwrap_or(commits.len());
        return Ok(Some(RebaseState {
            branch,
            original_head,
            onto,
            step,
            total,
            supported,
            commits,
            metadata,
        }));
    }
    Ok(None)
}

fn build_preview(
    path: &Path,
    target: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<RebasePreview> {
    let source = snapshot(path)?;
    if &source.commit_basis != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; preview the rebase again",
        ));
    }
    merge::ensure_idle(&source.repository.path)?;
    if expected.branch.is_none() || expected.head.is_none() {
        return Err(io::Error::other(
            "Select a committed local branch before rebasing",
        ));
    }
    if !source.changes.is_empty() {
        return Err(io::Error::other(
            "Save or commit local changes before rebasing",
        ));
    }
    if !source
        .branches
        .iter()
        .any(|b| b.name == target.name && b.oid == target.oid)
    {
        return Err(io::Error::other(
            "Target branch changed or was removed; refresh the rebase preview",
        ));
    }
    let head = expected
        .head
        .as_deref()
        .ok_or_else(|| io::Error::other("A committed HEAD is required"))?
        .trim();
    let mut preview = RebasePreview {
        repository: source.repository,
        target: target.clone(),
        basis: expected.clone(),
        kind: RebaseKind::AlreadyUpToDate,
        commits: Vec::new(),
    };
    if merge::ancestor(path, &target.oid, head)? {
        return Ok(preview);
    }
    if merge::ancestor(path, head, &target.oid)? {
        preview.kind = RebaseKind::FastForward;
        return Ok(preview);
    }
    merge::require_common_ancestor(path, head, &target.oid)?;
    let range = format!("{}..{head}", target.oid);
    if !read_metadata(
        path,
        &["rev-list", "--merges", "--max-count=1", &range, "--"],
    )?
    .is_empty()
    {
        return Err(io::Error::other(
            "Source contains merge commits; only linear rebase is supported",
        ));
    }
    let bytes = read_metadata(
        path,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-color",
            &format!("--max-count={}", COMMIT_LIMIT + 1),
            "--reverse",
            "--topo-order",
            "--format=%H%x00%s%x00",
            &range,
            "--",
        ],
    )?;
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let fields: Vec<_> = line.split(|b| *b == 0).collect();
        if fields.len() != 3 || !hash(&String::from_utf8_lossy(fields[0])) {
            return Err(io::Error::other("Invalid rebase commit list"));
        }
        if preview.commits.len() == COMMIT_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Rebase exceeds the 1,000-commit review limit",
            ));
        }
        preview.commits.push(RebaseCommit {
            oid: String::from_utf8_lossy(fields[0]).into_owned(),
            subject: String::from_utf8_lossy(fields[1]).into_owned(),
        });
    }
    preview.kind = RebaseKind::Replay;
    Ok(preview)
}

/// Preview candidate linear commits without modifying refs, index or files.
///
/// # Errors
///
/// Returns an error for stale/dirty state, unsupported histories/metadata, checkout collisions or Git failure.
pub fn preview_rebase(
    path: &Path,
    target: &LocalBranch,
    expected: &CommitBasis,
) -> io::Result<RebasePreview> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    build_preview(path, target, expected)
}

fn append_paths(
    root: &Path,
    args: &[&str],
    paths: &mut Vec<u8>,
    count: &mut usize,
) -> io::Result<()> {
    let remaining = PATH_BYTES_LIMIT.saturating_sub(paths.len());
    let bytes = super::read_output::complete(root, args, remaining.min(4 * 1024 * 1024))?;
    let additional = bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .count();
    if additional > PATH_COUNT_LIMIT.saturating_sub(*count) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Rebase exceeds the 10,000-path review limit",
        ));
    }
    *count += additional;
    paths.extend(bytes);
    Ok(())
}

fn check_collisions(root: &Path, trees: &[String], commits: &[String]) -> io::Result<()> {
    if trees.len() > 2 || commits.len() > COMMIT_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Rebase exceeds the supported review size",
        ));
    }
    let mut paths = Vec::new();
    let mut count = 0;
    for tree in trees {
        append_paths(
            root,
            &["ls-tree", "-r", "--name-only", "-z", tree, "--"],
            &mut paths,
            &mut count,
        )?;
    }
    for commit in commits {
        append_paths(
            root,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--no-renames",
                "--name-only",
                "-r",
                "-z",
                commit,
                "--",
            ],
            &mut paths,
            &mut count,
        )?;
    }
    let index = super::read_output::complete(root, &["ls-files", "-z"], 16 * 1024 * 1024)?;
    let tracked: HashSet<&[u8]> = index.split(|b| *b == 0).collect();
    let paths: HashSet<&[u8]> = paths.split(|b| *b == 0).filter(|p| !p.is_empty()).collect();
    for raw in paths {
        let relative = file_path(raw)?;
        let mut parent = relative.parent();
        while let Some(component) = parent.filter(|p| !p.as_os_str().is_empty()) {
            match std::fs::symlink_metadata(root.join(component)) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                    return Err(io::Error::other(
                        "A symlink or file blocks the rebase checkout path",
                    ))
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            parent = component.parent();
        }
        match std::fs::symlink_metadata(root.join(&relative)) {
            Ok(_) if !tracked.contains(raw) => {
                return Err(io::Error::other(format!(
                    "Untracked or ignored path would be overwritten: {}",
                    relative.display()
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn run(root: &Path, args: &[&str]) -> io::Result<String> {
    let mut command = git_command(root);
    command
        .args(args)
        .env("GIT_EDITOR", ":")
        .env("GIT_SEQUENCE_EDITOR", ":");
    let output = super::process::diagnostic_output(command)?;
    let diagnostics = output.diagnostics();
    if state(root)?.is_some() {
        if args.contains(&"--abort") {
            return Err(io::Error::other(diagnostics));
        }
        return Ok(format!(
            "Rebase paused. Resolve conflicts, then continue or abort.\n{diagnostics}"
        ));
    }
    if !output.status.success() {
        return Err(io::Error::other(diagnostics));
    }
    Ok(String::from("Rebase completed"))
}

/// Replay the reviewed commits with no autosquash, automatic stash or other-ref updates.
///
/// # Errors
///
/// Returns an error for a stale preview, unsupported state, checkout collisions or Git failure, including conflicts requiring resolution.
pub fn execute_rebase(preview: &RebasePreview) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if &build_preview(&preview.repository.path, &preview.target, &preview.basis)? != preview {
        return Err(io::Error::other("Rebase inputs changed; preview again"));
    }
    if preview.kind == RebaseKind::AlreadyUpToDate {
        return Ok(String::from("Already based on the selected branch"));
    }
    check_collisions(
        &preview.repository.path,
        std::slice::from_ref(&preview.target.oid),
        &preview
            .commits
            .iter()
            .map(|c| c.oid.clone())
            .collect::<Vec<_>>(),
    )?;
    run(
        &preview.repository.path,
        &[
            "-c",
            "rebase.autoStash=false",
            "rebase",
            "--merge",
            "--strategy=ort",
            "--no-autostash",
            "--no-autosquash",
            "--no-update-refs",
            "--no-rebase-merges",
            "--no-fork-point",
            "--reapply-cherry-picks",
            "--empty=keep",
            "--keep-empty",
            "--",
            &preview.target.oid,
        ],
    )
}

fn validate_active(root: &Path, expected: &CommitBasis) -> io::Result<RebaseState> {
    if &commit_basis(root)? != expected {
        return Err(io::Error::other(
            "Rebase, index or HEAD changed; refresh before continuing",
        ));
    }
    let active = expected
        .rebase
        .as_ref()
        .ok_or_else(|| io::Error::other("No rebase is in progress"))?;
    if !active.supported {
        return Err(io::Error::other("Use Git to continue/abort this unsupported rebase (apply backend, custom todo or autostash)"));
    }
    let branch_tip = super::read_output::complete(
        root,
        &[
            std::ffi::OsStr::new("rev-parse"),
            std::ffi::OsStr::new("--verify"),
            active.branch.as_os_str(),
        ],
        64 * 1024,
    )?;
    if text(&branch_tip) != active.original_head {
        return Err(io::Error::other(
            "The original branch moved during the rebase; use Git to recover",
        ));
    }
    Ok(active.clone())
}

/// Continue after resolutions are staged, retaining messages for replayed commits.
/// A conflict resolved entirely to rebased-onto contents can leave no patch; Git
/// may omit that commit rather than create an empty resolution commit.
///
/// # Errors
///
/// Returns an error for stale/unsupported sequencer state, unresolved conflicts, checkout collisions or Git failure.
pub fn continue_rebase(root: &Path, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let active = validate_active(root, expected)?;
    if snapshot(root)?.changes.iter().any(|c| c.is_conflicted()) {
        return Err(io::Error::other(
            "Resolve and stage all conflicts before continuing the rebase",
        ));
    }
    check_collisions(root, &[active.original_head, active.onto], &active.commits)?;
    run(root, &["rebase", "--continue"])
}

/// Abort after explicit confirmation, protecting untracked/ignored checkout collisions.
///
/// # Errors
///
/// Returns an error for stale/unsupported sequencer state, checkout collisions or Git rejection.
pub fn abort_rebase(root: &Path, expected: &CommitBasis) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let active = validate_active(root, expected)?;
    check_collisions(root, &[active.original_head], &[])?;
    run(root, &["rebase", "--abort"]).map(|_| String::from("Rebase aborted"))
}
