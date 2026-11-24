//! Initialize a local project without reusing existing Git metadata.
use super::{branches, run_write, WRITE_LOCK};
use std::{
    io,
    path::{Path, PathBuf},
};

fn entry_exists(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn check_location(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        if entry_exists(&ancestor.join(".git"))? {
            return Err(io::Error::other(
                "Destination is already inside Git metadata",
            ));
        }
        // Conservatively protect bare/separate Git directories even with broken
        // HEAD/refs or symlinked components. Never infer absence from Git failure.
        if entry_exists(&ancestor.join("HEAD"))? && entry_exists(&ancestor.join("objects"))? {
            for marker in ["refs", "reftable", "packed-refs"] {
                if entry_exists(&ancestor.join(marker))? {
                    return Err(io::Error::other(
                        "Destination is inside a Git metadata layout",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Initialize a non-bare repository on the explicit initial branch.
/// Accept a new directory with an existing parent, or an existing directory
/// containing ordinary project files. Existing/nested repositories and symlinks
/// are rejected. No project files are created, staged, committed or removed.
/// An initialization failure may leave newly reserved metadata for inspection.
///
/// # Errors
///
/// Returns an error for invalid names/destinations, existing or nested Git metadata, filesystem failure or Git rejection.
pub fn init_repository(destination: &Path, initial_branch: &str) -> io::Result<PathBuf> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if destination.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Destination cannot be empty",
        ));
    }
    let destination = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()?.join(destination)
    };
    let (directory, create) = match std::fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_dir() => (destination.canonicalize()?, false),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must be a directory, not a file or symlink",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let name = destination.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid destination")
            })?;
            let parent = destination
                .parent()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing parent"))?
                .canonicalize()?;
            check_location(&parent)?;
            branches::validate_name(&parent, initial_branch)?;
            (parent.join(name), true)
        }
        Err(error) => return Err(error),
    };
    if !create {
        check_location(&directory)?;
        branches::validate_name(&directory, initial_branch)?;
    } else {
        std::fs::create_dir(&directory)?;
    }
    // Reserve metadata atomically; another initializer must not be silently reused.
    std::fs::create_dir(directory.join(".git"))?;
    run_write(
        &directory,
        &["init", "--initial-branch", initial_branch, "--", "."],
        None,
    )?;
    Ok(directory)
}
