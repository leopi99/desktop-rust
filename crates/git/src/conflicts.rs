//! Review and stage externally edited text conflicts without replacing file contents.
use super::{read_bytes, run_write, snapshot, WRITE_LOCK};
use desktop_core::{ConflictContent, ConflictReview, FileChange};
use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::{self, Read},
    path::Path,
};

const MAX_TEXT: u64 = 8 * 1024 * 1024;

fn content(root: &Path, relative: &Path) -> io::Result<ConflictContent> {
    let mut parent = relative.parent();
    while let Some(component) = parent.filter(|p| !p.as_os_str().is_empty()) {
        match std::fs::symlink_metadata(root.join(component)) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::other(
                    "Conflict paths must not traverse a symlink or non-directory",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        parent = component.parent();
    }
    let path = root.join(relative);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ConflictContent::Deleted)
        }
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return Err(io::Error::other(
            "Resolve symlink, directory and submodule conflicts using Git",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_TEXT {
        return Err(io::Error::other(
            "Review supports regular text files up to 8 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TEXT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TEXT || bytes.contains(&0) {
        return Err(io::Error::other(
            "Resolve binary or oversized file conflicts using Git",
        ));
    }
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    Ok(ConflictContent::Text { bytes, executable })
}

fn marker_size(root: &Path, path: &Path) -> io::Result<usize> {
    let output = read_bytes(
        root,
        &[
            OsStr::new("check-attr"),
            OsStr::new("-z"),
            OsStr::new("conflict-marker-size"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
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
