//! Review and stage externally edited text conflicts without replacing file contents.
use super::{run_write, snapshot, WRITE_LOCK};
use desktop_core::{
    ConflictContent, ConflictReview, ConflictSide, ConflictSideContent, ConflictSidePreview,
    ConflictSideResolutionReview, ConflictWorkingContent, FileChange,
};
use std::{ffi::OsStr, io, path::Path};

const MAX_TEXT: u64 = 8 * 1024 * 1024;

pub(super) fn working_content(root: &Path, relative: &Path) -> io::Result<ConflictWorkingContent> {
    use super::working_files::WorkingFile;
    match super::working_files::inspect(root, relative, MAX_TEXT)? {
        WorkingFile::Missing => Ok(ConflictWorkingContent::Deleted),
        WorkingFile::Regular { bytes, executable } => {
            Ok(ConflictWorkingContent::File { bytes, executable })
        }
        WorkingFile::TooLarge => Err(io::Error::other(
            "Review supports regular files up to 8 MiB",
        )),
        WorkingFile::Symlink(_) | WorkingFile::Unsupported => Err(io::Error::other(
            "Resolve symlink, directory and submodule conflicts using Git",
        )),
    }
}

fn content(root: &Path, relative: &Path) -> io::Result<ConflictContent> {
    match working_content(root, relative)? {
        ConflictWorkingContent::Deleted => Ok(ConflictContent::Deleted),
        ConflictWorkingContent::File { bytes, executable } => {
            if bytes.contains(&0) {
                return Err(io::Error::other(
                    "Resolve binary or oversized file conflicts using Git",
                ));
            }
            Ok(ConflictContent::Text { bytes, executable })
        }
    }
}

fn marker_size(root: &Path, path: &Path) -> io::Result<usize> {
    let output = super::read_output::complete(
        root,
        &[
            OsStr::new("check-attr"),
            OsStr::new("-z"),
            OsStr::new("conflict-marker-size"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        64 * 1024,
    )?;
    let fields: Vec<_> = output.split(|b| *b == 0).collect();
    if fields.len() != 4 || fields[1] != b"conflict-marker-size" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid conflict marker attribute",
        ));
    }
    Ok(std::str::from_utf8(fields[2])
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(7))
}

fn count_markers(bytes: &[u8], size: usize) -> usize {
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let Some(first) = line.first() else {
                return false;
            };
            if !matches!(*first, b'<' | b'=' | b'>' | b'|') {
                return false;
            }
            let count = line.iter().take_while(|b| *b == first).count();
            count >= size && (count == line.len() || line[count].is_ascii_whitespace())
        })
        .count()
}

/// Review the current text or deletion of a still-unmerged file. No writes occur.
/// Marker-like lines are conservatively rejected, including diff3 base markers.
///
/// # Errors
///
/// Returns an error for stale/non-conflicted paths, unsupported contents, invalid metadata or I/O failure.
pub fn inspect_conflict(path: &Path, selected: &FileChange) -> io::Result<ConflictReview> {
    let state = snapshot(path)?;
    if !selected.is_conflicted() || !state.changes.contains(selected) {
        return Err(io::Error::other(
            "Conflict status changed; refresh and select the file again",
        ));
    }
    let content = content(&state.repository.path, &selected.path)?;
    let marker_size = marker_size(&state.repository.path, &selected.path)?;
    let marker_count = match &content {
        ConflictContent::Text { bytes, .. } => count_markers(bytes, marker_size),
        ConflictContent::Deleted => 0,
    };
    Ok(ConflictReview {
        repository: state.repository,
        change: selected.clone(),
        basis: state.commit_basis,
        content,
        marker_size,
        marker_count,
    })
}

