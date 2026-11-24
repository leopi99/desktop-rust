use desktop_core::{CommitBasis, LocalBranch, RebaseKind, RebasePreview};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
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
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-rebase-{}-{}",
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
        let f = Self { root, repo };
        f.write("base", b"base\n");
        f.commit("baseline");
        git(&f.repo, &["branch", "upstream"]);
        f
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.repo.join(name), bytes).unwrap();
    }
    fn commit(&self, message: &str) {
        git(&self.repo, &["add", "--all"]);
        git(&self.repo, &["commit", "--allow-empty", "-m", message]);
    }
    fn basis(&self) -> CommitBasis {
        desktop_git::snapshot(&self.repo).unwrap().commit_basis
    }
    fn target(&self) -> LocalBranch {
        desktop_git::snapshot(&self.repo)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.name == "upstream")
            .unwrap()
    }
    fn preview(&self) -> RebasePreview {
        desktop_git::preview_rebase(&self.repo, &self.target(), &self.basis()).unwrap()
    }
    fn upstream(&self, name: &str, bytes: &[u8]) {
        git(&self.repo, &["checkout", "upstream"]);
        self.write(name, bytes);
        self.commit("upstream commit");
        git(&self.repo, &["checkout", "main"]);
    }
    fn diverge(&self) {
        self.upstream("upstream-file", b"upstream\n");
        self.write("local-file", b"local\n");
        self.commit("local commit");
    }
    fn conflict(&self) {
        self.upstream("base", b"upstream side\n");
        self.write("base", b"local side\n");
        self.commit("local conflict commit");
    }
    fn start_conflict(&self) {
        self.conflict();
        let output = desktop_git::execute_rebase(&self.preview()).unwrap();
        assert!(output.contains("paused"));
        let active = self.basis().rebase.unwrap();
        assert!(active.supported, "{active:?}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn preview_is_read_only_and_linear_commits_are_replayed_on_reviewed_target() {
    let f = Fixture::new();
    f.diverge();
    let before = f.basis();
    let preview = f.preview();
    assert_eq!(preview.kind, RebaseKind::Replay);
    assert_eq!(preview.commits.len(), 1);
    assert_eq!(preview.commits[0].subject, "local commit");
    assert_eq!(f.basis(), before);
    desktop_git::execute_rebase(&preview).unwrap();
    let after = f.basis();
    assert_ne!(after.head, before.head);
    assert_eq!(after.branch, before.branch);
    assert!(after.rebase.is_none());
    assert_eq!(
        String::from_utf8(git(&f.repo, &["rev-parse", "HEAD~1"]))
            .unwrap()
            .trim(),
        preview.target.oid
    );
    assert_eq!(
        git(&f.repo, &["log", "-1", "--format=%s"]),
        b"local commit\n"
    );
    assert!(f.repo.join("local-file").exists());
    assert!(f.repo.join("upstream-file").exists());
}

#[test]
fn up_to_date_and_fast_forward_cases_do_not_rewrite_local_commits() {
    let f = Fixture::new();
    let old = f.basis();
    let preview = f.preview();
    assert_eq!(preview.kind, RebaseKind::AlreadyUpToDate);
    desktop_git::execute_rebase(&preview).unwrap();
    assert_eq!(f.basis(), old);
    f.upstream("new", b"upstream\n");
    let preview = f.preview();
    assert_eq!(preview.kind, RebaseKind::FastForward);
    desktop_git::execute_rebase(&preview).unwrap();
    assert_eq!(f.basis().head.as_ref().unwrap().trim(), preview.target.oid);
}

#[test]
fn conflicted_rebase_can_be_reviewed_resolved_and_continued_with_original_message() {
    let f = Fixture::new();
    f.start_conflict();
    let state = desktop_git::snapshot(&f.repo).unwrap();
    assert!(desktop_git::continue_rebase(&f.repo, &state.commit_basis).is_err());
    assert!(desktop_git::commit(&f.repo, "wrong manual commit", &state.commit_basis).is_err());
    assert!(desktop_git::save_stash(&f.repo, "during rebase", &state.commit_basis).is_err());
    assert!(desktop_git::switch_branch(&f.repo, &f.target(), &state.commit_basis).is_err());
    f.write("base", b"combined resolution\n");
    let conflict = state.changes.iter().find(|c| c.is_conflicted()).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, conflict).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    desktop_git::continue_rebase(&f.repo, &f.basis()).unwrap();
    assert!(f.basis().rebase.is_none());
    assert_eq!(
        f.basis().branch.as_deref(),
        Some(std::ffi::OsStr::new("refs/heads/main"))
    );
    assert_eq!(
        git(&f.repo, &["log", "-1", "--format=%s"]),
        b"local conflict commit\n"
    );
    assert_eq!(
        std::fs::read(f.repo.join("base")).unwrap(),
        b"combined resolution\n"
    );
}

