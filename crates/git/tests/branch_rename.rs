//! Git integration coverage for branch rename.
#![cfg(test)]

use desktop_git::{rename_current_branch, rename_local_branch, snapshot};
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

#[test]
fn oversized_local_config_refuses_rename_and_allows_retry_after_fixture_repair() {
    let repo = Fixture::new();
    repo.commit();
    let observed = snapshot(&repo.path).unwrap();
    let config_path = repo.path.join(".git/config");
    let original = std::fs::read(&config_path).unwrap();
    let mut config = original.clone();
    config.extend_from_slice(
        format!("\n[oversized]\nvalue = {}\n", "x".repeat(5 * 1024 * 1024)).as_bytes(),
    );
    std::fs::write(&config_path, &config).unwrap();
    let before = repo.unchanged_state();
    assert_eq!(
        rename_current_branch(&repo.path, "renamed", &observed.commit_basis)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(repo.unchanged_state(), before);
    assert_eq!(std::fs::read(&config_path).unwrap(), config);
    std::fs::write(&config_path, original).unwrap();
    rename_current_branch(&repo.path, "renamed", &observed.commit_basis).unwrap();
    assert_eq!(
        snapshot(&repo.path).unwrap().commit_basis.branch.unwrap(),
        "refs/heads/renamed"
    );
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
fn selected_rename_preserves_current_state_and_moves_only_target_tracking_and_reflog() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["branch", "topic"]);
    let target = selected(&repo, OsStr::new("topic"));
    repo.git(&["commit", "--allow-empty", "-m", "main only"]);
    repo.git(&["config", "branch.topic.remote", "origin"]);
    repo.git(&["config", "branch.topic.merge", "refs/heads/topic"]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", &target.oid]);
    std::fs::write(repo.path.join("file.txt"), b"staged\n").unwrap();
    repo.git(&["add", "file.txt"]);
    std::fs::write(repo.path.join("file.txt"), b"unstaged\n").unwrap();
    std::fs::write(repo.path.join("untracked"), b"keep\n").unwrap();
    let before = snapshot(&repo.path).unwrap();
    rename_local_branch(&repo.path, &target, "feature/nuovo-è", &before.commit_basis).unwrap();
    let after = snapshot(&repo.path).unwrap();
    assert_eq!(after.commit_basis, before.commit_basis);
    assert_eq!(after.changes, before.changes);
    assert!(!after.branches.iter().any(|branch| branch.name == "topic"));
    assert_eq!(
        selected(&repo, OsStr::new("feature/nuovo-è")).oid,
        target.oid
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/nuovo-è.remote"]),
        b"origin\n"
    );
    assert_eq!(
        repo.git(&["config", "branch.feature/nuovo-è.merge"]),
        b"refs/heads/topic\n"
    );
    assert_eq!(
        repo.git(&["rev-parse", "refs/remotes/origin/topic"]),
        format!("{}\n", target.oid).as_bytes()
    );
    let reflog = repo.git(&["reflog", "show", "--format=%gs", "feature/nuovo-è"]);
    assert!(String::from_utf8_lossy(&reflog).contains("branch: Created from main"));
    assert_eq!(
        std::fs::read(repo.path.join("file.txt")).unwrap(),
        b"unstaged\n"
    );
    assert_eq!(
        std::fs::read(repo.path.join("untracked")).unwrap(),
        b"keep\n"
    );
}

#[test]
fn selected_rename_rejects_moved_deleted_current_or_stale_observations() {
    for mutation in ["moved", "deleted", "flag", "current", "index"] {
        let repo = Fixture::new();
        repo.commit();
        repo.git(&["branch", "topic"]);
        let mut target = selected(&repo, OsStr::new("topic"));
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        match mutation {
            "moved" => {
                repo.git(&["commit", "--allow-empty", "-m", "advance"]);
                repo.git(&["branch", "--force", "topic", "HEAD"]);
            }
            "deleted" => {
                repo.git(&["branch", "--delete", "topic"]);
            }
            "flag" => target.is_current = true,
            "current" => {
                target = selected(&repo, OsStr::new("main"));
                target.is_current = false;
            }
            _ => {
                std::fs::write(repo.path.join("extra"), b"staged").unwrap();
                repo.git(&["add", "extra"]);
            }
        }
        let before = repo.unchanged_state();
        assert!(
            rename_local_branch(&repo.path, &target, "renamed", &basis).is_err(),
            "{mutation}"
        );
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn selected_rename_rejects_occupied_invalid_and_orphan_config_destinations() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["branch", "topic"]);
    repo.git(&["branch", "occupied"]);
    repo.git(&["config", "branch.orphan.description", "saved setting"]);
    let target = selected(&repo, OsStr::new("topic"));
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let before = repo.unchanged_state();
    for name in [
        "topic",
        "occupied",
        "orphan",
        "",
        "-M",
        "HEAD",
        "@{-1}",
        "bad..name",
        "with space",
    ] {
        assert!(
            rename_local_branch(&repo.path, &target, name, &basis).is_err(),
            "{name}"
        );
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[test]
fn selected_rename_never_changes_a_branch_checked_out_in_another_worktree() {
    let repo = Fixture::new();
    repo.commit();
    repo.git(&["branch", "topic"]);
    let linked = repo.root.join("linked");
    repo.git(&["worktree", "add", linked.to_str().unwrap(), "topic"]);
    let target = selected(&repo, OsStr::new("topic"));
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    let linked_basis = snapshot(&linked).unwrap().commit_basis;
    let before = repo.unchanged_state();
    assert!(rename_local_branch(&repo.path, &target, "renamed", &basis).is_err());
    assert_eq!(repo.unchanged_state(), before);
    assert_eq!(snapshot(&linked).unwrap().commit_basis, linked_basis);
}

#[test]
fn selected_rename_is_blocked_by_active_sequencers_and_detached_head() {
    for marker in [
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "sequencer",
        "rebase-merge",
        "detached",
    ] {
        let repo = Fixture::new();
        repo.commit();
        repo.git(&["branch", "topic"]);
        if marker == "detached" {
            repo.git(&["switch", "--detach"]);
        } else if marker.ends_with("HEAD") {
            std::fs::write(
                repo.path.join(".git").join(marker),
                repo.git(&["rev-parse", "HEAD"]),
            )
            .unwrap();
        } else {
            std::fs::create_dir(repo.path.join(".git").join(marker)).unwrap();
        }
        let target = selected(&repo, OsStr::new("topic"));
        let basis = snapshot(&repo.path).unwrap().commit_basis;
        let before = repo.unchanged_state();
        assert!(
            rename_local_branch(&repo.path, &target, "renamed", &basis).is_err(),
            "{marker}"
        );
        assert_eq!(repo.unchanged_state(), before);
    }
}

#[cfg(unix)]
#[test]
fn selected_rename_uses_the_complete_native_source_name() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    repo.commit();
    let name = OsStr::from_bytes(b"topic-\xff");
    repo.git(&[OsStr::new("branch"), name]);
    let target = selected(&repo, name);
    let basis = snapshot(&repo.path).unwrap().commit_basis;
    rename_local_branch(&repo.path, &target, "renamed", &basis).unwrap();
    assert_eq!(snapshot(&repo.path).unwrap().commit_basis, basis);
    assert_eq!(selected(&repo, OsStr::new("renamed")).oid, target.oid);
}