/// Stage only the reviewed file after checking the index/HEAD, file and attributes
/// again. Never rewrite the working tree or resolve other files automatically.
/// External edits can still race between this preflight and Git acquiring locks.
///
/// # Errors
///
/// Returns an error if the reviewed basis/content changed, markers remain or Git rejects staging.
pub fn resolve_conflict(review: &ConflictReview) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let current = inspect_conflict(&review.repository.path, &review.change)?;
    if &current != review {
        return Err(io::Error::other(
            "File, index, HEAD or attributes changed; review the conflict again",
        ));
    }
    if current.marker_count != 0 {
        return Err(io::Error::other(
            "Conflict markers remain; edit the file and review it again",
        ));
    }
    run_write(
        &review.repository.path,
        &[
            OsStr::new("add"),
            OsStr::new("--all"),
            OsStr::new("--"),
            review.change.path.as_os_str(),
        ],
        None,
    )?;
    Ok(format!(
        "Marked {} as resolved",
        review.change.path.display()
    ))
}

/// Inspect stage 2 (ours) or 3 (theirs) without changing the index or working file.
/// Text retains a 64 KiB display prefix; binary blobs expose size only. Complete
/// blobs are capped at 8 MiB. Missing sides represent deletion. Git stage meaning
/// is preserved during rebase; this preview is not a resolution/write token.
///
/// # Errors
///
/// Returns an error for stale/non-conflicted paths, malformed/oversized index or
/// blob output, unsupported entry modes, Git failure or Unix read deadlines.
pub fn inspect_conflict_side(
    path: &Path,
    selected: &FileChange,
    side: ConflictSide,
) -> io::Result<ConflictSidePreview> {
    let state = snapshot(path)?;
    if !selected.is_conflicted()
        || !state.changes.contains(selected)
        || selected.original_path.is_some()
    {
        return Err(io::Error::other(
            "Refresh and select a supported unmerged path before inspecting an index side",
        ));
    }
    let root = &state.repository.path;
    let raw = super::read_output::complete(
        root,
        &[
            OsStr::new("ls-files"),
            OsStr::new("--unmerged"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            selected.path.as_os_str(),
        ],
        64 * 1024,
    )?;
    let stage = match side {
        ConflictSide::Ours => b'2',
        ConflictSide::Theirs => b'3',
    };
    let mut seen = std::collections::HashSet::new();
    let mut entry = None;
    for record in raw
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid unmerged index record")
            })?;
        let fields: Vec<_> = record[..separator].split(|byte| *byte == b' ').collect();
        if fields.len() != 3
            || !matches!(fields[2], b"1" | b"2" | b"3")
            || !seen.insert(fields[2][0])
            || super::file_path(&record[separator + 1..])? != selected.path
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid unmerged index entry",
            ));
        }
        let oid = std::str::from_utf8(fields[1])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        super::history::validate_oid(oid)?;
        if fields[2][0] == stage {
            if !matches!(fields[0], b"100644" | b"100755") {
                return Err(io::Error::other(
                    "Use Git to inspect symlink or submodule conflict sides",
                ));
            }
            entry = Some((oid.to_owned(), fields[0] == b"100755"));
        }
    }
    if seen.is_empty() {
        return Err(io::Error::other("The selected path is no longer unmerged"));
    }
    let (oid, executable, content) = match entry {
        None => (None, false, ConflictSideContent::Deleted),
        Some((oid, executable)) => {
            let bytes =
                super::read_output::complete(root, &["cat-file", "blob", &oid], MAX_TEXT as usize)?;
            let content = if bytes.contains(&0) {
                ConflictSideContent::Binary { size: bytes.len() }
            } else {
                ConflictSideContent::Text {
                    preview: String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)])
                        .into_owned(),
                    truncated: bytes.len() > 64 * 1024,
                }
            };
            (Some(oid), executable, content)
        }
    };
    if super::commit_basis(root)? != state.commit_basis || !super::changes(root)?.contains(selected)
    {
        return Err(io::Error::other(
            "Conflict state changed while reading the side; refresh and review again",
        ));
    }
    Ok(ConflictSidePreview {
        repository: state.repository,
        change: selected.clone(),
        side,
        oid,
        executable,
        content,
    })
}

