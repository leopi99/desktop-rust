//! Immutable commit history and literal-path diffs. No checkout or index writes.
use super::{file_path, read_bytes};
use desktop_core::{CommitDetails, CommitFile, CommitSummary};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::Path,
};

const PAGE_SIZE: usize = 100;
const MESSAGE_LIMIT: usize = 64 * 1024;
const PATCH_LIMIT: usize = 256 * 1024;

fn validate_oid(oid: &str) -> io::Result<()> {
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

fn preview(bytes: &[u8], limit: usize, label: &str) -> String {
    let end = bytes.len().min(limit);
    format!(
        "{}{}",
        String::from_utf8_lossy(&bytes[..end]),
        if end < bytes.len() {
            format!("\n[{label} preview truncated]")
        } else {
            String::new()
        }
    )
}

/// Read up to 100 commits anchored at a fixed tip, so paging survives HEAD changes.
pub fn commit_history(path: &Path, head: &str, offset: usize) -> io::Result<Vec<CommitSummary>> {
    validate_oid(head)?;
    let bytes = read_bytes(
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
    )?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let fields: Vec<_> = bytes
        .strip_suffix(b"\0")
        .unwrap_or(&bytes)
        .split(|b| *b == 0)
        .collect();
    if fields.len() % 4 != 0 {
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
        read_bytes(
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
        )?
    } else {
        read_bytes(
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
        )?
    };
    let mut fields = bytes.split(|b| *b == 0).filter(|field| !field.is_empty());
    let mut result = Vec::new();
    while let Some(kind) = fields.next() {
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
pub fn commit_details(path: &Path, oid: &str) -> io::Result<CommitDetails> {
    validate_oid(oid)?;
    let bytes = read_bytes(
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
            "--format=%H%x00%s%x00%an%x00%aI%x00%P%x00%B",
            oid,
            "--",
        ],
    )?;
    let fields: Vec<_> = bytes.splitn(6, |b| *b == 0).collect();
    if fields.len() != 6 {
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
    let files = files(path, oid, &parents)?;
    Ok(CommitDetails {
        commit,
        parents,
        message: preview(fields[5], MESSAGE_LIMIT, "Commit message"),
        files,
    })
}

/// Show a single committed change without consulting or modifying the working tree.
pub fn commit_file_diff(path: &Path, oid: &str, selected: &CommitFile) -> io::Result<String> {
    let details = commit_details(path, oid)?;
    if !details.files.contains(selected) {
        return Err(io::Error::other(
            "Path does not belong to the selected commit",
        ));
    }
    let mut args: Vec<OsString> = if let Some(parent) = details.parents.first() {
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
    let patch = read_bytes(path, &refs)?;
    Ok(preview(&patch, PATCH_LIMIT, "Committed diff"))
}
