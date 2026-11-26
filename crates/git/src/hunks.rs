//! Complete context-bearing reviews and index-only updates of selected hunks.
mod lines;
use desktop_core::{ConflictWorkingContent, FileChange, HunkLine, HunkReview, HunkSide};
use std::{ffi::OsStr, io, path::Path};

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Unsupported or malformed complete text patch; use full-file staging",
    )
}

fn range(value: &str, prefix: char) -> io::Result<usize> {
    let value = value.strip_prefix(prefix).ok_or_else(invalid)?;
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    let _: usize = start.parse().map_err(|_| invalid())?;
    count.parse().map_err(|_| invalid())
}

fn validate_hunk(hunk: &str) -> io::Result<()> {
    let mut lines = hunk.split_inclusive('\n');
    let fields: Vec<_> = lines.next().ok_or_else(invalid)?.split(' ').collect();
    if fields.len() < 4 || fields[0] != "@@" || !fields[3].starts_with("@@") {
        return Err(invalid());
    }
    let expected = (range(fields[1], '-')?, range(fields[2], '+')?);
    let (mut old, mut new, mut changed) = (0usize, 0usize, false);
    for line in lines {
        if !line.ends_with('\n') {
            return Err(invalid());
        }
        match line.as_bytes().first() {
            Some(b' ') => {
                old += 1;
                new += 1;
            }
            Some(b'-') => {
                old += 1;
                changed = true;
            }
            Some(b'+') => {
                new += 1;
                changed = true;
            }
            Some(b'\\') if line == "\\ No newline at end of file\n" => {}
            _ => return Err(invalid()),
        }
    }
    if !changed || (old, new) != expected {
        return Err(invalid());
    }
    Ok(())
}

fn parse(patch: String) -> io::Result<(String, Vec<String>)> {
    let first = patch
        .find("\n@@ ")
        .map(|offset| offset + 1)
        .ok_or_else(invalid)?;
    let header = &patch[..first];
    let lines: Vec<_> = header.lines().collect();
    if lines.len() != 4
        || !lines[0].starts_with("diff --git ")
        || !lines[1].starts_with("index ")
        || !matches!(lines[1].split(' ').next_back(), Some("100644" | "100755"))
        || !lines[2].starts_with("--- ")
        || !lines[3].starts_with("+++ ")
    {
        return Err(invalid());
    }
    let mut hunks = Vec::new();
    let mut offsets = vec![first];
    offsets.extend(
        patch[first..]
            .match_indices("\n@@ ")
            .map(|(offset, _)| first + offset + 1),
    );
    offsets.push(patch.len());
    if offsets.len() > 129 {
        return Err(invalid());
    }
    for pair in offsets.windows(2) {
        let hunk = &patch[pair[0]..pair[1]];
        validate_hunk(hunk)?;
        hunks.push(hunk.to_owned());
    }
    Ok((header.to_owned(), hunks))
}

/// Review complete unstaged hunks for one existing regular UTF-8 text file.
/// Only unrenamed tracked modifications are supported, without content conversions.
/// Patch input is capped at 512 KiB and 128 hunks; file bytes at 8 MiB. This is
/// distinct from the possibly clipped ordinary display diff.
///
/// # Errors
///
/// Returns an error for stale/unsupported state, binary/non-UTF-8/oversized data,
/// conversion settings, malformed patches or Git/I/O failure. No writes occur.
pub fn review_hunks(path: &Path, selected: &FileChange) -> io::Result<HunkReview> {
    review(path, selected, HunkSide::Unstaged)
}

/// Review complete staged hunks of one existing regular UTF-8 text file.
/// Each reviewed block can be removed from the index without writing the working
/// file. Limits and conversion exclusions match the unstaged review.
///
/// # Errors
///
/// Returns an error for stale/unsupported status, active operations, non-text or
/// oversized data, conversions, malformed patch output or Git/I/O failure.
pub fn review_staged_hunks(path: &Path, selected: &FileChange) -> io::Result<HunkReview> {
    review(path, selected, HunkSide::Staged)
}

