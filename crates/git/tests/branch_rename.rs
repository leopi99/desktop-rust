use desktop_git::{rename_current_branch, snapshot};
use std::{
    ffi::OsStr,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-rust-rename-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("repo");
        std::fs::create_dir_all(&path).unwrap();
        let repo = Self { root, path };
        repo.git(&["init", "--initial-branch=main"]);
        repo.git(&["config", "user.name", "Test"]);
        repo.git(&["config", "user.email", "test@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.git(&["config", "core.hooksPath", "/dev/null"]);
        std::fs::write(repo.path.join("file.txt"), b"base\n").unwrap();
        repo
    }
    fn git<S: AsRef<OsStr>>(&self, args: &[S]) -> Vec<u8> {
        let output = Command::new("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(&self.path)
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
    fn commit(&self) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "-m", "fixture"]);
    }
    fn unchanged_state(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            self.git(&["show-ref"]),
            self.git(&["config", "--local", "--null", "--list"]),
            std::fs::read(self.path.join(".git/index")).unwrap(),
            self.git(&["status", "--porcelain=v1"]),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn rename_preserves_dirty_files_index_tracking_reflog_and_remote_refs() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/test.git",
    ]);
    for (key, value) in [
        ("remote", "origin"),
        ("merge", "refs/heads/main"),
        ("pushRemote", "origin"),
        ("rebase", "true"),
        ("description", "My branch"),
    ] {
        repo.git(&["config", &format!("branch.main.{key}"), value]);
    }
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    std::fs::write(repo.path.join("file.txt"), b"staged\n").unwrap();
    repo.git(&["add", "file.txt"]);
    std::fs::write(repo.path.join("file.txt"), b"unstaged\n").unwrap();
    std::fs::write(repo.path.join("new.txt"), b"untracked\n").unwrap();
    let before = snapshot(&repo.path).unwrap();
    let index = std::fs::read(repo.path.join(".git/index")).unwrap();
    rename_current_branch(&repo.path, "feature/renamed", &before.commit_basis).unwrap();
    let after = snapshot(&repo.path).unwrap();
    assert_eq!(
        after.commit_basis.branch.unwrap(),
        "refs/heads/feature/renamed"
    );
    assert_eq!(after.commit_basis.head, before.commit_basis.head);
    assert_eq!(after.changes, before.changes);
    assert_eq!(std::fs::read(repo.path.join(".git/index")).unwrap(), index);
    assert_eq!(
        std::fs::read(repo.path.join("file.txt")).unwrap(),
        b"unstaged\n"
    );
    assert_eq!(
        std::fs::read(repo.path.join("new.txt")).unwrap(),
        b"untracked\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/renamed.merge"]),
        b"refs/heads/main\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/renamed.remote"]),
        b"origin\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/renamed.pushRemote"]),
        b"origin\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/renamed.rebase"]),
        b"true\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/renamed.description"]),
        b"My branch\n"
    );
    assert_eq!(
        repo.git(&["rev-parse", "refs/remotes/origin/main"]),
        repo.git(&["rev-parse", "HEAD"])
    );
    let reflog = repo.git(&["reflog", "show", "--format=%gs", "feature/renamed"]);
    assert!(String::from_utf8_lossy(&reflog).contains("commit (initial): fixture"));
    assert!(!after.branches.iter().any(|branch| branch.name == "main"));
}

