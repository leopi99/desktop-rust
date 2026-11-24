use desktop_core::{CommitBasis, LocalBranch, MergeKind, MergePreview};
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
            "desktop-merge-{}-{}",
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
        let fixture = Self { root, repo };
        fixture.write("base", b"baseline\n");
        fixture.commit("baseline");
        git(&fixture.repo, &["branch", "feature"]);
        fixture
    }
    fn write(&self, name: &str, content: &[u8]) {
        std::fs::write(self.repo.join(name), content).unwrap();
    }
    fn commit(&self, message: &str) {
        git(&self.repo, &["add", "--all"]);
        git(&self.repo, &["commit", "--allow-empty", "-m", message]);
    }
    fn target(&self) -> LocalBranch {
        desktop_git::snapshot(&self.repo)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.name == "feature")
            .unwrap()
    }
    fn basis(&self) -> CommitBasis {
        desktop_git::snapshot(&self.repo).unwrap().commit_basis
    }
    fn preview(&self) -> MergePreview {
        desktop_git::preview_merge(&self.repo, &self.target(), &self.basis()).unwrap()
    }
    fn advance_feature(&self, name: &str, content: &[u8]) {
        git(&self.repo, &["checkout", "feature"]);
        self.write(name, content);
        self.commit("feature change");
        git(&self.repo, &["checkout", "main"]);
    }
    fn diverge(&self) {
        self.advance_feature("feature-file", b"feature\n");
        self.write("main-file", b"main\n");
        self.commit("main change");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn up_to_date_and_fast_forward_previews_preserve_repository_until_execution() {
    let f = Fixture::new();
    let before = f.basis();
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::AlreadyUpToDate);
    desktop_git::execute_merge(&preview).unwrap();
    assert_eq!(f.basis(), before);
    f.advance_feature("feature-file", b"feature\n");
    let before = f.basis();
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::FastForward);
    assert_eq!(f.basis(), before);
    assert!(!f.repo.join("feature-file").exists());
    desktop_git::execute_merge(&preview).unwrap();
    assert_eq!(f.basis().head.as_ref().unwrap().trim(), preview.target.oid);
    assert!(f.basis().merge_heads.is_none());
    assert!(f.repo.join("feature-file").exists());
}

#[test]
fn divergent_clean_merge_stops_for_review_and_creates_two_parent_commit() {
    let f = Fixture::new();
    f.diverge();
    let before = f.basis();
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::CleanMerge);
    assert_eq!(f.basis(), before);
    desktop_git::execute_merge(&preview).unwrap();
    let state = desktop_git::snapshot(&f.repo).unwrap();
    assert_eq!(state.commit_basis.head, before.head);
    assert!(state.merge_message.is_some());
    assert!(state.commit_basis.merge_heads.is_some());
    assert!(state
        .changes
        .iter()
        .any(|c| c.path == Path::new("feature-file")));
    desktop_git::commit(&f.repo, "Merge feature", &state.commit_basis).unwrap();
    assert!(f.basis().merge_heads.is_none());
    assert_eq!(
        String::from_utf8(git(&f.repo, &["rev-list", "--parents", "-1", "HEAD"]))
            .unwrap()
            .split_whitespace()
            .count(),
        3
    );
}

#[test]
fn conflicts_are_predicted_and_can_be_resolved_before_merge_commit() {
    let f = Fixture::new();
    f.advance_feature("base", b"feature side\n");
    f.write("base", b"main side\n");
    f.commit("main change");
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::Conflicts);
    assert_eq!(preview.conflicts, vec![PathBuf::from("base")]);
    assert!(desktop_git::execute_merge(&preview)
        .unwrap()
        .contains("paused with conflicts"));
    f.write("base", b"combined result\n");
    let state = desktop_git::snapshot(&f.repo).unwrap();
    let conflict = state.changes.iter().find(|c| c.is_conflicted()).unwrap();
    let review = desktop_git::inspect_conflict(&f.repo, conflict).unwrap();
    desktop_git::resolve_conflict(&review).unwrap();
    desktop_git::commit(&f.repo, "Resolve feature merge", &f.basis()).unwrap();
    assert!(f.basis().merge_heads.is_none());
}