pub(super) fn conversion_policy(root: &Path, path: &Path) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let names = [
        "filter",
        "ident",
        "working-tree-encoding",
        "text",
        "eol",
        "conflict-marker-size",
    ];
    let mut args = vec![OsStr::new("check-attr"), OsStr::new("-z")];
    args.extend(names.iter().map(OsStr::new));
    args.extend([OsStr::new("--"), path.as_os_str()]);
    let attributes = super::read_output::complete(root, &args, 64 * 1024)?;
    let fields: Vec<_> = attributes
        .strip_suffix(b"\0")
        .unwrap_or(&attributes)
        .split(|byte| *byte == 0)
        .collect();
    if fields.len() != names.len() * 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid side-resolution attributes",
        ));
    }
    for (record, name) in fields.as_chunks::<3>().0.iter().zip(names) {
        if super::file_path(record[0])? != path || record[1] != name.as_bytes() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid side-resolution attribute record",
            ));
        }
        if name != "conflict-marker-size" && !matches!(record[2], b"unspecified" | b"unset") {
            return Err(io::Error::other(
                "Use external Git for files with filters, text/EOL, ident or encoding conversions",
            ));
        }
    }
    let autocrlf = super::read_output::optional(
        root,
        &["config", "--null", "--type=bool", "--get", "core.autocrlf"],
        64 * 1024,
    )
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Cannot establish conversion policy; use external Git: {error}"),
        )
    })?;
    if autocrlf.as_deref().is_some_and(|value| value != b"false\0") {
        return Err(io::Error::other(
            "Use external Git for files with automatic line-ending conversion",
        ));
    }
    let settings = super::read_output::optional(root, &["config", "--null", "--get-regexp", "^core\\.(autocrlf|eol|safecrlf|filemode|checkroundtripencoding|sparsecheckout|sparsecheckoutcone|hookspath)$"], 64 * 1024)?.unwrap_or_default();
    Ok((attributes, settings))
}

/// Review an explicit side replacement/deletion without writing. Only regular
/// files up to 8 MiB and paths without checkout conversions are supported.
/// Only UU, AA, UD and DU statuses support side writes. AU/UA/DD require Git.
/// Externally resolved marker-free UU/AA text must use ordinary conflict review.
///
/// # Errors
///
/// Returns an error for unsupported/changed state, conversions, externally resolved
/// text, oversized data or Git/I/O failure. No destination is overwritten here.
pub fn review_conflict_side_resolution(
    path: &Path,
    selected: &FileChange,
    side: ConflictSide,
) -> io::Result<ConflictSideResolutionReview> {
    if !matches!(
        (selected.index, selected.worktree),
        ('U', 'U') | ('A', 'A') | ('U', 'D') | ('D', 'U')
    ) {
        return Err(io::Error::other(
            "Use Git for added-by-one-side, both-deleted or coordinated rename conflicts; an absent index side alone does not authorize deletion",
        ));
    }
    let preview = inspect_conflict_side(path, selected, side)?;
    let root = &preview.repository.path;
    let basis = super::commit_basis(root)?;
    let working = working_content(root, &selected.path)?;
    if matches!((selected.index, selected.worktree), ('U', 'U') | ('A', 'A')) {
        let externally_resolved = match &working {
            ConflictWorkingContent::Deleted => true,
            ConflictWorkingContent::File { bytes, .. } => {
                !bytes.contains(&0) && count_markers(bytes, marker_size(root, &selected.path)?) == 0
            }
        };
        if externally_resolved {
            return Err(io::Error::other("The file appears externally resolved; review and stage the edited contents instead"));
        }
    }
    let (attributes, settings) = conversion_policy(root, &selected.path)?;
    let target = match &preview.oid {
        None => ConflictWorkingContent::Deleted,
        Some(oid) => ConflictWorkingContent::File {
            bytes: super::read_output::complete(
                root,
                &["cat-file", "blob", oid],
                MAX_TEXT as usize,
            )?,
            executable: preview.executable,
        },
    };
    if inspect_conflict_side(root, selected, side)? != preview
        || super::commit_basis(root)? != basis
        || working_content(root, &selected.path)? != working
    {
        return Err(io::Error::other(
            "Conflict changed during side review; review again",
        ));
    }
    Ok(ConflictSideResolutionReview {
        preview,
        basis,
        working,
        selected: target,
        attributes,
        settings,
    })
}