fn review(path: &Path, selected: &FileChange, side: HunkSide) -> io::Result<HunkReview> {
    let state = super::snapshot(path)?;
    if !state.changes.contains(selected)
        || selected.original_path.is_some()
        || !match side {
            HunkSide::Unstaged => selected.worktree == 'M' && matches!(selected.index, ' ' | 'M'),
            HunkSide::Staged => selected.index == 'M' && matches!(selected.worktree, ' ' | 'M'),
        }
    {
        return Err(io::Error::other(
            "Hunk operations require an existing, unrenamed tracked modification on the chosen side",
        ));
    }
    let root = &state.repository.path;
    super::merge::ensure_idle(root)?;
    let working = super::conflicts::working_content(root, &selected.path)?;
    if !matches!(&working, ConflictWorkingContent::File {bytes, ..} if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok())
    {
        return Err(invalid());
    }
    let (attributes, settings) = super::conflicts::conversion_policy(root, &selected.path)?;
    let mut args = vec![
        OsStr::new("-c"),
        OsStr::new("core.quotePath=true"),
        OsStr::new("-c"),
        OsStr::new("diff.suppressBlankEmpty=false"),
        OsStr::new("diff"),
        OsStr::new("--no-ext-diff"),
        OsStr::new("--no-textconv"),
        OsStr::new("--no-color"),
        OsStr::new("--no-renames"),
        OsStr::new("--no-relative"),
        OsStr::new("--full-index"),
        OsStr::new("--unified=3"),
        OsStr::new("--inter-hunk-context=0"),
        OsStr::new("--diff-algorithm=myers"),
        OsStr::new("--no-indent-heuristic"),
        OsStr::new("--src-prefix=a/"),
        OsStr::new("--dst-prefix=b/"),
        OsStr::new("--output-indicator-new=+"),
        OsStr::new("--output-indicator-old=-"),
        OsStr::new("--output-indicator-context= "),
    ];
    if side == HunkSide::Staged {
        args.push(OsStr::new("--cached"));
    }
    args.extend([OsStr::new("--"), selected.path.as_os_str()]);
    let raw = super::read_output::complete(root, &args, 512 * 1024)?;
    let (header, hunks) = parse(String::from_utf8(raw).map_err(|_| invalid())?)?;
    let (current_attributes, current_settings) =
        super::conflicts::conversion_policy(root, &selected.path)?;
    if super::commit_basis(root)? != state.commit_basis
        || super::conflicts::working_content(root, &selected.path)? != working
        || !super::changes(root)?.contains(selected)
        || current_attributes != attributes
        || current_settings != settings
    {
        return Err(io::Error::other(
            "File, index or policy changed during hunk review; review again",
        ));
    }
    Ok(HunkReview {
        side,
        repository: state.repository,
        change: selected.clone(),
        basis: state.commit_basis,
        working,
        attributes,
        settings,
        header,
        hunks,
    })
}

/// Stage one explicitly selected hunk from a fresh complete review, index only.
/// Git applies the patch under its index lock. Working files are not written by
/// this command; preflight does not fence concurrent external clients or hooks.
///
/// # Errors
///
/// Returns an error for wrong-side/stale reviews, invalid hunk indices, failed patch checks,
/// Git/index-lock rejection or changed working contents after application. Refresh
/// after any write error; no automatic rollback overwrites later edits.
pub fn stage_hunk(review: &HunkReview, index: usize) -> io::Result<String> {
    stage_hunks(review, &[index])
}

/// Remove one freshly reviewed staged hunk from the index, working files intact.
/// Only staged-side reviews are accepted; forward-stage reviews cannot authorize
/// this reverse operation. External clients/hooks can still race preflight.
///
/// # Errors
///
/// Returns an error for wrong-side/stale reviews, invalid indices, patch-check or
/// index-write failures, or changed working contents after the operation. Refresh
/// after a write error; no automatic rollback replaces later edits.
pub fn unstage_hunk(review: &HunkReview, index: usize) -> io::Result<String> {
    unstage_hunks(review, &[index])
}

/// Stage a nonempty selection of complete hunks with one cached patch application.
/// Input order is normalized to patch order; duplicate or invalid indices are refused.
/// The complete review is revalidated once and working files are not written.
///
/// # Errors
///
/// Returns an error for an empty/duplicate/invalid selection, wrong-side/stale review,
/// patch-check/index-write failure or changed working contents. External clients and
/// hooks can race preflight; refresh after a write error, without automatic rollback.
pub fn stage_hunks(review: &HunkReview, indices: &[usize]) -> io::Result<String> {
    apply_hunks(review, indices, HunkSide::Unstaged)
}

