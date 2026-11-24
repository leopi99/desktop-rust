//! Git integration coverage for history.
#![cfg(test)]

use desktop_core::{CommitBasis, CommitFile};
use std::{
    collections::HashSet,
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
            "desktop-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let repo = desktop_git::init_repository(&root.join("project"), "main").unwrap();
        for (key, value) in [
            ("user.name", "Autore È Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git(&repo, &["config", key, value]);
        }
        let f = Self { root, repo };
        f.write("file", b"base\n");
        f.commit("Initial commit\n\nBody paragraph");
        f
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.repo.join(name), bytes).unwrap();
    }
    fn commit(&self, message: &str) {
        git(&self.repo, &["add", "--all"]);
        git(&self.repo, &["commit", "-m", message]);
    }
    fn oid(&self) -> String {
        String::from_utf8(git(&self.repo, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned()
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
fn root_commit_metadata_message_and_diff_are_read_without_mutation() {
    let f = Fixture::new();
    let before = f.basis();
    let history = desktop_git::commit_history(&f.repo, &f.oid(), 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].subject, "Initial commit");
    assert_eq!(history[0].author, "Autore È Test");
    assert!(history[0].authored_at.contains('T'));
    let details = desktop_git::commit_details(&f.repo, &history[0].oid).unwrap();
    assert!(details.parents.is_empty());
    assert!(details.message.contains("Body paragraph"));
    assert_eq!(details.files[0].status, 'A');
    assert_eq!(details.files[0].path, Path::new("file"));
    let patch = desktop_git::commit_file_diff(&f.repo, &history[0].oid, &details.files[0]).unwrap();
    assert!(patch.contains("+base"));
    assert_eq!(f.basis(), before);
}

#[test]
fn rename_and_newline_paths_keep_both_names() {
    let f = Fixture::new();
    std::fs::rename(f.repo.join("file"), f.repo.join("new\nname")).unwrap();
    f.commit("rename");
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    assert_eq!(details.files.len(), 1);
    assert_eq!(details.files[0].status, 'R');
    assert_eq!(details.files[0].path, Path::new("new\nname"));
    assert_eq!(
        details.files[0].original_path.as_deref(),
        Some(Path::new("file"))
    );
    assert!(
        desktop_git::commit_file_diff(&f.repo, &f.oid(), &details.files[0])
            .unwrap()
            .contains("rename from")
    );
}

#[test]
fn literal_committed_diff_does_not_show_other_files_or_current_working_changes() {
    let f = Fixture::new();
    f.write(":(glob)*", b"old\n");
    f.write("other", b"old\n");
    f.commit("two files");
    f.write(":(glob)*", b"selected committed\n");
    f.write("other", b"other committed\n");
    f.commit("modify");
    f.write(":(glob)*", b"current working edit\n");
    let before = f.basis();
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    let file = details
        .files
        .iter()
        .find(|file| file.path == Path::new(":(glob)*"))
        .unwrap();
    let patch = desktop_git::commit_file_diff(&f.repo, &f.oid(), file).unwrap();
    assert!(patch.contains("+selected committed"));
    assert!(!patch.contains("other committed"));
    assert!(!patch.contains("current working edit"));
    assert_eq!(
        std::fs::read(f.repo.join(":(glob)*")).unwrap(),
        b"current working edit\n"
    );
    assert_eq!(f.basis(), before);
}

#[test]
fn deletion_and_binary_patches_are_available_with_helpers_disabled() {
    let f = Fixture::new();
    f.write("binary", b"a\0b");
    f.commit("binary");
    std::fs::remove_file(f.repo.join("file")).unwrap();
    f.write("binary", b"c\0d");
    f.commit("delete and binary");
    git(
        &f.repo,
        &["config", "diff.external", "nonexistent-diff-helper"],
    );
    git(
        &f.repo,
        &[
            "config",
            "diff.custom.textconv",
            "nonexistent-textconv-helper",
        ],
    );
    std::fs::write(f.repo.join(".git/info/attributes"), b"binary diff=custom\n").unwrap();
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    let deleted = details
        .files
        .iter()
        .find(|file| file.status == 'D')
        .unwrap();
    assert!(desktop_git::commit_file_diff(&f.repo, &f.oid(), deleted)
        .unwrap()
        .contains("-base"));
    let binary = details
        .files
        .iter()
        .find(|file| file.path == Path::new("binary"))
        .unwrap();
    assert!(desktop_git::commit_file_diff(&f.repo, &f.oid(), binary)
        .unwrap()
        .contains("Binary files"));
}

#[test]
fn paging_uses_the_original_tip_even_after_head_advances() {
    let f = Fixture::new();
    let tree = String::from_utf8(git(&f.repo, &["rev-parse", "HEAD^{tree}"])).unwrap();
    let tree = tree.trim();
    let mut tip = f.oid();
    for index in 0..104 {
        tip = String::from_utf8(git(
            &f.repo,
            &[
                "commit-tree",
                tree,
                "-p",
                &tip,
                "-m",
                &format!("entry {index}"),
            ],
        ))
        .unwrap()
        .trim()
        .to_owned();
    }
    git(&f.repo, &["update-ref", "HEAD", &tip]);
    let first = desktop_git::commit_history(&f.repo, &tip, 0).unwrap();
    assert_eq!(first.len(), 100);
    git(&f.repo, &["commit", "--allow-empty", "-m", "new HEAD"]);
    let before = f.basis();
    let second = desktop_git::commit_history(&f.repo, &tip, 100).unwrap();
    assert_eq!(second.len(), 5);
    let ids: HashSet<_> = first.iter().chain(&second).map(|c| &c.oid).collect();
    assert_eq!(ids.len(), 105);
    assert_eq!(first[0].oid, tip);
    assert_eq!(f.basis(), before);
}

#[test]
fn merge_file_list_compares_with_first_parent() {
    let f = Fixture::new();
    git(&f.repo, &["checkout", "-b", "feature"]);
    f.write("feature", b"feature\n");
    f.commit("feature");
    git(&f.repo, &["checkout", "main"]);
    f.write("main", b"main\n");
    f.commit("main");
    git(&f.repo, &["merge", "--no-ff", "--no-edit", "feature"]);
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    assert_eq!(details.parents.len(), 2);
    assert_eq!(details.files.len(), 1);
    assert_eq!(details.files[0].path, Path::new("feature"));
    assert!(
        desktop_git::commit_file_diff(&f.repo, &f.oid(), &details.files[0])
            .unwrap()
            .contains("+feature")
    );
}

#[test]
fn notes_color_and_format_configuration_do_not_corrupt_history_records() {
    let f = Fixture::new();
    git(
        &f.repo,
        &["notes", "add", "-m", "Extra notes should not change fields"],
    );
    git(&f.repo, &["config", "color.ui", "always"]);
    git(&f.repo, &["config", "format.pretty", "oneline"]);
    let history = desktop_git::commit_history(&f.repo, &f.oid(), 0).unwrap();
    assert_eq!(history[0].subject, "Initial commit");
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    assert!(!details.message.contains("Extra notes"));
}

#[test]
fn invalid_ids_and_paths_not_in_commit_are_rejected() {
    let f = Fixture::new();
    for oid in ["HEAD", "--all", "1234567"] {
        assert!(desktop_git::commit_details(&f.repo, oid).is_err());
        assert!(desktop_git::commit_history(&f.repo, oid, 0).is_err());
    }
    let fake = CommitFile {
        path: "../outside".into(),
        original_path: None,
        status: 'A',
    };
    assert!(desktop_git::commit_file_diff(&f.repo, &f.oid(), &fake).is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_committed_paths_are_not_lossily_reconstructed() {
    use std::os::unix::ffi::OsStrExt;
    let f = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"native-\xff");
    std::fs::write(f.repo.join(name), b"native contents\n").unwrap();
    f.commit("native file");
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    let file = details
        .files
        .iter()
        .find(|file| file.path.as_os_str() == name)
        .unwrap();
    assert!(desktop_git::commit_file_diff(&f.repo, &f.oid(), file)
        .unwrap()
        .contains("+native contents"));
}

#[test]
fn large_messages_and_committed_diffs_have_bounded_previews() {
    let f = Fixture::new();
    f.write("large", &vec![b'x'; 300 * 1024]);
    f.commit(&format!("Large commit\n\n{}", "body ".repeat(15000)));
    let details = desktop_git::commit_details(&f.repo, &f.oid()).unwrap();
    assert!(details.message.contains("preview truncated"));
    assert!(details.message.len() < 65 * 1024);
    let file = details
        .files
        .iter()
        .find(|file| file.path == Path::new("large"))
        .unwrap();
    let patch = desktop_git::commit_file_diff(&f.repo, &f.oid(), file).unwrap();
    assert!(patch.contains("preview truncated"));
    assert!(patch.len() < 257 * 1024);
}