#[test]
fn identical_tree_divergence_can_complete_an_empty_merge_commit() {
    let f = Fixture::new();
    git(&f.repo, &["checkout", "feature"]);
    f.commit("empty feature commit");
    git(&f.repo, &["checkout", "main"]);
    f.commit("empty main commit");
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::CleanMerge);
    desktop_git::execute_merge(&preview).unwrap();
    let state = desktop_git::snapshot(&f.repo).unwrap();
    assert!(state.changes.is_empty());
    assert!(state.commit_basis.merge_heads.is_some());
    desktop_git::commit(&f.repo, "Merge identical trees", &state.commit_basis).unwrap();
    assert_eq!(
        String::from_utf8(git(&f.repo, &["rev-list", "--parents", "-1", "HEAD"]))
            .unwrap()
            .split_whitespace()
            .count(),
        3
    );
}

#[test]
fn dirty_worktree_and_stale_source_or_target_prevent_merge() {
    let f = Fixture::new();
    f.advance_feature("feature-file", b"feature\n");
    let preview = f.preview();
    f.write("untracked", b"keep me");
    assert!(desktop_git::execute_merge(&preview).is_err());
    assert_eq!(std::fs::read(f.repo.join("untracked")).unwrap(), b"keep me");
    std::fs::remove_file(f.repo.join("untracked")).unwrap();
    f.commit("new main HEAD");
    assert!(desktop_git::execute_merge(&preview).is_err());
    let preview = f.preview();
    git(&f.repo, &["branch", "--force", "feature", "HEAD"]);
    assert!(desktop_git::execute_merge(&preview).is_err());
}

#[test]
fn ignored_local_files_cannot_be_overwritten_by_fast_forward() {
    let f = Fixture::new();
    f.advance_feature("ignored.tmp", b"tracked remote content");
    std::fs::write(f.repo.join(".git/info/exclude"), b"ignored.tmp\n").unwrap();
    f.write("ignored.tmp", b"local ignored content");
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::FastForward);
    let before = f.basis();
    assert!(desktop_git::execute_merge(&preview).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(
        std::fs::read(f.repo.join("ignored.tmp")).unwrap(),
        b"local ignored content"
    );
}

#[test]
fn explicit_merge_flags_override_squash_no_ff_and_autostash_defaults() {
    let f = Fixture::new();
    f.advance_feature("feature-file", b"feature\n");
    git(&f.repo, &["config", "merge.ff", "false"]);
    git(&f.repo, &["config", "merge.autoStash", "true"]);
    git(
        &f.repo,
        &["config", "branch.main.mergeOptions", "--squash --no-ff"],
    );
    let preview = f.preview();
    desktop_git::execute_merge(&preview).unwrap();
    assert_eq!(f.basis().head.as_ref().unwrap().trim(), preview.target.oid);
    assert!(f.basis().merge_heads.is_none());
}

#[test]
fn abort_discards_merge_result_but_keeps_untracked_files_and_saved_stashes() {
    let f = Fixture::new();
    f.write("base", b"saved\n");
    desktop_git::save_stash(&f.repo, "saved", &f.basis()).unwrap();
    let stashes = desktop_git::snapshot(&f.repo).unwrap().stashes;
    f.diverge();
    let before = f.basis();
    desktop_git::execute_merge(&f.preview()).unwrap();
    f.write("untracked", b"keep\n");
    desktop_git::abort_merge(&f.repo, &f.basis()).unwrap();
    assert_eq!(f.basis(), before);
    assert!(!f.repo.join("feature-file").exists());
    assert_eq!(std::fs::read(f.repo.join("untracked")).unwrap(), b"keep\n");
    assert_eq!(desktop_git::snapshot(&f.repo).unwrap().stashes, stashes);
}

#[test]
fn existing_merge_blocks_new_merge_stash_and_branch_switching() {
    let f = Fixture::new();
    f.diverge();
    desktop_git::execute_merge(&f.preview()).unwrap();
    let basis = f.basis();
    assert!(desktop_git::preview_merge(&f.repo, &f.target(), &basis).is_err());
    assert!(desktop_git::save_stash(&f.repo, "during merge", &basis).is_err());
    assert!(desktop_git::switch_branch(&f.repo, &f.target(), &basis).is_err());
    let path = f.repo.join(".git/MERGE_HEAD");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, basis.head.as_ref().unwrap()).unwrap();
    assert!(desktop_git::commit(&f.repo, "unexpected parents", &basis).is_err());
    std::fs::write(path, original).unwrap();
    desktop_git::abort_merge(&f.repo, &f.basis()).unwrap();
}

