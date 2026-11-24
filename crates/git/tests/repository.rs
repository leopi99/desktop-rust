//! Git integration coverage for repository.
#![cfg(test)]

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "desktop-rust-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let fixture = Self(path);
        fixture.git(&["init"]);
        fixture.git(&["config", "user.email", "test@example.invalid"]);
        fixture.git(&["config", "user.name", "Test"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        fixture.git(&["config", "core.hooksPath", "/dev/null"]);
        fixture
    }
    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn write(&self, path: &str, content: &[u8]) {
        std::fs::write(self.0.join(path), content).unwrap();
    }
    fn commit(&self) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "-m", "fixture"]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_empty_repository_without_changing_working_tree() {
    let repo = Fixture::new();
    let snapshot = desktop_git::snapshot(&repo.0).unwrap();
    assert_eq!(snapshot.history, "No commits yet");
    assert!(snapshot.changes.is_empty());
    assert!(!repo.0.join(".git/index").exists());
}

#[test]
fn rejects_a_non_repository() {
    assert!(desktop_git::snapshot(Path::new("/dev/null")).is_err());
}

#[test]
fn separates_staged_and_unstaged_changes() {
    let repo = Fixture::new();
    repo.write("file.txt", b"base\n");
    repo.commit();
    repo.write("file.txt", b"staged\n");
    repo.git(&["add", "file.txt"]);
    repo.write("file.txt", b"working\n");
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let diff = desktop_git::diff(&repo.0, &state.changes[0]).unwrap();
    assert!(diff.staged.contains("+staged"));
    assert!(!diff.staged.contains("+working"));
    assert!(diff.unstaged.contains("+working"));
    assert!(diff.unstaged.contains("-staged"));
}

#[test]
fn shows_unborn_index_and_untracked_binary() {
    let repo = Fixture::new();
    repo.write("new.txt", b"hello\n");
    repo.git(&["add", "new.txt"]);
    repo.write("binary", b"a\0b");
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let added = state
        .changes
        .iter()
        .find(|c| c.path == Path::new("new.txt"))
        .unwrap();
    assert!(desktop_git::diff(&repo.0, added)
        .unwrap()
        .staged
        .contains("+hello"));
    let binary = state
        .changes
        .iter()
        .find(|c| c.path == Path::new("binary"))
        .unwrap();
    assert!(desktop_git::diff(&repo.0, binary)
        .unwrap()
        .unstaged
        .contains("Binary file"));
}

#[test]
fn rename_keeps_both_paths_and_diff() {
    let repo = Fixture::new();
    repo.write("old name.txt", b"content\n");
    repo.commit();
    repo.git(&["mv", "old name.txt", "new\nname.txt"]);
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let change = &state.changes[0];
    assert_eq!(change.path, Path::new("new\nname.txt"));
    assert_eq!(
        change.original_path.as_deref(),
        Some(Path::new("old name.txt"))
    );
    assert!(desktop_git::diff(&repo.0, change)
        .unwrap()
        .staged
        .contains("rename from"));
}

#[test]
fn literal_pathspec_cannot_select_another_file() {
    let repo = Fixture::new();
    repo.write(":(glob)*", b"base\n");
    repo.write("other", b"base\n");
    repo.commit();
    repo.write(":(glob)*", b"selected\n");
    repo.write("other", b"secret-other\n");
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let change = state
        .changes
        .iter()
        .find(|c| c.path == Path::new(":(glob)*"))
        .unwrap();
    let diff = desktop_git::diff(&repo.0, change).unwrap();
    assert!(diff.unstaged.contains("+selected"));
    assert!(!diff.unstaged.contains("secret-other"));
}

#[test]
fn selection_must_still_be_in_status() {
    let repo = Fixture::new();
    repo.write("file", b"new");
    let state = desktop_git::snapshot(&repo.0).unwrap();
    std::fs::remove_file(repo.0.join("file")).unwrap();
    assert!(desktop_git::diff(&repo.0, &state.changes[0]).is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_preserved() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"file-\xff");
    std::fs::write(repo.0.join(name), b"preview").unwrap();
    let state = desktop_git::snapshot(&repo.0).unwrap();
    assert_eq!(state.changes[0].path.as_os_str(), name);
    assert!(desktop_git::diff(&repo.0, &state.changes[0])
        .unwrap()
        .unstaged
        .contains("preview"));
}

#[cfg(unix)]
#[test]
fn untracked_symlink_is_not_followed() {
    let repo = Fixture::new();
    std::os::unix::fs::symlink("/etc/passwd", repo.0.join("link")).unwrap();
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let diff = desktop_git::diff(&repo.0, &state.changes[0]).unwrap();
    assert!(diff.unstaged.contains("Symbolic link"));
    assert!(!diff.unstaged.contains("root:"));
}