#[test]
fn abort_restores_original_branch_and_preserves_unrelated_untracked_files() {
    let f = Fixture::new();
    f.conflict();
    let before = f.basis();
    desktop_git::execute_rebase(&f.preview()).unwrap();
    f.write("untracked", b"keep me\n");
    let message = desktop_git::abort_rebase(&f.repo, &f.basis()).unwrap();
    assert_eq!(message, "Rebase aborted");
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.repo.join("base")).unwrap(), b"local side\n");
    assert_eq!(
        std::fs::read(f.repo.join("untracked")).unwrap(),
        b"keep me\n"
    );
}

#[test]
fn stale_source_target_and_sequencer_state_are_rejected() {
    let f = Fixture::new();
    f.diverge();
    let preview = f.preview();
    f.write("untracked", b"keep");
    assert!(desktop_git::execute_rebase(&preview).is_err());
    std::fs::remove_file(f.repo.join("untracked")).unwrap();
    f.commit("new source HEAD");
    assert!(desktop_git::execute_rebase(&preview).is_err());
    let preview = f.preview();
    git(&f.repo, &["branch", "--force", "upstream", "HEAD"]);
    assert!(desktop_git::execute_rebase(&preview).is_err());
    let f = Fixture::new();
    f.start_conflict();
    let old = f.basis();
    std::fs::write(
        f.repo.join(".git/rebase-merge/git-rebase-todo"),
        b"# changed after review\n",
    )
    .unwrap();
    assert!(desktop_git::continue_rebase(&f.repo, &old).is_err());
    assert!(desktop_git::abort_rebase(&f.repo, &old).is_err());
}

#[test]
fn configured_autosquash_backend_and_update_refs_do_not_change_reviewed_policy() {
    let f = Fixture::new();
    f.diverge();
    let old = f.basis().head.unwrap();
    git(&f.repo, &["branch", "backup"]);
    for (key, value) in [
        ("rebase.updateRefs", "true"),
        ("rebase.autoSquash", "true"),
        ("rebase.autoStash", "true"),
        ("rebase.backend", "apply"),
    ] {
        git(&f.repo, &["config", key, value]);
    }
    desktop_git::execute_rebase(&f.preview()).unwrap();
    assert_eq!(git(&f.repo, &["rev-parse", "backup"]), old.as_bytes());
    assert!(f.basis().rebase.is_none());
    assert!(desktop_git::snapshot(&f.repo).unwrap().stashes.is_empty());
}

#[test]
fn commits_that_become_empty_are_kept_in_the_replayed_history() {
    let f = Fixture::new();
    f.upstream("duplicate", b"same patch\n");
    f.write("duplicate", b"same patch\n");
    f.commit("local duplicate patch");
    let preview = f.preview();
    assert_eq!(preview.commits.len(), 1);
    desktop_git::execute_rebase(&preview).unwrap();
    assert_eq!(
        git(&f.repo, &["log", "-1", "--format=%s"]),
        b"local duplicate patch\n"
    );
    assert_eq!(
        String::from_utf8(git(&f.repo, &["rev-parse", "HEAD~1"]))
            .unwrap()
            .trim(),
        preview.target.oid
    );
}

#[test]
fn ignored_file_collision_stops_rebase_before_checkout() {
    let f = Fixture::new();
    f.upstream("ignored.tmp", b"tracked upstream content");
    std::fs::write(f.repo.join(".git/info/exclude"), b"ignored.tmp\n").unwrap();
    f.write("ignored.tmp", b"local ignored content");
    let preview = f.preview();
    let before = f.basis();
    assert!(desktop_git::execute_rebase(&preview).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(f.repo.join("ignored.tmp")).unwrap(),
        b"local ignored content"
    );
}

#[test]
fn source_merge_commits_and_unrelated_histories_are_not_flattened() {
    let f = Fixture::new();
    f.upstream("upstream-file", b"upstream");
    git(&f.repo, &["checkout", "-b", "topic"]);
    f.write("topic", b"topic");
    f.commit("topic");
    git(&f.repo, &["checkout", "main"]);
    f.write("main", b"main");
    f.commit("main");
    git(&f.repo, &["merge", "--no-edit", "topic"]);
    assert!(
        desktop_git::preview_rebase(&f.repo, &f.target(), &f.basis())
            .unwrap_err()
            .to_string()
            .contains("merge commits")
    );
    git(&f.repo, &["checkout", "--orphan", "unrelated"]);
    f.commit("unrelated root");
    git(&f.repo, &["checkout", "main"]);
    let state = desktop_git::snapshot(&f.repo).unwrap();
    let target = state
        .branches
        .iter()
        .find(|b| b.name == "unrelated")
        .unwrap();
    assert!(desktop_git::preview_rebase(&f.repo, target, &state.commit_basis).is_err());
}

