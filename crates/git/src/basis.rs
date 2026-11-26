//! Complete freshness data: read failures must never look like an unborn branch.
use super::{file_path, history::validate_oid, merge, read_output, rebase};
use desktop_core::CommitBasis;
use std::{ffi::OsStr, io, path::Path};

const REF_LIMIT: usize = 64 * 1024;
const INDEX_LIMIT: usize = 16 * 1024 * 1024;

pub(super) fn commit_basis(path: &Path) -> io::Result<CommitBasis> {
    let branch = read_output::optional(path, &["symbolic-ref", "--quiet", "HEAD"], REF_LIMIT)?
        .map(|bytes| {
            file_path(bytes.strip_suffix(b"\n").unwrap_or(&bytes)).map(|path| path.into_os_string())
        })
        .transpose()?;
    let head = read_output::optional(
        path,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
        REF_LIMIT,
    )?;
    let head = match head {
        Some(bytes) => {
            let text = String::from_utf8(bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            validate_oid(text.strip_suffix('\n').unwrap_or(&text))?;
            Some(text)
        }
        None => {
            let branch = branch.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Detached HEAD does not resolve to a commit",
                )
            })?;
            // A missing commit is unborn only when its symbolic ref does not exist.
            let exists = read_output::optional(
                path,
                &[
                    OsStr::new("show-ref"),
                    OsStr::new("--verify"),
                    OsStr::new("--quiet"),
                    OsStr::new("--"),
                    branch.as_os_str(),
                ],
                REF_LIMIT,
            )?;
            if exists.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HEAD reference does not resolve to a commit",
                ));
            }
            None
        }
    };
    Ok(CommitBasis {
        index: read_output::complete(path, &["ls-files", "--stage", "-z"], INDEX_LIMIT)?,
        head,
        branch,
        merge_heads: merge::metadata(path, "MERGE_HEAD")?,
        rebase: rebase::state(path)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "desktop-basis-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            let repo = super::super::init_repository(&root.join("repo"), "main").unwrap();
            Self(repo)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn unborn_is_distinct_from_detached_or_symbolic_refs_with_missing_objects() {
        let fixture = Fixture::new();
        let basis = commit_basis(&fixture.0).unwrap();
        assert!(basis.head.is_none());
        assert_eq!(basis.branch.as_deref(), Some(OsStr::new("refs/heads/main")));
        let missing = "a".repeat(40);
        std::fs::write(
            fixture.0.join(".git/refs/heads/main"),
            format!("{missing}\n"),
        )
        .unwrap();
        assert!(commit_basis(&fixture.0).is_err());
        std::fs::write(fixture.0.join(".git/HEAD"), format!("{missing}\n")).unwrap();
        assert_eq!(
            commit_basis(&fixture.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn malformed_symbolic_ref_is_an_error_instead_of_a_detached_observation() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join(".git/HEAD"), b"invalid HEAD\n").unwrap();
        assert!(commit_basis(&fixture.0).is_err());
    }
}