#[test]
fn deletion_and_binary_diff_do_not_use_external_helpers() {
    let repo = Fixture::new();
    repo.write("deleted", b"original\n");
    repo.write("binary", b"a\0b");
    repo.commit();
    repo.git(&[
        "config",
        "diff.external",
        "nonexistent-desktop-rust-diff-helper",
    ]);
    std::fs::remove_file(repo.0.join("deleted")).unwrap();
    repo.write("binary", b"c\0d");
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let deleted = state
        .changes
        .iter()
        .find(|c| c.path == Path::new("deleted"))
        .unwrap();
    assert!(desktop_git::diff(&repo.0, deleted)
        .unwrap()
        .unstaged
        .contains("-original"));
    let binary = state
        .changes
        .iter()
        .find(|c| c.path == Path::new("binary"))
        .unwrap();
    assert!(desktop_git::diff(&repo.0, binary)
        .unwrap()
        .unstaged
        .contains("Binary files"));
}

#[test]
fn reads_conflicted_file_and_resolves_subdirectory_root() {
    let repo = Fixture::new();
    repo.write("file", b"base\n");
    repo.commit();
    repo.git(&["checkout", "-b", "side"]);
    repo.write("file", b"side\n");
    repo.commit();
    repo.git(&["checkout", "-b", "other", "HEAD~1"]);
    repo.write("file", b"other\n");
    repo.commit();
    let merge = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["merge", "side"])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    std::fs::create_dir(repo.0.join("nested")).unwrap();
    let state = desktop_git::snapshot(&repo.0.join("nested")).unwrap();
    assert_eq!(state.repository.path, repo.0);
    let conflict = state
        .changes
        .iter()
        .find(|c| c.path == Path::new("file"))
        .unwrap();
    assert_eq!((conflict.index, conflict.worktree), ('U', 'U'));
    assert!(desktop_git::create_branch(&repo.0, "conflicted", &state.commit_basis).is_err());
    let target = state
        .branches
        .iter()
        .find(|branch| !branch.is_current)
        .unwrap();
    assert!(desktop_git::switch_branch(&repo.0, target, &state.commit_basis).is_err());
    assert!(desktop_git::stage(&repo.0, conflict).is_err());
    assert!(desktop_git::unstage(&repo.0, conflict).is_err());
    assert!(desktop_git::commit(&repo.0, "unresolved", &state.commit_basis).is_err());
    assert!(!desktop_git::diff(&state.repository.path, conflict)
        .unwrap()
        .unstaged
        .is_empty());
}

fn change(repo: &Fixture, name: &str) -> desktop_core::FileChange {
    desktop_git::snapshot(&repo.0)
        .unwrap()
        .changes
        .into_iter()
        .find(|c| c.path == Path::new(name))
        .unwrap()
}

#[test]
fn stage_and_unstage_unborn_file_preserve_working_contents() {
    let repo = Fixture::new();
    repo.write("new", b"first");
    desktop_git::stage(&repo.0, &change(&repo, "new")).unwrap();
    assert_eq!(change(&repo, "new").index, 'A');
    // Unstage must preserve working content even when it differs from the index.
    repo.write("new", b"second");
    desktop_git::unstage(&repo.0, &change(&repo, "new")).unwrap();
    assert_eq!(change(&repo, "new").index, '?');
    assert_eq!(std::fs::read(repo.0.join("new")).unwrap(), b"second");
}

#[test]
fn stage_deletion_and_unstage_rename_preserve_worktree() {
    let repo = Fixture::new();
    repo.write("old", b"content");
    repo.commit();
    repo.git(&["mv", "old", "new"]);
    desktop_git::unstage(&repo.0, &change(&repo, "new")).unwrap();
    assert_eq!(std::fs::read(repo.0.join("new")).unwrap(), b"content");
    assert!(!repo.0.join("old").exists());
    desktop_git::stage(&repo.0, &change(&repo, "old")).unwrap();
    assert_eq!(change(&repo, "old").index, 'D');
    desktop_git::unstage(&repo.0, &change(&repo, "old")).unwrap();
    assert_eq!(change(&repo, "old").worktree, 'D');
}

#[test]
fn commit_uses_index_only_and_supports_multiline_messages() {
    let repo = Fixture::new();
    repo.write("file", b"staged");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    repo.write("file", b"unstaged");
    repo.write("other", b"untracked");
    let basis = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    desktop_git::commit(&repo.0, "--literal summary\n\nDescription", &basis).unwrap();
    let show = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["show", "HEAD:file"])
        .output()
        .unwrap();
    assert_eq!(show.stdout, b"staged");
    let message = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["log", "-1", "--format=%B"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&message.stdout).contains("--literal summary\n\nDescription"));
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"unstaged");
    assert_eq!(change(&repo, "other").index, '?');
}

