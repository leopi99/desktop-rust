//! Immutable commit history and literal-path diffs. No checkout or index writes.
use super::{file_path, read_output as read};
use desktop_core::{CommitDetails, CommitFile, CommitSummary};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::Path,
};

const PAGE_SIZE: usize = 100;
const MESSAGE_LIMIT: usize = 64 * 1024;
const PATCH_LIMIT: usize = 256 * 1024;
const METADATA_LIMIT: usize = 1024 * 1024;
const FILE_LIST_LIMIT: usize = 4 * 1024 * 1024;
const FILE_COUNT_LIMIT: usize = 10_000;

pub(super) fn validate_oid(oid: &str) -> io::Result<()> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Use the full commit object ID",
        ));
    }
    Ok(())
}

fn summary(fields: &[&[u8]]) -> io::Result<CommitSummary> {
    let oid = String::from_utf8_lossy(fields[0]).into_owned();
    validate_oid(&oid)?;
    Ok(CommitSummary {
        oid,
        subject: String::from_utf8_lossy(fields[1]).into_owned(),
        author: String::from_utf8_lossy(fields[2]).into_owned(),
        authored_at: String::from_utf8_lossy(fields[3]).into_owned(),
    })
}

/// Read up to 100 commits anchored at a fixed tip, so paging survives HEAD changes.
///
/// # Errors
///
/// Returns an error for invalid IDs, malformed/oversized metadata or Git failure.
/// Unix commands also have a per-process thirty-second deadline.
pub fn commit_history(path: &Path, head: &str, offset: usize) -> io::Result<Vec<CommitSummary>> {
    validate_oid(head)?;
    let bytes = read::complete(
        path,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-color",
            "--no-patch",
            "-z",
            "--format=%H%x00%s%x00%an%x00%aI",
            &format!("--max-count={PAGE_SIZE}"),
            &format!("--skip={offset}"),
            head,
            "--",
        ],
        METADATA_LIMIT,
    )?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let fields: Vec<_> = bytes
        .strip_suffix(b"\0")
        .unwrap_or(&bytes)
        .split(|b| *b == 0)
        .take(PAGE_SIZE * 4 + 1)
        .collect();
    if fields.len() > PAGE_SIZE * 4 || fields.len() % 4 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid commit history record",
        ));
    }
    fields
        .as_chunks::<4>()
        .0
        .iter()
        .map(|fields| summary(fields))
        .collect()
}

fn files(path: &Path, oid: &str, parents: &[String]) -> io::Result<Vec<CommitFile>> {
    let bytes = if let Some(parent) = parents.first() {
        read::complete(
            path,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--name-status",
                "--find-renames",
                "-z",
                parent,
                oid,
                "--",
            ],
            FILE_LIST_LIMIT,
        )?
    } else {
        read::complete(
            path,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--no-ext-diff",
                "--no-textconv",
                "--name-status",
                "--find-renames",
                "-r",
                "-z",
                oid,
                "--",
            ],
            FILE_LIST_LIMIT,
        )?
    };
    parse_files(&bytes)
}

fn parse_files(bytes: &[u8]) -> io::Result<Vec<CommitFile>> {
    let mut fields = bytes.split(|b| *b == 0).filter(|field| !field.is_empty());
    let mut result = Vec::new();
    while let Some(kind) = fields.next() {
        if result.len() == FILE_COUNT_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Commit exceeds the supported file-count limit",
            ));
        }
        let status = kind
            .first()
            .copied()
            .ok_or_else(|| io::Error::other("Missing committed file status"))?;
        if !matches!(status, b'A' | b'C' | b'D' | b'M' | b'R' | b'T') {
            return Err(io::Error::other("Invalid committed file status"));
        }
        let first = file_path(
            fields
                .next()
                .ok_or_else(|| io::Error::other("Missing committed path"))?,
        )?;
        let (original_path, path) = if matches!(status, b'R' | b'C') {
            (
                Some(first),
                file_path(
                    fields
                        .next()
                        .ok_or_else(|| io::Error::other("Missing rename destination"))?,
                )?,
            )
        } else {
            (None, first)
        };
        result.push(CommitFile {
            path,
            original_path,
            status: char::from(status),
        });
    }
    Ok(result)
}

/// Read a commit message and file list. Merge commits use the first parent.
///
/// # Errors
///
/// Returns an error for invalid/missing commits, oversized metadata/file lists,
/// more than 10,000 changed files or Git failure. Unix reads also have deadlines.
pub fn commit_details(path: &Path, oid: &str) -> io::Result<CommitDetails> {
    let (commit, parents) = metadata(path, oid)?;
    let files = files(path, oid, &parents)?;
    let message = read::preview(
        path,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "show",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-color",
            "--no-patch",
            "--format=%B",
            oid,
            "--",
        ],
        MESSAGE_LIMIT,
        "Commit message",
    )?;
    Ok(CommitDetails {
        commit,
        parents,
        message,
        files,
    })
}

fn metadata(path: &Path, oid: &str) -> io::Result<(CommitSummary, Vec<String>)> {
    validate_oid(oid)?;
    let bytes = read::complete(
        path,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "show",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-color",
            "--no-patch",
            "--format=%H%x00%s%x00%an%x00%aI%x00%P",
            oid,
            "--",
        ],
        METADATA_LIMIT,
    )?;
    let fields: Vec<_> = bytes.splitn(5, |b| *b == 0).collect();
    if fields.len() != 5 {
        return Err(io::Error::other("Invalid commit detail record"));
    }
    let commit = summary(&fields[..4])?;
    let parents: Vec<_> = String::from_utf8_lossy(fields[4])
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    for parent in &parents {
        validate_oid(parent)?;
    }
    Ok((commit, parents))
}

/// Show a single committed change without consulting or modifying the working tree.
///
/// # Errors
///
/// Returns an error for invalid commits/paths, oversized metadata/file lists,
/// Git failure or a Unix read deadline. Oversized patches yield marked prefixes.
pub fn commit_file_diff(path: &Path, oid: &str, selected: &CommitFile) -> io::Result<String> {
    let (_, parents) = metadata(path, oid)?;
    let committed_files = files(path, oid, &parents)?;
    if !committed_files.contains(selected) {
        return Err(io::Error::other(
            "Path does not belong to the selected commit",
        ));
    }
    let mut args: Vec<OsString> = if let Some(parent) = parents.first() {
        [
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--find-renames",
        ]
        .map(OsString::from)
        .to_vec()
        .into_iter()
        .chain([parent.into(), oid.into()])
        .collect()
    } else {
        [
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--find-renames",
            "-p",
            "-r",
            oid,
        ]
        .map(OsString::from)
        .to_vec()
    };
    args.push(OsString::from("--"));
    args.push(selected.path.as_os_str().to_owned());
    if let Some(original) = &selected.original_path {
        args.push(original.as_os_str().to_owned());
    }
    let refs: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    read::preview(path, &refs, PATCH_LIMIT, "Committed diff")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_count_limit_rejects_incomplete_lists_instead_of_returning_a_prefix() {
        let mut bytes = Vec::new();
        for index in 0..FILE_COUNT_LIMIT {
            bytes.extend_from_slice(format!("A\0path-{index}\0").as_bytes());
        }
        assert_eq!(parse_files(&bytes).unwrap().len(), FILE_COUNT_LIMIT);
        bytes.extend_from_slice(b"A\0one-too-many\0");
        assert_eq!(
            parse_files(&bytes).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