#[test]
fn unrelated_histories_are_not_merged() {
    let f = Fixture::new();
    git(&f.repo, &["checkout", "--orphan", "unrelated"]);
    f.commit("unrelated root");
    git(&f.repo, &["checkout", "main"]);
    let state = desktop_git::snapshot(&f.repo).unwrap();
    let target = state
        .branches
        .iter()
        .find(|b| b.name == "unrelated")
        .unwrap();
    let error = desktop_git::preview_merge(&f.repo, target, &state.commit_basis).unwrap_err();
    assert!(error.to_string().contains("unrelated histories"));
    assert!(f.basis().merge_heads.is_none());
}

#[test]
fn linked_worktree_merge_metadata_is_isolated_from_main_worktree() {
    let f = Fixture::new();
    f.diverge();
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
    let state = desktop_git::snapshot(&linked).unwrap();
    let preview = desktop_git::preview_merge(&linked, &f.target(), &state.commit_basis).unwrap();
    desktop_git::execute_merge(&preview).unwrap();
    assert!(desktop_git::snapshot(&linked)
        .unwrap()
        .commit_basis
        .merge_heads
        .is_some());
    assert!(f.basis().merge_heads.is_none());
    let basis = desktop_git::snapshot(&linked).unwrap().commit_basis;
    desktop_git::abort_merge(&linked, &basis).unwrap();
    assert!(f.basis().merge_heads.is_none());
}

#[test]
fn configured_ours_strategy_cannot_replace_the_previewed_merge_result() {
    let f = Fixture::new();
    f.diverge();
    git(
        &f.repo,
        &[
            "config",
            "branch.main.mergeOptions",
            "--strategy=ours --no-ff",
        ],
    );
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::CleanMerge);
    desktop_git::execute_merge(&preview).unwrap();
    assert_eq!(
        std::fs::read(f.repo.join("feature-file")).unwrap(),
        b"feature\n"
    );
    assert!(f.basis().merge_heads.is_some());
}

#[test]
fn conflict_preview_preserves_newlines_and_literal_pathspec_names() {
    let f = Fixture::new();
    let name = ":(glob)*\nfile";
    std::fs::rename(f.repo.join("base"), f.repo.join(name)).unwrap();
    f.commit("rename file");
    git(&f.repo, &["branch", "--force", "feature", "HEAD"]);
    f.advance_feature(name, b"feature side\n");
    f.write(name, b"main side\n");
    f.commit("main change");
    let preview = f.preview();
    assert_eq!(preview.kind, MergeKind::Conflicts);
    assert_eq!(preview.conflicts, vec![PathBuf::from(name)]);
}

#[test]
fn stale_abort_basis_does_not_discard_newly_staged_work() {
    let f = Fixture::new();
    f.diverge();
    desktop_git::execute_merge(&f.preview()).unwrap();
    let old = f.basis();
    f.write("newly-staged", b"keep me\n");
    git(&f.repo, &["add", "newly-staged"]);
    assert!(desktop_git::abort_merge(&f.repo, &old).is_err());
    assert_eq!(
        std::fs::read(f.repo.join("newly-staged")).unwrap(),
        b"keep me\n"
    );
    assert!(f.basis().merge_heads.is_some());
}

#[test]
fn resolved_but_unfinished_cherry_pick_blocks_starting_a_merge() {
    let f = Fixture::new();
    f.advance_feature("base", b"feature side\n");
    f.write("base", b"main side\n");
    f.commit("main change");
    let output = Command::new("git")
        .arg("-C")
        .arg(&f.repo)
        .args(["cherry-pick", &f.target().oid])
        .output()
        .unwrap();
    assert!(!output.status.success());
    f.write("base", b"main side\n");
    git(&f.repo, &["add", "base"]);
    assert!(desktop_git::snapshot(&f.repo).unwrap().changes.is_empty());
    let error = desktop_git::preview_merge(&f.repo, &f.target(), &f.basis()).unwrap_err();
    assert!(error.to_string().contains("existing Git operation"));
}