#[test]
fn commit_rejects_empty_message_empty_index_and_stale_index() {
    let repo = Fixture::new();
    let empty = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    assert!(desktop_git::commit(&repo.0, "message", &empty).is_err());
    repo.write("file", b"first");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    let basis = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    assert!(desktop_git::commit(&repo.0, " \n\t", &basis).is_err());
    repo.write("file", b"second");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    let error = desktop_git::commit(&repo.0, "message", &basis).unwrap_err();
    assert!(error.to_string().contains("HEAD changed"));
    assert_eq!(change(&repo, "file").index, 'A');
}

#[test]
fn commit_rejects_branch_switch_with_same_index() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    repo.write("file", b"staged");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    let basis = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    repo.git(&["checkout", "-b", "other"]);
    assert!(desktop_git::commit(&repo.0, "message", &basis).is_err());
}

#[test]
fn stage_is_literal_and_rejects_stale_status() {
    let repo = Fixture::new();
    repo.write(":(glob)*", b"selected");
    repo.write("other", b"other");
    let selected = change(&repo, ":(glob)*");
    desktop_git::stage(&repo.0, &selected).unwrap();
    assert_eq!(change(&repo, "other").index, '?');
    assert!(desktop_git::stage(&repo.0, &selected).is_err());
    desktop_git::unstage(&repo.0, &change(&repo, ":(glob)*")).unwrap();
    assert_eq!(change(&repo, ":(glob)*").index, '?');
}

#[cfg(unix)]
#[test]
fn failing_hook_preserves_index_and_returns_diagnostic() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Fixture::new();
    repo.write("file", b"staged");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    repo.git(&["config", "--unset", "core.hooksPath"]);
    let hook = repo.0.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        b"#!/bin/sh\necho 'hook rejected this commit' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let basis = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    let error = desktop_git::commit(&repo.0, "message", &basis).unwrap_err();
    assert!(error.to_string().contains("hook rejected this commit"));
    assert_eq!(desktop_git::snapshot(&repo.0).unwrap().commit_basis, basis);
}

fn branch(repo: &Fixture, name: &str) -> desktop_core::LocalBranch {
    desktop_git::snapshot(&repo.0)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.name == std::ffi::OsStr::new(name))
        .unwrap()
}

fn basis(repo: &Fixture) -> desktop_core::CommitBasis {
    desktop_git::snapshot(&repo.0).unwrap().commit_basis
}

#[test]
fn creates_and_switches_local_branches() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    let original = desktop_git::snapshot(&repo.0).unwrap().branches[0].clone();
    desktop_git::create_branch(&repo.0, "feature/nested", &basis(&repo)).unwrap();
    assert!(branch(&repo, "feature/nested").is_current);
    desktop_git::switch_branch(&repo.0, &original, &basis(&repo)).unwrap();
    assert!(branch(&repo, original.name.to_str().unwrap()).is_current);
}

#[test]
fn creates_initial_branch_and_recovers_detached_head() {
    let repo = Fixture::new();
    desktop_git::create_branch(&repo.0, "initial", &basis(&repo)).unwrap();
    let state = desktop_git::snapshot(&repo.0).unwrap();
    assert_eq!(
        state.commit_basis.branch.as_deref(),
        Some(std::ffi::OsStr::new("refs/heads/initial"))
    );
    assert!(state.branches.is_empty());
    repo.write("file", b"base");
    repo.commit();
    let initial = branch(&repo, "initial");
    repo.git(&["checkout", "--detach", "HEAD"]);
    assert!(basis(&repo).branch.is_none());
    desktop_git::create_branch(&repo.0, "recovered", &basis(&repo)).unwrap();
    desktop_git::switch_branch(&repo.0, &initial, &basis(&repo)).unwrap();
    assert!(branch(&repo, "initial").is_current);
}

#[test]
fn branch_validation_rejects_options_shorthand_and_existing_names() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    let before = basis(&repo);
    for name in [
        "",
        "--force",
        "-",
        "HEAD",
        "@{-1}",
        "bad name",
        "a..b",
        "bad.lock",
        "bad\nname",
    ] {
        assert!(
            desktop_git::create_branch(&repo.0, name, &before).is_err(),
            "{name}"
        );
        assert_eq!(basis(&repo), before);
    }
    desktop_git::create_branch(&repo.0, "existing", &before).unwrap();
    let current = basis(&repo);
    assert!(desktop_git::create_branch(&repo.0, "existing", &current).is_err());
    assert_eq!(basis(&repo), current);
}

