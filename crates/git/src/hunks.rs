//! Complete context-bearing patch review and index-only staging of one hunk.
use desktop_core::{ConflictWorkingContent, FileChange, HunkReview};
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
    let state = super::snapshot(path)?;
    if !state.changes.contains(selected)
        || selected.original_path.is_some()
        || selected.worktree != 'M'
        || !matches!(selected.index, ' ' | 'M')
    {
        return Err(io::Error::other(
            "Hunk staging requires an existing, unrenamed tracked modification",
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
    let args = [
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
        OsStr::new("--"),
        selected.path.as_os_str(),
    ];
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
/// Returns an error for stale reviews, invalid hunk indices, failed patch checks,
/// Git/index-lock rejection or changed working contents after application. Refresh
/// after any write error; no automatic rollback overwrites later edits.
pub fn stage_hunk(review: &HunkReview, index: usize) -> io::Result<String> {
    let _guard = super::WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    let hunk = review.hunks.get(index).ok_or_else(invalid)?;
    let root = &review.repository.path;
    if review_hunks(root, &review.change)? != *review {
        return Err(io::Error::other(
            "Hunk review changed; review again before staging",
        ));
    }
    let patch = format!("{}{hunk}", review.header);
    super::run_write(
        root,
        &[
            "apply",
            "--cached",
            "--check",
            "--whitespace=nowarn",
            "--",
            "-",
        ],
        Some(&patch),
    )?;
    super::run_write(
        root,
        &["apply", "--cached", "--whitespace=nowarn", "--", "-"],
        Some(&patch),
    )
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Hunk staging failed; index may have changed, refresh and inspect: {error}"),
        )
    })?;
    if super::conflicts::working_content(root, &review.change.path)? != review.working {
        return Err(io::Error::other(
            "Working file changed during hunk staging; refresh and inspect",
        ));
    }
    Ok(format!(
        "Staged hunk {} of {}; working file retained",
        index + 1,
        review.change.path.display()
    ))
}