/// Unstage selected complete hunks with one reverse cached patch application.
/// Unselected index blocks, other paths and working files remain outside the patch.
/// Input order is normalized; empty selections and duplicate/invalid indices fail.
///
/// # Errors
///
/// Returns an error for invalid selection, wrong-side/stale review, patch-check or
/// index-write failure, or changed working contents. Preflight does not fence
/// external clients/hooks; no automatic rollback overwrites later edits.
pub fn unstage_hunks(review: &HunkReview, indices: &[usize]) -> io::Result<String> {
    apply_hunks(review, indices, HunkSide::Staged)
}

fn apply_hunks(review: &HunkReview, indices: &[usize], side: HunkSide) -> io::Result<String> {
    if review.side != side {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Review the matching staged or unstaged side before changing the index",
        ));
    }
    if indices.is_empty() || indices.len() > review.hunks.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Select unique valid hunks before changing the index",
        ));
    }
    let ordered: std::collections::BTreeSet<_> = indices.iter().copied().collect();
    if ordered.is_empty()
        || ordered.len() != indices.len()
        || ordered.iter().any(|index| *index >= review.hunks.len())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Select unique valid hunks before changing the index",
        ));
    }
    let mut patch = review.header.clone();
    for index in ordered {
        patch.push_str(&review.hunks[index]);
    }
    apply_patch(review, side, &patch, indices.len(), "hunk(s)")
}

/// Stage selected added/deleted rows from a complete unstaged review.
/// Coordinates include the hunk header as row zero. Context and no-newline marker
/// rows cannot be selected. Partial selection of no-newline hunks is refused.
///
/// # Errors
///
/// Returns an error for invalid/duplicate/empty selection, line-control limits,
/// unsupported EOF selection, stale/wrong-side review, patch or index failure.
pub fn stage_lines(review: &HunkReview, selection: &[HunkLine]) -> io::Result<String> {
    let patch = lines::patch(review, selection)?;
    apply_patch(
        review,
        HunkSide::Unstaged,
        &patch,
        selection.len(),
        "line(s)",
    )
}

/// Unstage selected rows from a complete staged review, without writing working files.
/// Unselected additions remain context; unselected deletions are omitted from the
/// reverse patch. The same limits and conservative EOF rules as stage_lines apply.
///
/// # Errors
///
/// Returns an error for invalid/duplicate/empty selection, line-control limits,
/// unsupported EOF selection, stale/wrong-side review, patch or index failure.
pub fn unstage_lines(review: &HunkReview, selection: &[HunkLine]) -> io::Result<String> {
    let patch = lines::patch(review, selection)?;
    apply_patch(review, HunkSide::Staged, &patch, selection.len(), "line(s)")
}

fn apply_patch(
    review: &HunkReview,
    side: HunkSide,
    patch: &str,
    count: usize,
    unit: &str,
) -> io::Result<String> {
    if review.side != side {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Review the matching side before updating the index",
        ));
    }
    let _guard = super::WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let root = &review.repository.path;
    if self::review(root, &review.change, side)? != *review {
        return Err(io::Error::other(
            "Hunk review changed; review the matching side again before updating the index",
        ));
    }
    let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
    if side == HunkSide::Staged {
        args.push("--reverse");
    }
    let mut check = args.clone();
    check.extend(["--check", "--", "-"]);
    super::run_write(root, &check, Some(patch))?;
    args.extend(["--", "-"]);
    super::run_write(root, &args, Some(patch)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Hunk index update failed; index may have changed, refresh and inspect: {error}"
            ),
        )
    })?;
    if super::conflicts::working_content(root, &review.change.path)? != review.working {
        return Err(io::Error::other(
            "Working file changed during hunk index update; refresh and inspect",
        ));
    }
    Ok(format!(
        "{} {} {} of {}; working file retained",
        if side == HunkSide::Staged {
            "Unstaged"
        } else {
            "Staged"
        },
        count,
        unit,
        review.change.path.display()
    ))
}