#[test]
fn switching_rejects_overwriting_local_changes() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    let original = desktop_git::snapshot(&repo.0).unwrap().branches[0].clone();
    repo.git(&["checkout", "-b", "target"]);
    repo.write("file", b"target");
    repo.commit();
    let target = branch(&repo, "target");
    desktop_git::switch_branch(&repo.0, &original, &basis(&repo)).unwrap();
    repo.write("file", b"local edits");
    let before = basis(&repo);
    assert!(desktop_git::switch_branch(&repo.0, &target, &before).is_err());
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"local edits");
    assert_eq!(basis(&repo), before);
}

#[test]
fn switching_rejects_overwriting_untracked_files() {
    let repo = Fixture::new();
    repo.write("base", b"base");
    repo.commit();
    let original = desktop_git::snapshot(&repo.0).unwrap().branches[0].clone();
    repo.git(&["checkout", "-b", "target"]);
    repo.write("new", b"tracked");
    repo.commit();
    let target = branch(&repo, "target");
    desktop_git::switch_branch(&repo.0, &original, &basis(&repo)).unwrap();
    repo.write("new", b"untracked content");
    assert!(desktop_git::switch_branch(&repo.0, &target, &basis(&repo)).is_err());
    assert_eq!(
        std::fs::read(repo.0.join("new")).unwrap(),
        b"untracked content"
    );
}

#[test]
fn compatible_dirty_index_is_preserved_when_creating_and_switching() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    let original = desktop_git::snapshot(&repo.0).unwrap().branches[0].clone();
    repo.write("file", b"staged");
    desktop_git::stage(&repo.0, &change(&repo, "file")).unwrap();
    repo.write("file", b"unstaged");
    repo.write("other", b"untracked");
    let before = basis(&repo).index;
    desktop_git::create_branch(&repo.0, "dirty", &basis(&repo)).unwrap();
    desktop_git::switch_branch(&repo.0, &original, &basis(&repo)).unwrap();
    assert_eq!(basis(&repo).index, before);
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"unstaged");
    assert_eq!(std::fs::read(repo.0.join("other")).unwrap(), b"untracked");
}

#[test]
fn stale_source_or_target_does_not_switch() {
    let repo = Fixture::new();
    repo.write("file", b"first");
    repo.commit();
    repo.git(&["branch", "target"]);
    let target = branch(&repo, "target");
    let old = basis(&repo);
    repo.write("file", b"second");
    repo.commit();
    assert!(desktop_git::switch_branch(&repo.0, &target, &old).is_err());
    assert!(desktop_git::create_branch(&repo.0, "stale", &old).is_err());
    repo.git(&["branch", "--force", "target", "HEAD"]);
    assert!(desktop_git::switch_branch(&repo.0, &target, &basis(&repo)).is_err());
    let target = branch(&repo, "target");
    repo.git(&["branch", "-D", "target"]);
    assert!(desktop_git::switch_branch(&repo.0, &target, &basis(&repo)).is_err());
}

#[test]
fn local_branch_wins_over_identically_named_tag() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    repo.git(&["tag", "target"]);
    repo.git(&["checkout", "-b", "target"]);
    repo.write("file", b"branch commit");
    repo.commit();
    let target = branch(&repo, "target");
    repo.git(&["checkout", "-b", "other", "HEAD~1"]);
    desktop_git::switch_branch(&repo.0, &target, &basis(&repo)).unwrap();
    assert_eq!(
        std::fs::read(repo.0.join("file")).unwrap(),
        b"branch commit"
    );
    assert!(branch(&repo, "target").is_current);
}

#[test]
fn switching_respects_linked_worktree_exclusivity() {
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    repo.git(&["branch", "target"]);
    repo.git(&[
        "worktree",
        "add",
        "--quiet",
        "--no-checkout",
        "linked",
        "target",
    ]);
    let before = basis(&repo);
    assert!(desktop_git::switch_branch(&repo.0, &branch(&repo, "target"), &before).is_err());
    assert_eq!(basis(&repo), before);
}

#[cfg(unix)]
#[test]
fn branch_list_and_switch_preserve_non_utf8_names() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    let name = std::ffi::OsStr::from_bytes(b"feature-\xff");
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .arg("branch")
        .arg(name)
        .output()
        .unwrap();
    assert!(output.status.success());
    let target = desktop_git::snapshot(&repo.0)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.name == name)
        .unwrap();
    desktop_git::switch_branch(&repo.0, &target, &basis(&repo)).unwrap();
    let current = desktop_git::snapshot(&repo.0)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.is_current)
        .unwrap();
    assert_eq!(current.name, name);
}

