use desktop_git::{delete_local_branch, snapshot};
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
            "desktop-rust-delete-{}-{}",
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

fn selected(repo: &Fixture, name: &OsStr) -> desktop_core::LocalBranch {
    snapshot(&repo.path)
        .unwrap()
        .branches
        .into_iter()
        .find(|branch| branch.name == name)
        .unwrap()
}

#[test]
fn merged_branch_deletion_preserves_files_index_head_and_remote_tracking_ref() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["branch", "topic"]);
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", "topic"]);
    repo.git(&["config", "branch.topic.remote", "origin"]);
    repo.git(&["config", "branch.topic.merge", "refs/heads/topic"]);
    repo.git(&["commit", "--allow-empty", "-m", "new main commit"]);
    std::fs::write(repo.path.join("file.txt"), b"staged\n").unwrap();
    repo.git(&["add", "file.txt"]);
    std::fs::write(repo.path.join("file.txt"), b"unstaged\n").unwrap();
    std::fs::write(repo.path.join("new.txt"), b"untracked\n").unwrap();
    let before = snapshot(&repo.path).unwrap();
    let branch = selected(&repo, OsStr::new("topic"));
    let index = std::fs::read(repo.path.join(".git/index")).unwrap();
    let remote = repo.git(&["rev-parse", "refs/remotes/origin/topic"]);
    delete_local_branch(&repo.path, &branch, &before.commit_basis).unwrap();
    let after = snapshot(&repo.path).unwrap();
    assert_eq!(after.commit_basis, before.commit_basis);
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
        repo.git(&["rev-parse", "refs/remotes/origin/topic"]),
        remote
    );
    assert!(!after.branches.iter().any(|branch| branch.name == "topic"));
    assert!(!repo.path.join(".git/logs/refs/heads/topic").exists());
    assert!(
        !String::from_utf8_lossy(&repo.git(&["config", "--local", "--list"]))
            .contains("branch.topic.")
    );
}

#[test]
fn upstream_containing_the_tip_does_not_allow_unmerged_current_history_to_be_deleted() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["switch", "--create", "topic"]);
    repo.git(&["commit", "--allow-empty", "-m", "only on topic"]);
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", "topic"]);
    repo.git(&["config", "branch.topic.remote", "origin"]);
    repo.git(&["config", "branch.topic.merge", "refs/heads/topic"]);
    repo.git(&["switch", "main"]);
    let branch = selected(&repo, OsStr::new("topic"));
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    let error = delete_local_branch(&repo.path, &branch, &basis).unwrap_err();
    assert!(error.to_string().contains("not merged"));
    assert_eq!(repo.unchanged_state(), before);
}

#[test]
fn git_upstream_merge_check_is_never_forced_even_when_current_head_contains_tip() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    repo.git(&["commit", "--allow-empty", "-m", "new commit"]);
    repo.git(&["branch", "topic"]);
    repo.git(&["config", "branch.topic.remote", "origin"]);
    repo.git(&["config", "branch.topic.merge", "refs/heads/topic"]);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let branch = selected(&repo, OsStr::new("topic"));
    let before = repo.unchanged_state();
    assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
}

#[test]
fn current_branch_cannot_be_deleted_even_with_a_forged_current_flag() {
    let repo = Fixture::new();
    repo.commit();
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let mut branch = selected(&repo, OsStr::new("main"));
    branch.is_current = false;
    let before = repo.unchanged_state();
    assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
}

#[test]
fn moved_or_removed_target_is_not_deleted_from_a_stale_selection() {
    for removed in [false, true] {
        let repo = Fixture::new();
        repo.commit();
        repo.git(&["branch", "topic"]);
        let branch = selected(&repo, OsStr::new("topic"));
        repo.git(&["commit", "--allow-empty", "-m", "advanced main"]);
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        if removed {
            repo.git(&["branch", "--delete", "topic"]);
        } else {
            repo.git(&["branch", "--force", "topic", "HEAD"]);
        }
        let before = repo.unchanged_state();
        assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn changed_index_head_and_branch_block_deletion() {
    for change in ["index", "head", "branch"] {
        let repo = Fixture::new();
        repo.commit();
        repo.git(&["branch", "topic"]);
        let branch = selected(&repo, OsStr::new("topic"));
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
        assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn branch_in_a_linked_worktree_is_not_deleted() {
    let repo = Fixture::new();
    repo.commit();
    let linked = repo.root.join("linked");
    repo.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
    let branch = selected(&repo, OsStr::new("topic"));
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
    assert_eq!(
        snapshot(&linked).unwrap().commit_basis.branch.unwrap(),
        "refs/heads/topic"
    );
}

#[test]
fn active_git_operations_block_deletion() {
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
        repo.git(&["branch", "topic"]);
        let branch = selected(&repo, OsStr::new("topic"));
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        let metadata = repo.path.join(".git").join(name);
        if name.contains("HEAD") {
            std::fs::write(&metadata, repo.git(&["rev-parse", "HEAD"])).unwrap();
        } else {
            std::fs::create_dir(&metadata).unwrap();
        }
        let refs = repo.git(&["show-ref"]);
        assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
        assert_eq!(repo.git(&["show-ref"]), refs);
        assert!(metadata.exists());
    }
}

#[test]
fn detached_and_unborn_head_do_not_allow_deletion() {
    let repo = Fixture::new();
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let invented = desktop_core::LocalBranch {
        name: "topic".into(),
        oid: "0".repeat(40),
        is_current: false,
    };
    assert!(delete_local_branch(&repo.path, &invented, &basis).is_err());
    repo.commit();
    repo.git(&["branch", "topic"]);
    repo.git(&["switch", "--detach"]);
    let branch = selected(&repo, OsStr::new("topic"));
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(delete_local_branch(&repo.path, &branch, &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
}

#[cfg(unix)]
#[test]
fn native_non_utf8_target_name_is_deleted_without_lossy_conversion() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    repo.commit();
    let native = OsStr::from_bytes(b"topic-\xff");
    repo.git(&[OsStr::new("branch"), native]);
    let branch = selected(&repo, native);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    delete_local_branch(&repo.path, &branch, &basis).unwrap();
    assert_eq!(snapshot(&repo.path).unwrap().branches.len(), 1);
}