#[test]
fn occupied_invalid_and_same_names_never_overwrite_refs_or_config() {
    let repo = Fixture::new();
    repo.commit();
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    repo.git(&["branch", "occupied"]); // Destination created after the observation.
    let before = repo.unchanged_state();
    for name in [
        "occupied",
        "main",
        "",
        "-M",
        "HEAD",
        "@{-1}",
        "bad..name",
        "refs/../bad",
        "with space",
    ] {
        assert!(
            rename_current_branch(&repo.path, name, &basis).is_err(),
            "{name}"
        );
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn orphan_destination_configuration_is_not_merged_or_overwritten() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["config", "branch.future.remote", "saved-remote"]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(rename_current_branch(&repo.path, "future", &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
}

#[test]
fn changed_index_head_or_branch_requires_a_new_observation() {
    for change in ["index", "head", "branch"] {
        let repo = Fixture::new();
        repo.commit();
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        match change {
            "index" => {
                std::fs::write(repo.path.join("new.txt"), b"new\n").unwrap();
                repo.git(&["add", "new.txt"]);
            }
            "head" => {
                repo.git(&["commit", "--allow-empty", "-m", "new tip"]);
            }
            _ => {
                repo.git(&["switch", "--create", "other"]);
            }
        }
        let before = repo.unchanged_state();
        assert!(rename_current_branch(&repo.path, "renamed", &basis).is_err());
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn unborn_and_detached_head_are_rejected() {
    let repo = Fixture::new();
    let unborn = snapshot(&repo.path).unwrap().commit_basis;
    assert!(rename_current_branch(&repo.path, "renamed", &unborn).is_err());
    assert_eq!(repo.git(&["symbolic-ref", "HEAD"]), b"refs/heads/main\n");
    repo.commit();
    repo.git(&["switch", "--detach"]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(rename_current_branch(&repo.path, "renamed", &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
}

#[test]
fn active_merge_rebase_cherry_pick_and_revert_metadata_block_rename() {
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "sequencer",
        "rebase-merge",
        "rebase-apply",
    ] {
        let repo = Fixture::new();
        repo.commit();
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        let metadata = repo.path.join(".git").join(name);
        if name.contains("HEAD") {
            std::fs::write(&metadata, repo.git(&["rev-parse", "HEAD"])).unwrap();
        } else {
            std::fs::create_dir(&metadata).unwrap();
        }
        let refs = repo.git(&["show-ref"]);
        assert!(
            rename_current_branch(&repo.path, "renamed", &basis).is_err(),
            "{name}"
        );
        assert_eq!(repo.git(&["show-ref"]), refs);
        assert!(metadata.exists());
    }
}

#[test]
fn linked_worktree_can_rename_its_own_branch_without_changing_main_head() {
    let repo = Fixture::new();
    repo.commit();
    let linked = repo.root.join("linked");
    repo.git(&["worktree", "add", "-b", "work", linked.to_str().unwrap()]);
    let basis = snapshot(&linked).unwrap().commit_basis;
    rename_current_branch(&linked, "work-renamed", &basis).unwrap();
    assert_eq!(
        snapshot(&linked).unwrap().commit_basis.branch.unwrap(),
        "refs/heads/work-renamed"
    );
    assert_eq!(repo.git(&["symbolic-ref", "HEAD"]), b"refs/heads/main\n");
}

#[test]
fn branch_shared_with_another_worktree_is_not_renamed() {
    let repo = Fixture::new();
    repo.commit();
    let linked = repo.root.join("linked");
    repo.git(&[
        "worktree",
        "add",
        "--force",
        linked.to_str().unwrap(),
        "main",
    ]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(rename_current_branch(&repo.path, "renamed", &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
    assert_eq!(
        snapshot(&linked).unwrap().commit_basis.branch.unwrap(),
        "refs/heads/main"
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_source_branch_is_renamed_using_its_native_name() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    repo.commit();
    let native = OsStr::from_bytes(b"feature-\xff");
    repo.git(&[
        OsStr::new("branch"),
        OsStr::new("--move"),
        OsStr::new("main"),
        native,
    ]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    rename_current_branch(&repo.path, "renamed", &basis).unwrap();
    assert_eq!(repo.git(&["symbolic-ref", "HEAD"]), b"refs/heads/renamed\n");
}

#[test]
fn destination_prefix_does_not_match_another_branch_configuration() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["config", "branch.future.child.remote", "saved-remote"]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    rename_current_branch(&repo.path, "future", &basis).unwrap();
    assert_eq!(
        repo.git(&["config", "branch.future.child.remote"]),
        b"saved-remote\n"
    );
}
