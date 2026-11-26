//! Bounded working-file inspection; Unix leaf opens do not follow symlinks.
use std::{
    fs::OpenOptions,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub(super) enum WorkingFile {
    Missing,
    Symlink(PathBuf),
    Unsupported,
    TooLarge,
    Regular { bytes: Vec<u8>, executable: bool },
}

fn parents(root: &Path, relative: &Path) -> io::Result<()> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Use a normal repository-relative working-file path",
        ));
    }
    let mut parent = relative.parent();
    while let Some(component) = parent.filter(|path| !path.as_os_str().is_empty()) {
        match std::fs::symlink_metadata(root.join(component)) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::other(
                    "Working-file paths must not traverse symlinks or non-directories",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        parent = component.parent();
    }
    Ok(())
}

fn bounded(reader: impl Read, maximum: u64) -> io::Result<Option<Vec<u8>>> {
    let budget = maximum.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Working-file read limit overflow",
        )
    })?;
    let mut bytes = Vec::new();
    reader.take(budget).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= maximum).then_some(bytes))
}

// Ancestor checks are preflight, not descriptor-relative fencing against external
// renames. Unix O_NOFOLLOW protects the leaf; Windows retains a best-effort check.
// O_NONBLOCK plus descriptor type checks reject a raced FIFO/device before reads.
pub(super) fn inspect(root: &Path, relative: &Path, maximum: u64) -> io::Result<WorkingFile> {
    parents(root, relative)?;
    let path = root.join(relative);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(WorkingFile::Missing),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Ok(WorkingFile::Symlink(std::fs::read_link(path)?));
    }
    if !metadata.is_file() {
        return Ok(WorkingFile::Unsupported);
    }
    if metadata.len() > maximum {
        return Ok(WorkingFile::TooLarge);
    }
    read_regular(root, relative, maximum)
}

fn read_regular(root: &Path, relative: &Path, maximum: u64) -> io::Result<WorkingFile> {
    let path = root.join(relative);
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Ok(WorkingFile::Unsupported);
    }
    if metadata.len() > maximum {
        return Ok(WorkingFile::TooLarge);
    }
    parents(root, relative)?;
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    match bounded(file, maximum)? {
        Some(bytes) => Ok(WorkingFile::Regular { bytes, executable }),
        None => Ok(WorkingFile::TooLarge),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct Endless(Rc<Cell<usize>>);
    impl Read for Endless {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(b'x');
            self.0.set(self.0.get() + buffer.len());
            Ok(buffer.len())
        }
    }
    #[test]
    fn growing_streams_cannot_read_past_the_limit_plus_one_probe_byte() {
        let consumed = Rc::new(Cell::new(0));
        assert!(bounded(Endless(consumed.clone()), 1024).unwrap().is_none());
        assert_eq!(consumed.get(), 1025);
        assert_eq!(bounded(&b"exact"[..], 5).unwrap().unwrap(), b"exact");
        assert!(bounded(&b"excess"[..], 5).unwrap().is_none());
        assert_eq!(bounded(&b""[..], 0).unwrap().unwrap(), b"");
        assert!(bounded(&b""[..], u64::MAX).is_err());
    }
    struct Failed;
    impl Read for Failed {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("fixture read failed"))
        }
    }
    #[test]
    fn a_late_read_failure_never_returns_a_valid_prefix() {
        assert!(bounded((&b"prefix"[..]).chain(Failed), 1024).is_err());
    }
    #[test]
    fn empty_absolute_and_parent_paths_are_refused_before_filesystem_access() {
        let root = std::env::temp_dir();
        for path in [
            Path::new(""),
            Path::new(".."),
            Path::new("a/../file"),
            Path::new("./file"),
        ] {
            assert_eq!(
                inspect(&root, path, 1024).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(
            inspect(&root, &root.join("outside"), 1024)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlink_ancestors_and_non_regular_leaves_are_not_read() {
        use std::os::unix::fs::symlink;
        let root =
            std::env::temp_dir().join(format!("desktop-working-file-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("real")).unwrap();
        std::fs::write(root.join("real/file"), b"private contents").unwrap();
        symlink("real", root.join("link")).unwrap();
        assert!(inspect(&root, Path::new("link/file"), 1024).is_err());
        symlink("real/file", root.join("leaf")).unwrap();
        // Simulate replacement after the initial metadata check: leaf opens still refuse links.
        assert!(read_regular(&root, Path::new("leaf"), 1024).is_err());
        assert!(matches!(
            read_regular(&root, Path::new("real"), 1024).unwrap(),
            WorkingFile::Unsupported
        ));
        let output = std::process::Command::new("mkfifo")
            .arg(root.join("fifo"))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(matches!(
            inspect(&root, Path::new("fifo"), 1024).unwrap(),
            WorkingFile::Unsupported
        ));
        assert!(matches!(
            inspect(&root, Path::new("real"), 1024).unwrap(),
            WorkingFile::Unsupported
        ));
        assert_eq!(
            std::fs::read(root.join("real/file")).unwrap(),
            b"private contents"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