#[cfg(unix)]
#[test]
fn non_utf8_branch_changes_invalidate_the_commit_basis() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    repo.write("file", b"base");
    repo.commit();
    for name in [b"feature-\xff".as_slice(), b"feature-\xfe".as_slice()] {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo.0)
            .arg("branch")
            .arg(std::ffi::OsStr::from_bytes(name))
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    let branches = desktop_git::snapshot(&repo.0).unwrap().branches;
    let first = branches
        .iter()
        .find(|b| b.name == std::ffi::OsStr::from_bytes(b"feature-\xff"))
        .unwrap();
    let second = branches
        .iter()
        .find(|b| b.name == std::ffi::OsStr::from_bytes(b"feature-\xfe"))
        .unwrap();
    desktop_git::switch_branch(&repo.0, first, &basis(&repo)).unwrap();
    let old = basis(&repo);
    desktop_git::switch_branch(&repo.0, second, &old).unwrap();
    assert_ne!(basis(&repo), old);
    assert!(desktop_git::create_branch(&repo.0, "stale", &old).is_err());
}

fn git_at(path: &Path, args: &[&str]) -> Vec<u8> {
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

struct RemoteFixture {
    _root: Fixture,
    seed: PathBuf,
    local: PathBuf,
    bare: PathBuf,
}

impl RemoteFixture {
    fn new() -> Self {
        let root = Fixture::new();
        root.git(&["init", "--bare", "--initial-branch=main", "remote.git"]);
        root.git(&["init", "--initial-branch=main", "seed"]);
        let seed = root.0.join("seed");
        let bare = root.0.join("remote.git");
        for (key, value) in [
            ("user.email", "test@example.invalid"),
            ("user.name", "Test"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git_at(&seed, &["config", key, value]);
        }
        std::fs::write(seed.join("file"), b"base\n").unwrap();
        git_at(&seed, &["add", "file"]);
        git_at(&seed, &["commit", "-m", "initial"]);
        git_at(&seed, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_at(&seed, &["push", "-u", "origin", "main"]);
        let local =
            desktop_git::clone_repository(bare.to_str().unwrap(), &root.0.join("local")).unwrap();
        for (key, value) in [
            ("user.email", "test@example.invalid"),
            ("user.name", "Test"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git_at(&local, &["config", key, value]);
        }
        Self {
            _root: root,
            seed,
            local,
            bare,
        }
    }
    fn remote(&self) -> desktop_core::Remote {
        desktop_git::snapshot(&self.local)
            .unwrap()
            .remotes
            .into_iter()
            .find(|r| r.name == "origin")
            .unwrap()
    }
    fn action(&self, action: desktop_core::RemoteAction) -> std::io::Result<String> {
        let snapshot = desktop_git::snapshot(&self.local).unwrap();
        desktop_git::remote_operation(&self.local, &self.remote(), action, &snapshot.commit_basis)
    }
    fn advance(&self) {
        std::fs::write(self.seed.join("file"), b"remote update\n").unwrap();
        git_at(&self.seed, &["commit", "-am", "remote update"]);
        git_at(&self.seed, &["push", "origin", "main"]);
    }
    fn local_commit(&self) {
        std::fs::write(self.local.join("local-file"), b"local\n").unwrap();
        git_at(&self.local, &["add", "local-file"]);
        git_at(&self.local, &["commit", "-m", "local update"]);
    }
}

#[test]
fn clone_and_fetch_only_update_remote_refs() {
    let repo = RemoteFixture::new();
    assert_eq!(std::fs::read(repo.local.join("file")).unwrap(), b"base\n");
    let before = desktop_git::snapshot(&repo.local).unwrap().commit_basis;
    repo.advance();
    repo.action(desktop_core::RemoteAction::Fetch).unwrap();
    assert_eq!(
        desktop_git::snapshot(&repo.local).unwrap().commit_basis,
        before
    );
    assert_ne!(
        git_at(&repo.local, &["rev-parse", "HEAD"]),
        git_at(&repo.local, &["rev-parse", "origin/main"])
    );
    assert_eq!(std::fs::read(repo.local.join("file")).unwrap(), b"base\n");
}

#[test]
fn pull_fast_forwards_but_never_merges_diverged_history() {
    let repo = RemoteFixture::new();
    repo.advance();
    repo.action(desktop_core::RemoteAction::Pull).unwrap();
    assert_eq!(
        std::fs::read(repo.local.join("file")).unwrap(),
        b"remote update\n"
    );
    repo.local_commit();
    std::fs::write(repo.seed.join("another"), b"remote\n").unwrap();
    git_at(&repo.seed, &["add", "another"]);
    git_at(&repo.seed, &["commit", "-m", "diverge"]);
    git_at(&repo.seed, &["push", "origin", "main"]);
    let before = git_at(&repo.local, &["rev-parse", "HEAD"]);
    assert!(repo.action(desktop_core::RemoteAction::Pull).is_err());
    assert_eq!(git_at(&repo.local, &["rev-parse", "HEAD"]), before);
    assert!(!repo.local.join(".git/MERGE_HEAD").exists());
    assert!(repo.local.join("local-file").exists());
}

#[test]
fn pull_rejects_overwriting_local_changes_even_with_autostash_configured() {
    let repo = RemoteFixture::new();
    repo.advance();
    git_at(&repo.local, &["config", "merge.autoStash", "true"]);
    std::fs::write(repo.local.join("file"), b"local edits\n").unwrap();
    let before = git_at(&repo.local, &["rev-parse", "HEAD"]);
    assert!(repo.action(desktop_core::RemoteAction::Pull).is_err());
    assert_eq!(git_at(&repo.local, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read(repo.local.join("file")).unwrap(),
        b"local edits\n"
    );
    assert!(git_at(&repo.local, &["stash", "list"]).is_empty());
}

#[test]
fn push_sends_only_current_branch_even_with_mirror_and_tag_configuration() {
    let repo = RemoteFixture::new();
    repo.local_commit();
    git_at(&repo.local, &["branch", "extra"]);
    git_at(&repo.local, &["tag", "-a", "release", "-m", "tag"]);
    git_at(&repo.local, &["config", "remote.origin.mirror", "true"]);
    git_at(&repo.local, &["config", "push.followTags", "true"]);
    git_at(
        &repo.local,
        &["config", "remote.origin.push", "+refs/*:refs/*"],
    );
    repo.action(desktop_core::RemoteAction::Push).unwrap();
    assert_eq!(
        git_at(&repo.bare, &["rev-parse", "refs/heads/main"]),
        git_at(&repo.local, &["rev-parse", "HEAD"])
    );
    assert_eq!(
        git_at(&repo.bare, &["for-each-ref", "--format=%(refname)"]),
        b"refs/heads/main\n"
    );
}

#[test]
fn rejected_push_does_not_force_remote_or_change_local_index() {
    let repo = RemoteFixture::new();
    repo.advance();
    repo.local_commit();
    let remote_head = git_at(&repo.bare, &["rev-parse", "HEAD"]);
    let before = desktop_git::snapshot(&repo.local).unwrap().commit_basis;
    assert!(repo.action(desktop_core::RemoteAction::Push).is_err());
    assert_eq!(git_at(&repo.bare, &["rev-parse", "HEAD"]), remote_head);
    assert_eq!(
        desktop_git::snapshot(&repo.local).unwrap().commit_basis,
        before
    );
}

#[test]
fn publishing_branch_sets_upstream_and_honors_differently_named_upstream() {
    let repo = RemoteFixture::new();
    let basis = desktop_git::snapshot(&repo.local).unwrap().commit_basis;
    desktop_git::create_branch(&repo.local, "feature", &basis).unwrap();
    repo.local_commit();
    repo.action(desktop_core::RemoteAction::Push).unwrap();
    assert_eq!(
        git_at(&repo.local, &["config", "branch.feature.merge"]),
        b"refs/heads/feature\n"
    );
    git_at(&repo.bare, &["branch", "published", "main"]);
    git_at(
        &repo.local,
        &["config", "branch.feature.merge", "refs/heads/published"],
    );
    repo.action(desktop_core::RemoteAction::Push).unwrap();
    assert_eq!(
        git_at(&repo.bare, &["rev-parse", "published"]),
        git_at(&repo.local, &["rev-parse", "HEAD"])
    );
}

#[test]
fn stale_remote_or_source_basis_is_rejected_before_transfer() {
    let repo = RemoteFixture::new();
    let old = desktop_git::snapshot(&repo.local).unwrap();
    git_at(
        &repo.local,
        &[
            "remote",
            "set-url",
            "origin",
            "/nonexistent-desktop-rust-remote",
        ],
    );
    let error = desktop_git::remote_operation(
        &repo.local,
        &old.remotes[0],
        desktop_core::RemoteAction::Fetch,
        &old.commit_basis,
    )
    .unwrap_err();
    assert!(error.to_string().contains("configuration changed"));
    git_at(
        &repo.local,
        &["remote", "set-url", "origin", repo.bare.to_str().unwrap()],
    );
    repo.local_commit();
    let error = desktop_git::remote_operation(
        &repo.local,
        &repo.remote(),
        desktop_core::RemoteAction::Push,
        &old.commit_basis,
    )
    .unwrap_err();
    assert!(error.to_string().contains("HEAD changed"));
}

#[test]
fn detached_head_can_fetch_but_cannot_pull_or_push() {
    let repo = RemoteFixture::new();
    git_at(&repo.local, &["checkout", "--detach", "HEAD"]);
    repo.action(desktop_core::RemoteAction::Fetch).unwrap();
    assert!(repo.remote().target_branch.is_none());
    assert!(repo.action(desktop_core::RemoteAction::Pull).is_err());
    assert!(repo.action(desktop_core::RemoteAction::Push).is_err());
}

#[test]
fn clone_refuses_existing_destinations_and_invalid_transports() {
    let repo = RemoteFixture::new();
    let empty = repo._root.0.join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert!(desktop_git::clone_repository(repo.bare.to_str().unwrap(), &empty).is_err());
    assert!(std::fs::read_dir(empty).unwrap().next().is_none());
    for source in [
        "",
        "--upload-pack=bad",
        "ext::bad",
        "unknown://example.test/repo",
    ] {
        let target = repo._root.0.join("invalid");
        assert!(desktop_git::clone_repository(source, &target).is_err());
        assert!(!target.exists());
    }
}

#[test]
fn remote_tracking_on_another_remote_disables_pull_and_push() {
    let repo = RemoteFixture::new();
    git_at(
        &repo.local,
        &["remote", "add", "other", repo.bare.to_str().unwrap()],
    );
    let state = desktop_git::snapshot(&repo.local).unwrap();
    let other = state.remotes.iter().find(|r| r.name == "other").unwrap();
    assert!(other.target_branch.is_none());
    desktop_git::remote_operation(
        &repo.local,
        other,
        desktop_core::RemoteAction::Fetch,
        &state.commit_basis,
    )
    .unwrap();
    assert!(desktop_git::remote_operation(
        &repo.local,
        other,
        desktop_core::RemoteAction::Push,
        &state.commit_basis
    )
    .is_err());
}

#[test]
fn missing_remote_returns_an_error_without_changing_local_contents() {
    let repo = RemoteFixture::new();
    git_at(
        &repo.local,
        &[
            "remote",
            "set-url",
            "origin",
            repo._root.0.join("missing").to_str().unwrap(),
        ],
    );
    let before = desktop_git::snapshot(&repo.local).unwrap().commit_basis;
    assert!(repo.action(desktop_core::RemoteAction::Fetch).is_err());
    assert_eq!(
        desktop_git::snapshot(&repo.local).unwrap().commit_basis,
        before
    );
    assert_eq!(std::fs::read(repo.local.join("file")).unwrap(), b"base\n");
}

#[test]
fn pre_cancelled_clone_never_creates_destination() {
    let root = Fixture::new();
    let control = desktop_git::TransferControl::default();
    control.cancel();
    let target = root.0.join("cancelled-clone");
    let error = desktop_git::clone_repository_with_control("/unused/source", &target, &control)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert!(!target.exists());
}

#[test]
fn large_tracked_previews_preserve_index_and_worktree_and_still_stage_full_files() {
    let repo = Fixture::new();
    repo.write("file", b"base\n");
    repo.commit();
    let staged = "staged line\n".repeat(220_000);
    repo.write("file", staged.as_bytes());
    repo.git(&["add", "--", "file"]);
    let working = "working line\n".repeat(230_000);
    repo.write("file", working.as_bytes());
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let index = std::fs::read(repo.0.join(".git/index")).unwrap();
    let diff = desktop_git::diff(&repo.0, &state.changes[0]).unwrap();
    assert!(diff.staged.contains("+staged line"));
    assert!(diff.unstaged.contains("-staged line"));
    assert!(!diff.staged.contains("working line"));
    assert!(diff.staged.ends_with("[Staged diff preview truncated]"));
    assert!(diff.unstaged.ends_with("[Unstaged diff preview truncated]"));
    assert!(diff.staged.len() < 257 * 1024);
    assert!(diff.unstaged.len() < 257 * 1024);
    assert_eq!(std::fs::read(repo.0.join(".git/index")).unwrap(), index);
    assert_eq!(
        std::fs::read(repo.0.join("file")).unwrap(),
        working.as_bytes()
    );
    assert_eq!(
        desktop_git::snapshot(&repo.0).unwrap().commit_basis,
        state.commit_basis
    );
    desktop_git::stage(&repo.0, &state.changes[0]).unwrap();
    let stored = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["show", ":file"])
        .output()
        .unwrap();
    assert!(stored.status.success());
    assert_eq!(stored.stdout, working.as_bytes());
    assert_eq!(
        std::fs::read(repo.0.join("file")).unwrap(),
        working.as_bytes()
    );
}

#[test]
fn large_unborn_staged_patch_is_marked_without_inventing_an_unstaged_diff() {
    let repo = Fixture::new();
    repo.write("new", &vec![b'x'; 2 * 1024 * 1024]);
    repo.git(&["add", "--", "new"]);
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let before = std::fs::read(repo.0.join(".git/index")).unwrap();
    let diff = desktop_git::diff(&repo.0, &state.changes[0]).unwrap();
    assert!(diff.staged.contains("+xxxxxxxx"));
    assert!(diff.staged.ends_with("[Staged diff preview truncated]"));
    assert!(diff.staged.len() < 257 * 1024);
    assert!(diff.unstaged.is_empty());
    assert_eq!(std::fs::read(repo.0.join(".git/index")).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn clipped_tracked_diff_keeps_native_path_and_disables_textconv() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"file-\xff");
    std::fs::write(repo.0.join(name), b"base\n").unwrap();
    repo.commit();
    repo.git(&["config", "diff.external", "nonexistent-diff-helper"]);
    repo.git(&[
        "config",
        "diff.custom.textconv",
        "nonexistent-textconv-helper",
    ]);
    std::fs::write(repo.0.join(".git/info/attributes"), b"* diff=custom\n").unwrap();
    let content = vec![b'x'; 2 * 1024 * 1024];
    std::fs::write(repo.0.join(name), &content).unwrap();
    let state = desktop_git::snapshot(&repo.0).unwrap();
    let selected = state
        .changes
        .iter()
        .find(|change| change.path.as_os_str() == name)
        .unwrap();
    let diff = desktop_git::diff(&repo.0, selected).unwrap();
    assert!(diff.staged.is_empty());
    assert!(diff.unstaged.contains("+xxxxxxxx"));
    assert!(diff.unstaged.ends_with("[Unstaged diff preview truncated]"));
    assert!(diff.unstaged.len() < 257 * 1024);
    assert_eq!(std::fs::read(repo.0.join(name)).unwrap(), content);
}

#[cfg(unix)]
fn noisy_commit_hook(repo: &Fixture, reject: bool) {
    use std::os::unix::fs::PermissionsExt;
    let hooks = repo.0.join(".git/hooks");
    repo.git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, format!("#!/bin/sh\nhead -c 2097152 /dev/zero\nhead -c 2097152 /dev/zero >&2\nprintf 'hook diagnostic tail' >&2\nexit {}\n", if reject { 1 } else { 0 })).unwrap();
    std::fs::set_permissions(hook, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn noisy_rejected_hook_keeps_staged_and_working_data_and_allows_retry() {
    let repo = Fixture::new();
    repo.write("file", b"base\n");
    repo.commit();
    repo.write("file", b"staged\n");
    repo.git(&["add", "--", "file"]);
    repo.write("file", b"working\n");
    repo.write("untracked", b"keep\n");
    let before = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    noisy_commit_hook(&repo, true);
    let message = format!("Large rejected commit\n\n{}", "body line\n".repeat(250_000));
    let error = desktop_git::commit(&repo.0, &message, &before)
        .unwrap_err()
        .to_string();
    // Git routes hook stdout to its stderr as well.
    assert!(error.contains("Git stderr earlier output truncated"));
    assert!(error.ends_with("hook diagnostic tail"));
    assert!(error.len() < 2 * 64 * 1024 + 200);
    assert_eq!(desktop_git::snapshot(&repo.0).unwrap().commit_basis, before);
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"working\n");
    assert_eq!(std::fs::read(repo.0.join("untracked")).unwrap(), b"keep\n");
    repo.git(&["config", "core.hooksPath", "/dev/null"]);
    desktop_git::commit(&repo.0, "Retry after hook failure", &before).unwrap();
    let committed = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["show", "HEAD:file"])
        .output()
        .unwrap();
    assert!(committed.status.success());
    assert_eq!(committed.stdout, b"staged\n");
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"working\n");
}

#[cfg(unix)]
#[test]
fn noisy_successful_hook_keeps_full_commit_message_and_unstaged_data() {
    let repo = Fixture::new();
    repo.write("file", b"staged\n");
    repo.git(&["add", "--", "file"]);
    repo.write("file", b"working\n");
    let before = desktop_git::snapshot(&repo.0).unwrap().commit_basis;
    noisy_commit_hook(&repo, false);
    let message = format!(
        "Large successful commit\n\n{}",
        "body line\n".repeat(250_000)
    );
    let result = desktop_git::commit(&repo.0, &message, &before).unwrap();
    assert!(result.contains("Large successful commit"));
    assert!(!result.contains("hook diagnostic tail"));
    assert!(result.len() < 64 * 1024 + 100);
    let stored = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["log", "-1", "--format=%B"])
        .output()
        .unwrap();
    assert!(stored.status.success());
    assert_eq!(
        String::from_utf8(stored.stdout)
            .unwrap()
            .trim_end_matches('\n'),
        message.trim_end_matches('\n')
    );
    let committed = Command::new("git")
        .arg("-C")
        .arg(&repo.0)
        .args(["show", "HEAD:file"])
        .output()
        .unwrap();
    assert!(committed.status.success());
    assert_eq!(committed.stdout, b"staged\n");
    assert_eq!(std::fs::read(repo.0.join("file")).unwrap(), b"working\n");
}