/// Apply only an explicitly confirmed fresh side review, replacing/deleting the
/// named working file and staging it. Git's configured checkout hooks remain active.
/// Preflight checks do not fence concurrent external clients. Compound-operation
/// failures can leave the working file changed; inspect/refresh before recovery.
///
/// # Errors
///
/// Returns an error for stale/unsupported reviews, Git checkout/remove/add failure
/// or a failed postcondition. No automatic rollback overwrites subsequent edits.
pub fn apply_conflict_side_resolution(review: &ConflictSideResolutionReview) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let root = &review.preview.repository.path;
    let selected = &review.preview.change;
    if review_conflict_side_resolution(root, selected, review.preview.side)? != *review {
        return Err(io::Error::other(
            "File, index, HEAD, attributes or side changed; review again before applying",
        ));
    }
    apply_reviewed_side(review).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Side resolution failed; file/index may have changed, refresh and inspect: {error}"
            ),
        )
    })
}

// Called only after freshness validation, while the caller holds WRITE_LOCK.
// Every error here is potentially after a Git write, including failed reads.
fn apply_reviewed_side(review: &ConflictSideResolutionReview) -> io::Result<String> {
    let root = &review.preview.repository.path;
    let selected = &review.preview.change;
    let result = if review.preview.oid.is_none() {
        run_write(
            root,
            &[
                OsStr::new("rm"),
                OsStr::new("--"),
                selected.path.as_os_str(),
            ],
            None,
        )
    } else {
        let side = match review.preview.side {
            ConflictSide::Ours => "--ours",
            ConflictSide::Theirs => "--theirs",
        };
        run_write(
            root,
            &[
                OsStr::new("checkout"),
                OsStr::new(side),
                OsStr::new("--"),
                selected.path.as_os_str(),
            ],
            None,
        )?;
        let (attributes, settings) = conversion_policy(root, &selected.path)?;
        if super::commit_basis(root)? != review.basis
            || working_content(root, &selected.path)? != review.selected
            || attributes != review.attributes
            || settings != review.settings
        {
            return Err(io::Error::other(
                "State changed after side checkout; file may be replaced, inspect before staging",
            ));
        }
        run_write(
            root,
            &[
                OsStr::new("add"),
                OsStr::new("--"),
                selected.path.as_os_str(),
            ],
            None,
        )
    };
    result?;
    let staged = super::read_output::complete(
        root,
        &[
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            selected.path.as_os_str(),
        ],
        64 * 1024,
    )?;
    let expected = match &review.preview.oid {
        None => Vec::new(),
        Some(oid) => {
            let mut expected = format!(
                "{} {oid} 0\t",
                if review.preview.executable {
                    "100755"
                } else {
                    "100644"
                }
            )
            .into_bytes();
            expected.extend_from_slice(selected.path.as_os_str().as_encoded_bytes());
            expected.push(0);
            expected
        }
    };
    if staged != expected || working_content(root, &selected.path)? != review.selected {
        return Err(io::Error::other(
            "Side resolution postcondition failed; inspect file/index before continuing",
        ));
    }
    Ok(format!(
        "Applied reviewed {:?} side to {} and staged the result",
        review.preview.side,
        selected.path.display()
    ))
}
