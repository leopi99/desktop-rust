//! Git integration coverage for conflicts.
#![cfg(test)]

use desktop_core::{CommitBasis, ConflictContent, ConflictSide, ConflictSideContent, FileChange};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    name: PathBuf,
}
fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
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
impl Fixture {
    fn new(name: impl AsRef<Path>) -> Self {
        Self::with_contents(name, b"base\n", b"current\n", b"other\n")
    }
    fn with_contents(name: impl AsRef<Path>, base: &[u8], current: &[u8], other: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-conflict-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let repo = desktop_git::init_repository(&root.join("project"), "main").unwrap();
        let fixture = Self {
            root,
            repo,
            name: name.as_ref().to_path_buf(),
        };
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git(&fixture.repo, &["config", key, value]);
        }
        std::fs::create_dir_all(fixture.file().parent().unwrap()).unwrap();
        fixture.write(base);
        fixture.commit("baseline");
        git(&fixture.repo, &["checkout", "-b", "side"]);
        fixture.write(other);
        fixture.commit("other side");
        git(&fixture.repo, &["checkout", "main"]);
        fixture.write(current);
        fixture.commit("current side");
        let output = Command::new("git")
            .arg("-C")
            .arg(&fixture.repo)
            .args(["merge", "side"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        fixture
    }
    fn file(&self) -> PathBuf {
        self.repo.join(&self.name)
    }
    fn write(&self, bytes: &[u8]) {
        std::fs::write(self.file(), bytes).unwrap();
    }
    fn commit(&self, message: &str) {
        git(&self.repo, &["add", "--all"]);
        git(&self.repo, &["commit", "-m", message]);
    }
    fn change(&self) -> FileChange {
        desktop_git::snapshot(&self.repo)
            .unwrap()
            .changes
            .into_iter()
            .find(|c| c.path == self.name)
            .unwrap()
    }
    fn basis(&self) -> CommitBasis {
        desktop_git::snapshot(&self.repo).unwrap().commit_basis
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn markers_block_resolution_without_changing_index_or_working_file() {
    let f = Fixture::new("file");
    let before = f.basis();
    let bytes = std::fs::read(f.file()).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    assert!(review.marker_count >= 3);
    assert_eq!(review.marker_size, 7);
    assert!(desktop_git::resolve_conflict(&review).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
}

#[test]
fn reviewed_text_resolution_can_complete_a_merge_commit() {
    let f = Fixture::new("file");
    f.write(b"combined result\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    assert_eq!(review.marker_count, 0);
    desktop_git::resolve_conflict(&review).unwrap();
    let state = desktop_git::snapshot(&f.repo).unwrap();
    assert!(!state.changes.iter().any(FileChange::is_conflicted));
    assert_eq!(std::fs::read(f.file()).unwrap(), b"combined result\n");
    desktop_git::commit(&f.repo, "Merge resolved", &state.commit_basis).unwrap();
    assert_eq!(
        String::from_utf8(git(&f.repo, &["rev-list", "--parents", "-1", "HEAD"]))
            .unwrap()
            .split_whitespace()
            .count(),
        3
    );
}

#[test]
fn file_index_and_head_changes_require_a_new_review() {
    let f = Fixture::new("file");
    f.write(b"reviewed\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    f.write(b"changed since review\n");
    assert!(desktop_git::resolve_conflict(&review).is_err());
    assert!(f.change().is_conflicted());
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    std::fs::write(f.repo.join("another"), b"new staged file").unwrap();
    git(&f.repo, &["add", "another"]);
    assert!(desktop_git::resolve_conflict(&review).is_err());
    assert!(f.change().is_conflicted());
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    git(&f.repo, &["symbolic-ref", "HEAD", "refs/heads/side"]);
    assert!(desktop_git::resolve_conflict(&review).is_err());
}

#[test]
fn custom_marker_size_and_diff3_base_markers_are_checked() {
    let f = Fixture::new("file");
    std::fs::write(
        f.repo.join(".git/info/attributes"),
        b"file conflict-marker-size=10\n",
    )
    .unwrap();
    f.write(b"<<<<<<<<<< ours\r\ncurrent\r\n|||||||||| base\r\nbase\r\n==========\r\nother\r\n>>>>>>>>>> theirs\r\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    assert_eq!(review.marker_size, 10);
    assert_eq!(review.marker_count, 4);
    assert!(desktop_git::resolve_conflict(&review).is_err());
    f.write(b"resolved text\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    std::fs::write(
        f.repo.join(".git/info/attributes"),
        b"file conflict-marker-size=12\n",
    )
    .unwrap();
    assert!(desktop_git::resolve_conflict(&review).is_err());
}

#[test]
fn reviewed_deletion_stages_only_the_deletion() {
    let f = Fixture::new("nested/file");
    std::fs::remove_dir_all(f.repo.join("nested")).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    assert_eq!(review.content, ConflictContent::Deleted);
    desktop_git::resolve_conflict(&review).unwrap();
    assert!(!f.file().exists());
    assert!(!f.change().is_conflicted());
    assert_eq!(f.change().index, 'D');
}

#[test]
fn literal_pathspec_and_newline_names_resolve_only_the_selected_path() {
    let f = Fixture::new(":(glob)*\nfile");
    f.write(b"resolved\n");
    std::fs::write(f.repo.join("unrelated"), b"must remain untracked").unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    let state = desktop_git::snapshot(&f.repo).unwrap();
    assert!(state
        .changes
        .iter()
        .any(|c| c.path == Path::new("unrelated") && c.index == '?'));
}

#[test]
fn binary_and_oversized_conflicts_require_external_git_resolution() {
    let f = Fixture::with_contents("file", b"base\0x", b"current\0x", b"other\0x");
    let before = f.basis();
    assert!(desktop_git::inspect_conflict(&f.repo, &f.change()).is_err());
    assert_eq!(f.basis(), before);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(f.file())
        .unwrap();
    file.set_len(9 * 1024 * 1024).unwrap();
    assert!(desktop_git::inspect_conflict(&f.repo, &f.change()).is_err());
    assert_eq!(f.basis(), before);
}

#[test]
fn resolved_entry_cannot_be_marked_resolved_again() {
    let f = Fixture::new("file");
    f.write(b"resolved\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    let before = f.basis();
    assert!(desktop_git::resolve_conflict(&review).is_err());
    assert_eq!(f.basis(), before);
}

#[cfg(unix)]
#[test]
fn executable_mode_changes_invalidate_the_review() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("file");
    f.write(b"resolved\n");
    let review = desktop_git::inspect_conflict(&f.repo, &f.change()).unwrap();
    std::fs::set_permissions(f.file(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(desktop_git::resolve_conflict(&review).is_err());
    assert!(f.change().is_conflicted());
}

#[cfg(unix)]
#[test]
fn symlink_content_is_never_read_during_conflict_review() {
    let f = Fixture::new("file");
    std::fs::remove_file(f.file()).unwrap();
    let outside = f.root.join("outside");
    std::fs::write(&outside, b"outside content").unwrap();
    std::os::unix::fs::symlink(&outside, f.file()).unwrap();
    assert!(desktop_git::inspect_conflict(&f.repo, &f.change()).is_err());
    assert_eq!(std::fs::read(outside).unwrap(), b"outside content");
}

#[test]
fn index_side_previews_keep_both_versions_and_user_state_unchanged() {
    let f = Fixture::new("file");
    let before = f.basis();
    let index = std::fs::read(f.repo.join(".git/index")).unwrap();
    let working = std::fs::read(f.file()).unwrap();
    for (side, expected) in [
        (ConflictSide::Ours, "current\n"),
        (ConflictSide::Theirs, "other\n"),
    ] {
        let preview = desktop_git::inspect_conflict_side(&f.repo, &f.change(), side).unwrap();
        assert_eq!(preview.side, side);
        assert!(preview.oid.is_some());
        assert_eq!(preview.change.path, f.name);
        assert_eq!(
            preview.content,
            ConflictSideContent::Text {
                preview: expected.to_owned(),
                truncated: false
            }
        );
    }
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.repo.join(".git/index")).unwrap(), index);
    assert_eq!(std::fs::read(f.file()).unwrap(), working);
}

#[test]
fn side_previews_identify_binary_data_and_bound_large_text_or_blob_reads() {
    let f = Fixture::with_contents("binary", b"a\0b", b"c\0d", b"e\0f");
    let before = f.basis();
    let preview =
        desktop_git::inspect_conflict_side(&f.repo, &f.change(), ConflictSide::Theirs).unwrap();
    assert_eq!(preview.content, ConflictSideContent::Binary { size: 3 });
    assert!(desktop_git::inspect_conflict(&f.repo, &f.change()).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.file()).unwrap(), b"c\0d");
    let f = Fixture::with_contents(
        "large",
        b"base\n",
        "current\n".repeat(12_000).as_bytes(),
        b"other\n",
    );
    let preview =
        desktop_git::inspect_conflict_side(&f.repo, &f.change(), ConflictSide::Ours).unwrap();
    match preview.content {
        ConflictSideContent::Text { preview, truncated } => {
            assert!(truncated);
            assert_eq!(preview.len(), 64 * 1024);
        }
        other => panic!("unexpected preview: {other:?}"),
    }
    let large = vec![0; 9 * 1024 * 1024];
    let f = Fixture::with_contents("oversized", b"base\n", &large, b"other\n");
    let before = f.basis();
    assert_eq!(
        desktop_git::inspect_conflict_side(&f.repo, &f.change(), ConflictSide::Ours)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.file()).unwrap(), large);
}

#[test]
fn absent_side_is_a_deletion_in_both_modify_delete_directions() {
    for deleted in [ConflictSide::Ours, ConflictSide::Theirs] {
        let f = Fixture::new("file");
        git(&f.repo, &["merge", "--abort"]);
        git(
            &f.repo,
            &[
                "checkout",
                if deleted == ConflictSide::Ours {
                    "main"
                } else {
                    "side"
                },
            ],
        );
        std::fs::remove_file(f.file()).unwrap();
        f.commit("delete selected side");
        git(&f.repo, &["checkout", "main"]);
        let merge = Command::new("git")
            .arg("-C")
            .arg(&f.repo)
            .args(["merge", "side"])
            .output()
            .unwrap();
        assert!(!merge.status.success());
        let before = f.basis();
        let bytes = std::fs::read(f.file()).unwrap();
        let preview = desktop_git::inspect_conflict_side(&f.repo, &f.change(), deleted).unwrap();
        assert_eq!(preview.content, ConflictSideContent::Deleted);
        assert!(preview.oid.is_none());
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
    }
}

#[cfg(unix)]
#[test]
fn index_side_paths_remain_literal_and_native() {
    use std::os::unix::ffi::OsStrExt;
    for name in [
        PathBuf::from(":(glob)*\n\tfile"),
        PathBuf::from(std::ffi::OsStr::from_bytes(b"native-\xff")),
    ] {
        let f = Fixture::new(&name);
        let preview =
            desktop_git::inspect_conflict_side(&f.repo, &f.change(), ConflictSide::Theirs).unwrap();
        assert_eq!(preview.change.path, name);
        assert_eq!(
            preview.content,
            ConflictSideContent::Text {
                preview: "other\n".into(),
                truncated: false
            }
        );
    }
}

#[test]
fn resolved_status_cannot_be_reused_as_an_unmerged_side_preview() {
    let f = Fixture::new("file");
    let selected = f.change();
    f.write(b"resolved externally\n");
    git(&f.repo, &["add", "--all"]);
    let before = f.basis();
    assert!(desktop_git::inspect_conflict_side(&f.repo, &selected, ConflictSide::Ours).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.file()).unwrap(), b"resolved externally\n");
}
