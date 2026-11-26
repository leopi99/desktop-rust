//! Read-only preview bounds, native paths and repository-root binding.
#![cfg(test)]
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-preview-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("repo with\nnewline");
        std::fs::create_dir_all(&repo).unwrap();
        let f = Self { root, repo };
        f.git(&["init", "--initial-branch=main"]);
        for (key, value) in [
            ("user.name", "Fixture"),
            ("user.email", "fixture@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
            ("core.autocrlf", "false"),
            ("core.attributesFile", "/dev/null"),
        ] {
            f.git(&["config", key, value]);
        }
        std::fs::write(f.repo.join("tracked"), b"base\n").unwrap();
        f.git(&["add", "--all"]);
        f.git(&["commit", "-m", "fixture"]);
        f
    }
    fn git<S: AsRef<OsStr>>(&self, args: &[S]) -> Vec<u8> {
        let output = Command::new("git")
            .arg("--literal-pathspecs")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(&self.repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn change(&self, path: &Path) -> desktop_core::FileChange {
        desktop_git::snapshot(&self.repo)
            .unwrap()
            .changes
            .into_iter()
            .find(|change| change.path == path)
            .unwrap()
    }
    fn basis(&self) -> desktop_core::CommitBasis {
        desktop_git::snapshot(&self.repo).unwrap().commit_basis
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn untracked_text_binary_and_size_boundaries_preserve_data_and_allow_full_staging() {
    let f = Fixture::new();
    let config = f.git(&["config", "--local", "--null", "--list"]);
    for (name, bytes) in [
        ("empty", Vec::new()),
        ("text", b"preview text\n".to_vec()),
        ("binary", b"binary\0data".to_vec()),
        ("exact", vec![b'x'; 1024 * 1024]),
        ("over", vec![b'x'; 1024 * 1024 + 1]),
    ] {
        let path = Path::new(name);
        std::fs::write(f.repo.join(path), &bytes).unwrap();
        let selected = f.change(path);
        let before = f.basis();
        let preview = desktop_git::diff(&f.repo, &selected).unwrap();
        assert!(preview.staged.is_empty());
        if name == "binary" {
            assert_eq!(preview.unstaged, "Untracked file\nBinary file");
        } else if name == "over" {
            assert!(preview.unstaged.contains("Preview unavailable"));
            assert!(preview.unstaged.len() < 200);
        } else {
            assert_eq!(
                preview.unstaged,
                format!(
                    "Untracked file\n{}",
                    String::from_utf8(bytes.clone()).unwrap()
                )
            );
        }
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(f.repo.join(path)).unwrap(), bytes);
        assert_eq!(f.git(&["config", "--local", "--null", "--list"]), config);
        if name == "over" {
            desktop_git::stage(&f.repo, &selected).unwrap();
            assert_eq!(f.git(&["show", ":over"]), bytes);
            assert_eq!(std::fs::read(f.repo.join(path)).unwrap(), bytes);
        }
    }
}

#[cfg(unix)]
#[test]
fn untracked_symlinks_show_the_target_spelling_without_reading_its_contents() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = f.root.join("external");
    std::fs::write(&outside, b"external contents must not be previewed").unwrap();
    for (name, target) in [
        ("link", outside.clone()),
        ("dangling", f.root.join("missing")),
    ] {
        symlink(&target, f.repo.join(name)).unwrap();
        let before = f.basis();
        let preview = desktop_git::diff(&f.repo, &f.change(Path::new(name))).unwrap();
        assert_eq!(
            preview.unstaged,
            format!("Untracked file\nSymbolic link → {}", target.display())
        );
        assert!(!preview.unstaged.contains("external contents"));
        assert_eq!(f.basis(), before);
    }
    assert_eq!(
        std::fs::read(outside).unwrap(),
        b"external contents must not be previewed"
    );
}

#[test]
fn tracked_and_untracked_previews_resolve_native_root_paths_from_a_subdirectory() {
    let f = Fixture::new();
    std::fs::create_dir(f.repo.join("nested")).unwrap();
    std::fs::write(f.repo.join("nested/new"), b"nested draft\n").unwrap();
    std::fs::write(f.repo.join("tracked"), b"updated tracked\n").unwrap();
    let before = f.basis();
    for path in [Path::new("nested/new"), Path::new("tracked")] {
        let selected = f.change(path);
        let root = desktop_git::diff(&f.repo, &selected).unwrap();
        let nested = desktop_git::diff(&f.repo.join("nested"), &selected).unwrap();
        assert_eq!(nested, root);
        assert!(nested.unstaged.contains(if path == Path::new("tracked") {
            "+updated tracked"
        } else {
            "nested draft"
        }));
    }
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(f.repo.join("nested/new")).unwrap(),
        b"nested draft\n"
    );
}

#[cfg(unix)]
#[test]
fn untracked_native_and_literal_names_are_read_without_lossy_path_conversion() {
    use std::os::unix::ffi::OsStrExt;
    let f = Fixture::new();
    let before = f.basis();
    for name in [
        PathBuf::from(":(glob)*\n\tfile"),
        PathBuf::from(OsStr::from_bytes(b"native-\xff")),
    ] {
        std::fs::write(f.repo.join(&name), b"native contents\n").unwrap();
        let selected = f.change(&name);
        assert_eq!(selected.path, name);
        assert_eq!(
            desktop_git::diff(&f.repo, &selected).unwrap().unstaged,
            "Untracked file\nnative contents\n"
        );
        assert_eq!(
            std::fs::read(f.repo.join(name)).unwrap(),
            b"native contents\n"
        );
    }
    assert_eq!(f.basis(), before);
}

#[cfg(unix)]
#[test]
fn replacing_an_observed_parent_with_a_symlink_invalidates_the_preview() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    std::fs::create_dir(f.repo.join("nested")).unwrap();
    std::fs::write(f.repo.join("nested/file"), b"original draft").unwrap();
    let selected = f.change(Path::new("nested/file"));
    let outside = f.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("file"), b"external contents").unwrap();
    std::fs::rename(f.repo.join("nested"), f.root.join("held")).unwrap();
    symlink(&outside, f.repo.join("nested")).unwrap();
    let before = f.basis();
    assert!(desktop_git::diff(&f.repo, &selected).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(outside.join("file")).unwrap(),
        b"external contents"
    );
    assert_eq!(
        std::fs::read(f.root.join("held/file")).unwrap(),
        b"original draft"
    );
}