#[test]
fn custom_todo_commands_are_not_executed_by_continue() {
    let f = Fixture::new();
    f.start_conflict();
    std::fs::write(
        f.repo.join(".git/rebase-merge/git-rebase-todo"),
        b"exec touch should-not-exist\n",
    )
    .unwrap();
    let state = f.basis();
    assert!(!state.rebase.as_ref().unwrap().supported);
    assert!(desktop_git::continue_rebase(&f.repo, &state).is_err());
    assert!(!f.repo.join("should-not-exist").exists());
}

#[test]
fn original_branch_movement_blocks_continue_and_abort() {
    let f = Fixture::new();
    f.start_conflict();
    git(&f.repo, &["update-ref", "refs/heads/main", &f.target().oid]);
    let basis = f.basis();
    assert!(desktop_git::continue_rebase(&f.repo, &basis).is_err());
    assert!(desktop_git::abort_rebase(&f.repo, &basis).is_err());
    assert!(f.basis().rebase.is_some());
}

#[test]
fn linked_worktree_rebase_state_is_isolated_and_survives_reload() {
    let f = Fixture::new();
    f.conflict();
    let original = f.basis();
    let linked = f.root.join("linked");
    git(
        &f.repo,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().unwrap(),
            "main",
        ],
    );
    let basis = desktop_git::snapshot(&linked).unwrap().commit_basis;
    let preview = desktop_git::preview_rebase(&linked, &f.target(), &basis).unwrap();
    desktop_git::execute_rebase(&preview).unwrap();
    let state = desktop_git::snapshot(&linked).unwrap();
    assert!(state.commit_basis.rebase.as_ref().unwrap().supported);
    assert_eq!(
        state.commit_basis.rebase.as_ref().unwrap().branch,
        "refs/heads/linked"
    );
    assert_eq!(f.basis(), original);
    desktop_git::abort_rebase(&linked, &state.commit_basis).unwrap();
    assert!(desktop_git::snapshot(&linked)
        .unwrap()
        .commit_basis
        .rebase
        .is_none());
    assert_eq!(f.basis(), original);
}

#[test]
fn ignored_collisions_in_intermediate_replayed_commits_are_not_missed() {
    let f = Fixture::new();
    f.upstream("upstream-file", b"upstream");
    f.write("transient", b"temporary committed content");
    f.commit("add transient");
    std::fs::remove_file(f.repo.join("transient")).unwrap();
    f.commit("remove transient");
    std::fs::write(f.repo.join(".git/info/exclude"), b"transient\n").unwrap();
    f.write("transient", b"ignored local content");
    let preview = f.preview();
    assert_eq!(preview.commits.len(), 2);
    let before = f.basis();
    assert!(desktop_git::execute_rebase(&preview).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(f.repo.join("transient")).unwrap(),
        b"ignored local content"
    );
}

#[test]
fn ignored_file_created_while_paused_blocks_continue_until_moved_aside() {
    let f = Fixture::new();
    f.conflict();
    f.write("future", b"future committed content");
    f.commit("later commit");
    desktop_git::execute_rebase(&f.preview()).unwrap();
    f.write("base", b"resolved\n");
    let state = desktop_git::snapshot(&f.repo).unwrap();
    let conflict = state.changes.iter().find(|c| c.is_conflicted()).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, conflict).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    std::fs::write(f.repo.join(".git/info/exclude"), b"future\n").unwrap();
    f.write("future", b"new ignored work");
    let before = f.basis();
    assert!(desktop_git::continue_rebase(&f.repo, &before).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(f.repo.join("future")).unwrap(),
        b"new ignored work"
    );
    std::fs::rename(f.repo.join("future"), f.root.join("kept-future")).unwrap();
    desktop_git::continue_rebase(&f.repo, &f.basis()).unwrap();
    assert!(f.basis().rebase.is_none());
    assert_eq!(
        std::fs::read(f.repo.join("future")).unwrap(),
        b"future committed content"
    );
    assert_eq!(
        std::fs::read(f.root.join("kept-future")).unwrap(),
        b"new ignored work"
    );
}
