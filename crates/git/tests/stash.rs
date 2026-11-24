use desktop_core::{CommitBasis, StashEntry};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}
fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-stash-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let repo = desktop_git::init_repository(&root.join("project"), "main").unwrap();
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git(&repo, &["config", key, value]);
        }
        std::fs::write(repo.join("file"), b"base\n").unwrap();
        git(&repo, &["add", "file"]);
        git(&repo, &["commit", "-m", "baseline"]);
        Self { root, repo }
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.repo.join(name), bytes).unwrap();
    }
    fn basis(&self) -> CommitBasis {
        desktop_git::snapshot(&self.repo).unwrap().commit_basis
    }
    fn entries(&self) -> Vec<StashEntry> {
        desktop_git::snapshot(&self.repo).unwrap().stashes
    }
    fn save(&self, message: &str) {
        desktop_git::save_stash(&self.repo, message, &self.basis()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn saving_and_restoring_preserves_staged_unstaged_and_new_file_boundaries() {
    let f = Fixture::new();
    f.write("file", b"staged\n");
    git(&f.repo, &["add", "file"]);
    f.write("file", b"unstaged\n");
    f.write("added", b"staged new file\n");
    git(&f.repo, &["add", "added"]);
    f.write("untracked", b"keep local\n");
    let before = f.basis();
    f.save("--literal description");
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"base\n");
    assert!(!f.repo.join("added").exists());
    assert_eq!(
        std::fs::read(f.repo.join("untracked")).unwrap(),
        b"keep local\n"
    );
    std::fs::rename(f.repo.join("untracked"), f.root.join("kept-untracked")).unwrap();
    let entry = f.entries()[0].clone();
    assert!(entry.subject.contains("--literal description"));
    desktop_git::apply_stash(&f.repo, &entry, &f.basis()).unwrap();
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"unstaged\n");
    assert_eq!(
        std::fs::read(f.repo.join("added")).unwrap(),
        b"staged new file\n"
    );
    assert_eq!(f.basis().index, before.index);
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn restoring_uses_object_identity_when_a_new_stash_shifts_the_reflog() {
    let f = Fixture::new();
    f.write("file", b"first\n");
    f.save("first");
    let first = f.entries()[0].clone();
    f.write("file", b"second\n");
    f.save("second");
    assert_ne!(f.entries()[0].oid, first.oid);
    desktop_git::apply_stash(&f.repo, &first, &f.basis()).unwrap();
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"first\n");
    assert_eq!(f.entries().len(), 2);
}

#[test]
fn save_rejects_unborn_clean_and_untracked_only_repositories() {
    let f = Fixture::new();
    assert!(desktop_git::save_stash(&f.repo, "empty", &f.basis()).is_err());
    f.write("untracked", b"keep");
    assert!(desktop_git::save_stash(&f.repo, "empty", &f.basis()).is_err());
    assert!(f.entries().is_empty());
    assert_eq!(std::fs::read(f.repo.join("untracked")).unwrap(), b"keep");
    let unborn = desktop_git::init_repository(&f.root.join("unborn"), "main").unwrap();
    let state = desktop_git::snapshot(&unborn).unwrap();
    assert!(desktop_git::save_stash(&unborn, "unborn", &state.commit_basis).is_err());
}

#[test]
fn stale_source_or_removed_entry_is_rejected_without_changes() {
    let f = Fixture::new();
    f.write("file", b"saved\n");
    f.save("");
    let entry = f.entries()[0].clone();
    let old = f.basis();
    git(&f.repo, &["commit", "--allow-empty", "-m", "new HEAD"]);
    assert!(desktop_git::apply_stash(&f.repo, &entry, &old).is_err());
    f.write("file", b"unsaved\n");
    assert!(desktop_git::save_stash(&f.repo, "stale", &old).is_err());
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"unsaved\n");
    git(&f.repo, &["restore", "file"]);
    git(&f.repo, &["stash", "drop", "stash@{0}"]);
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"base\n");
}

#[test]
fn restore_rejects_dirty_and_untracked_working_trees() {
    let f = Fixture::new();
    f.write("file", b"saved\n");
    f.save("save");
    let entry = f.entries()[0].clone();
    f.write("file", b"local\n");
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    assert_eq!(std::fs::read(f.repo.join("file")).unwrap(), b"local\n");
    git(&f.repo, &["restore", "file"]);
    f.write("new", b"untracked\n");
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    assert_eq!(std::fs::read(f.repo.join("new")).unwrap(), b"untracked\n");
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn ignored_file_collision_is_detected_before_restoring_tracked_additions() {
    let f = Fixture::new();
    f.write("added", b"saved file\n");
    git(&f.repo, &["add", "added"]);
    f.save("added");
    let entry = f.entries()[0].clone();
    std::fs::write(f.repo.join(".git/info/exclude"), b"added\n").unwrap();
    f.write("added", b"ignored local file\n");
    let error = desktop_git::apply_stash(&f.repo, &entry, &f.basis()).unwrap_err();
    assert!(error.to_string().contains("ignored path"));
    assert_eq!(
        std::fs::read(f.repo.join("added")).unwrap(),
        b"ignored local file\n"
    );
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn external_stash_with_untracked_files_is_restored_and_retained() {
    let f = Fixture::new();
    f.write("new", b"external new file\n");
    git(&f.repo, &["stash", "push", "-u", "-m", "external"]);
    let entry = f.entries()[0].clone();
    desktop_git::apply_stash(&f.repo, &entry, &f.basis()).unwrap();
    assert_eq!(
        std::fs::read(f.repo.join("new")).unwrap(),
        b"external new file\n"
    );
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn external_untracked_stash_cannot_overwrite_an_ignored_file() {
    let f = Fixture::new();
    f.write("new", b"external new file\n");
    git(&f.repo, &["stash", "push", "-u", "-m", "external"]);
    let entry = f.entries()[0].clone();
    std::fs::write(f.repo.join(".git/info/exclude"), b"new\n").unwrap();
    f.write("new", b"local ignored file\n");
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    assert_eq!(
        std::fs::read(f.repo.join("new")).unwrap(),
        b"local ignored file\n"
    );
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn conflicting_restore_keeps_the_stash_available_for_recovery() {
    let f = Fixture::new();
    f.write("file", b"saved side\n");
    f.save("save");
    let entry = f.entries()[0].clone();
    f.write("file", b"committed side\n");
    git(&f.repo, &["commit", "-am", "conflicting change"]);
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    let snapshot = desktop_git::snapshot(&f.repo).unwrap();
    assert!(snapshot.changes.iter().any(|c| c.is_conflicted()));
    assert_eq!(snapshot.stashes, vec![entry]);
    assert!(desktop_git::save_stash(&f.repo, "conflicts", &snapshot.commit_basis).is_err());
}

#[cfg(unix)]
#[test]
fn restore_never_traverses_an_ignored_symlink_directory() {
    let f = Fixture::new();
    std::fs::create_dir(f.repo.join("dir")).unwrap();
    f.write("dir/file", b"saved");
    git(&f.repo, &["add", "dir/file"]);
    f.save("new nested file");
    std::fs::write(f.repo.join(".git/info/exclude"), b"dir\n").unwrap();
    let outside = f.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, f.repo.join("dir")).unwrap();
    let entry = f.entries()[0].clone();
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    assert!(!outside.join("file").exists());
    assert_eq!(f.entries(), vec![entry]);
}

#[test]
fn unicode_multiline_stash_messages_remain_a_single_entry() {
    let f = Fixture::new();
    f.write("file", b"changed\n");
    f.save("Résumé\nAdditional details");
    let entries = f.entries();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].subject.contains("Résumé"));
}

#[test]
fn stash_conflict_can_be_reviewed_resolved_and_committed_without_dropping_stash() {
    let f = Fixture::new();
    f.write("file", b"saved side\n");
    f.save("saved");
    let entry = f.entries()[0].clone();
    f.write("file", b"committed side\n");
    git(&f.repo, &["commit", "-am", "conflicting change"]);
    assert!(desktop_git::apply_stash(&f.repo, &entry, &f.basis()).is_err());
    f.write("file", b"combined resolution\n");
    let state = desktop_git::snapshot(&f.repo).unwrap();
    let conflict = state.changes.iter().find(|c| c.is_conflicted()).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, conflict).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    desktop_git::commit(&f.repo, "Resolved saved changes", &f.basis()).unwrap();
    assert!(desktop_git::snapshot(&f.repo).unwrap().changes.is_empty());
    assert_eq!(f.entries(), vec![entry]);
}
