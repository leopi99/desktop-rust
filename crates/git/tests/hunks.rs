//! Behavioral coverage for complete hunk reviews and index-only partial staging.
#![cfg(test)]
use desktop_core::FileChange;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    name: PathBuf,
}
impl Fixture {
    fn new(name: impl AsRef<Path>, base: &[u8], working: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-hunks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let f = Self {
            root,
            name: name.as_ref().into(),
        };
        f.git(&["init", "--initial-branch=main"]);
        for (k, v) in [
            ("user.name", "Fixture"),
            ("user.email", "fixture@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
            ("core.autocrlf", "false"),
            ("core.attributesFile", "/dev/null"),
        ] {
            f.git(&["config", k, v]);
        }
        std::fs::write(f.file(), base).unwrap();
        f.git(&["add", "--all"]);
        f.git(&["commit", "-m", "baseline"]);
        std::fs::write(f.file(), working).unwrap();
        f
    }
    fn file(&self) -> PathBuf {
        self.root.join(&self.name)
    }
    fn git<S: AsRef<OsStr>>(&self, args: &[S]) -> Vec<u8> {
        let result = Command::new("git")
            .arg("--literal-pathspecs")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(&self.root)
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
    fn change(&self) -> FileChange {
        desktop_git::snapshot(&self.root)
            .unwrap()
            .changes
            .into_iter()
            .find(|c| c.path == self.name)
            .unwrap()
    }
    fn basis(&self) -> desktop_core::CommitBasis {
        desktop_git::snapshot(&self.root).unwrap().commit_basis
    }
    fn index(&self) -> Vec<u8> {
        self.git(&[
            OsStr::new("show"),
            {
                let mut value = std::ffi::OsString::from(":");
                value.push(&self.name);
                value
            }
            .as_os_str(),
        ])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn contents(edits: &[usize]) -> Vec<u8> {
    (0..50)
        .map(|n| {
            format!(
                "line {n:03}{}\n",
                if edits.contains(&n) { " edited" } else { "" }
            )
        })
        .collect::<String>()
        .into_bytes()
}

#[test]
fn either_hunk_stages_only_its_block_and_preserves_the_working_file() {
    for (index, line) in [(0, 10), (1, 35)] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
        let before = f.basis();
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        assert_eq!(review.hunks.len(), 2);
        assert_eq!(f.basis(), before);
        desktop_git::stage_hunk(&review, index).unwrap();
        assert_eq!(f.index(), contents(&[line]));
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 35]));
        assert_eq!(f.basis().head, before.head);
        let staged = f.basis();
        assert!(desktop_git::stage_hunk(&review, index).is_err());
        assert_eq!(f.basis(), staged);
        desktop_git::commit(&f.root, "Commit only the chosen hunk", &staged).unwrap();
        assert_eq!(f.git(&["show", "HEAD:file"]), contents(&[line]));
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 35]));
        let fresh = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        assert_eq!(fresh.hunks.len(), 1);
        desktop_git::stage_hunk(&fresh, 0).unwrap();
        assert_eq!(f.index(), contents(&[10, 35]));
    }
}

#[test]
fn partial_staging_preserves_prior_index_changes_and_other_files() {
    let f = Fixture::new("file", &contents(&[]), &contents(&[1]));
    f.git(&["add", "--", "file"]);
    std::fs::write(f.file(), contents(&[1, 15, 35])).unwrap();
    std::fs::write(f.root.join("other"), b"staged other").unwrap();
    f.git(&["add", "--", "other"]);
    let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(review.hunks.len(), 2);
    desktop_git::stage_hunk(&review, 1).unwrap();
    assert_eq!(f.index(), contents(&[1, 35]));
    assert_eq!(f.git(&["show", ":other"]), b"staged other");
    assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[1, 15, 35]));
}

#[test]
fn stale_contents_index_head_or_policy_never_stage_a_reviewed_hunk() {
    for mutation in ["working", "index", "head", "attributes", "settings"] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        match mutation {
            "working" => std::fs::write(f.file(), contents(&[11, 35])).unwrap(),
            "index" => {
                std::fs::write(f.root.join("other"), b"new").unwrap();
                f.git(&["add", "--", "other"]);
            }
            "head" => {
                f.git(&["commit", "--allow-empty", "-m", "advance"]);
            }
            "attributes" => {
                std::fs::write(f.root.join(".git/info/attributes"), b"file text\n").unwrap()
            }
            _ => {
                f.git(&["config", "core.safecrlf", "true"]);
            }
        }
        let before = f.basis();
        let bytes = std::fs::read(f.file()).unwrap();
        assert!(desktop_git::stage_hunk(&review, 0).is_err(), "{mutation}");
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
    }
}

#[test]
fn binary_non_utf8_converted_and_new_files_keep_full_file_staging_only() {
    for kind in ["binary", "encoding", "filter", "new"] {
        let f = Fixture::new("file", b"base\n", b"edited\n");
        match kind {
            "binary" => std::fs::write(f.file(), b"binary\0data").unwrap(),
            "encoding" => std::fs::write(f.file(), b"invalid-\xff\n").unwrap(),
            "filter" => std::fs::write(
                f.root.join(".git/info/attributes"),
                b"file filter=special\n",
            )
            .unwrap(),
            _ => {
                f.git(&["rm", "--cached", "--", "file"]);
            }
        }
        let before = f.basis();
        let bytes = std::fs::read(f.file()).unwrap();
        assert!(
            desktop_git::review_hunks(&f.root, &f.change()).is_err(),
            "{kind}"
        );
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
    }
}

#[test]
fn oversized_complete_patches_and_excess_hunks_are_refused_without_partial_staging() {
    let f = Fixture::new("file", b"base\n", &vec![b'x'; 600_000]);
    let before = f.basis();
    assert!(desktop_git::review_hunks(&f.root, &f.change()).is_err());
    assert_eq!(f.basis(), before);
    desktop_git::stage(&f.root, &f.change()).unwrap();
    assert_eq!(f.index(), vec![b'x'; 600_000]);
    for count in [128, 129] {
        let base = (0..count * 12)
            .map(|n| format!("line {n}\n"))
            .collect::<String>();
        let work = (0..count * 12)
            .map(|n| format!("line {n}{}\n", if n % 12 == 5 { " changed" } else { "" }))
            .collect::<String>();
        let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
        let before = f.basis();
        let review = desktop_git::review_hunks(&f.root, &f.change());
        if count == 128 {
            assert_eq!(review.unwrap().hunks.len(), 128);
        } else {
            assert!(review.is_err());
        }
        assert_eq!(f.basis(), before);
    }
}

#[test]
fn newline_markers_crlf_and_patch_like_content_round_trip_without_byte_changes() {
    for (base, work) in [
        (b"old".as_slice(), b"new".as_slice()),
        (b"", b"created inside tracked empty file\n"),
        (b"existing tracked file\n", b""),
        (b"a\r\nb\r\n", b"a\r\nc\r\n"),
        (b"a\nb\n", b"a\n"),
        (b"base\n", b"@@ -1 +1 @@\n--- fake\n+++ fake\n"),
    ] {
        let f = Fixture::new("file", base, work);
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        desktop_git::stage_hunk(&review, 0).unwrap();
        assert_eq!(f.index(), work);
        assert_eq!(std::fs::read(f.file()).unwrap(), work);
    }
}

#[test]
fn forged_or_invalid_hunk_choices_and_index_lock_failure_preserve_user_data() {
    let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
    let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    let before = f.basis();
    assert!(desktop_git::stage_hunk(&review, 99).is_err());
    let mut forged = review.clone();
    forged.hunks[0] = String::from("@@ -1 +1 @@\n-line 000\n+forged\n");
    assert!(desktop_git::stage_hunk(&forged, 0).is_err());
    assert_eq!(f.basis(), before);
    std::fs::write(f.root.join(".git/index.lock"), b"fixture lock").unwrap();
    assert!(desktop_git::stage_hunk(&review, 0).is_err());
    assert_eq!(f.basis(), before);
    assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 35]));
}

#[cfg(unix)]
#[test]
fn literal_and_native_paths_never_stage_a_different_file() {
    use std::os::unix::ffi::OsStrExt;
    for name in [
        PathBuf::from(":(glob)*\n\tfile"),
        PathBuf::from(OsStr::from_bytes(b"file-\xff")),
    ] {
        let f = Fixture::new(name, &contents(&[]), &contents(&[10, 35]));
        std::fs::write(f.root.join("untouched"), b"keep").unwrap();
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        desktop_git::stage_hunk(&review, 0).unwrap();
        assert_eq!(f.index(), contents(&[10]));
        assert_eq!(std::fs::read(f.root.join("untouched")).unwrap(), b"keep");
    }
}

#[cfg(unix)]
#[test]
fn symlinks_mode_changes_and_renamed_paths_do_not_produce_hunk_write_reviews() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for kind in ["symlink", "mode", "rename"] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
        match kind {
            "symlink" => {
                std::fs::remove_file(f.file()).unwrap();
                std::fs::write(f.root.join("target"), b"private").unwrap();
                symlink("target", f.file()).unwrap();
            }
            "mode" => {
                std::fs::set_permissions(f.file(), std::fs::Permissions::from_mode(0o755)).unwrap()
            }
            _ => {
                f.git(&["mv", "--", "file", "renamed"]);
            }
        }
        let state = desktop_git::snapshot(&f.root).unwrap();
        let before = state.commit_basis;
        let change = state
            .changes
            .iter()
            .find(|c| c.path == f.name || c.path == Path::new("renamed"))
            .unwrap();
        assert!(
            desktop_git::review_hunks(&f.root, change).is_err(),
            "{kind}"
        );
        assert_eq!(f.basis(), before);
        if kind == "symlink" {
            assert_eq!(std::fs::read(f.root.join("target")).unwrap(), b"private");
        }
    }
}

#[test]
fn choosing_a_later_hunk_uses_index_coordinates_despite_earlier_insertions() {
    let base = contents(&[]);
    let work = String::from_utf8(contents(&[35]))
        .unwrap()
        .replace("line 010\n", "inserted one\ninserted two\nline 010\n");
    let f = Fixture::new("file", &base, work.as_bytes());
    let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(review.hunks.len(), 2);
    desktop_git::stage_hunk(&review, 1).unwrap();
    assert_eq!(f.index(), contents(&[35]));
    assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
    let fresh = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(fresh.hunks.len(), 1);
    desktop_git::stage_hunk(&fresh, 0).unwrap();
    assert_eq!(f.index(), work.as_bytes());
}

#[test]
fn either_staged_hunk_can_be_removed_then_the_remaining_block_committed() {
    for (index, kept) in [(0, 35), (1, 10)] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
        f.git(&["add", "--all"]);
        let before = f.basis();
        let review = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
        assert_eq!(review.side, desktop_core::HunkSide::Staged);
        assert_eq!(review.hunks.len(), 2);
        assert_eq!(f.basis(), before);
        desktop_git::unstage_hunk(&review, index).unwrap();
        assert_eq!(f.index(), contents(&[kept]));
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 35]));
        let staged = f.basis();
        assert!(desktop_git::unstage_hunk(&review, index).is_err());
        assert_eq!(f.basis(), staged);
        desktop_git::commit(&f.root, "Commit remaining staged hunk", &staged).unwrap();
        assert_eq!(f.git(&["show", "HEAD:file"]), contents(&[kept]));
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 35]));
    }
}

#[test]
fn unstaging_preserves_divergent_working_edits_and_other_staged_files() {
    let f = Fixture::new("file", &contents(&[]), &contents(&[1, 15, 35]));
    f.git(&["add", "--all"]);
    std::fs::write(f.file(), contents(&[1, 15, 25, 35])).unwrap();
    std::fs::write(f.root.join("other"), b"keep staged").unwrap();
    f.git(&["add", "--all"]);
    // Restore an additional working-only edit after staging the other path.
    std::fs::write(f.file(), contents(&[1, 15, 20, 25, 35])).unwrap();
    let review = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
    desktop_git::unstage_hunk(&review, 0).unwrap();
    assert_eq!(f.index(), contents(&[15, 25, 35]));
    assert_eq!(f.git(&["show", ":other"]), b"keep staged");
    assert_eq!(
        std::fs::read(f.file()).unwrap(),
        contents(&[1, 15, 20, 25, 35])
    );
}

#[test]
fn reverse_hunk_coordinates_remain_correct_after_an_earlier_insertion_is_removed() {
    let work = String::from_utf8(contents(&[35]))
        .unwrap()
        .replace("line 010\n", "inserted one\ninserted two\nline 010\n");
    let f = Fixture::new("file", &contents(&[]), work.as_bytes());
    f.git(&["add", "--all"]);
    let review = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(review.hunks.len(), 2);
    desktop_git::unstage_hunk(&review, 0).unwrap();
    assert_eq!(f.index(), contents(&[35]));
    let fresh = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(fresh.hunks.len(), 1);
    desktop_git::unstage_hunk(&fresh, 0).unwrap();
    assert_eq!(f.index(), contents(&[]));
    assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
}

#[test]
fn staged_and_unstaged_reviews_cannot_authorize_the_opposite_index_action() {
    let f = Fixture::new("file", &contents(&[]), &contents(&[10]));
    f.git(&["add", "--all"]);
    std::fs::write(f.file(), contents(&[10, 35])).unwrap();
    let staged = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
    let unstaged = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    let before = f.basis();
    assert!(desktop_git::stage_hunk(&staged, 0).is_err());
    assert!(desktop_git::unstage_hunk(&unstaged, 0).is_err());
    let mut forged = unstaged;
    forged.side = desktop_core::HunkSide::Staged;
    assert!(desktop_git::unstage_hunk(&forged, 0).is_err());
    assert!(desktop_git::unstage_hunk(&staged, 999).is_err());
    assert_eq!(f.basis(), before);
}

#[test]
fn staged_reviews_reject_changed_inputs_and_index_lock_failures_without_losing_data() {
    for mutation in ["working", "index", "head", "attributes", "settings", "lock"] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 35]));
        f.git(&["add", "--all"]);
        let review = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
        match mutation {
            "working" => std::fs::write(f.file(), contents(&[11, 35])).unwrap(),
            "index" => {
                std::fs::write(f.root.join("other"), b"new").unwrap();
                f.git(&["add", "--all"]);
            }
            "head" => {
                f.git(&["commit", "-m", "external commit"]);
            }
            "attributes" => {
                std::fs::write(f.root.join(".git/info/attributes"), b"file text\n").unwrap()
            }
            "settings" => {
                f.git(&["config", "core.safecrlf", "true"]);
            }
            _ => std::fs::write(f.root.join(".git/index.lock"), b"fixture lock").unwrap(),
        }
        let before = f.basis();
        let bytes = std::fs::read(f.file()).unwrap();
        assert!(desktop_git::unstage_hunk(&review, 0).is_err(), "{mutation}");
        assert_eq!(f.basis(), before);
        assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
    }
}

#[test]
fn staged_patch_limits_and_unsupported_types_never_apply_a_prefix() {
    for kind in ["binary", "encoding", "new", "deleted", "filter", "large"] {
        let f = Fixture::new("file", b"base\n", b"edited\n");
        match kind {
            "binary" => std::fs::write(f.file(), b"binary\0bytes").unwrap(),
            "encoding" => std::fs::write(f.file(), b"invalid-\xff\n").unwrap(),
            "new" => {
                f.git(&["rm", "--cached", "--", "file"]);
                f.git(&["commit", "-m", "remove from HEAD"]);
            }
            "deleted" => std::fs::remove_file(f.file()).unwrap(),
            "large" => std::fs::write(f.file(), vec![b'x'; 600_000]).unwrap(),
            _ => {}
        }
        f.git(&["add", "--all"]);
        if kind == "filter" {
            std::fs::write(
                f.root.join(".git/info/attributes"),
                b"file filter=example\n",
            )
            .unwrap();
        }
        let before = f.basis();
        assert!(
            desktop_git::review_staged_hunks(&f.root, &f.change()).is_err(),
            "{kind}"
        );
        assert_eq!(f.basis(), before);
    }
    for count in [128, 129] {
        let base = (0..count * 12)
            .map(|n| format!("line {n}\n"))
            .collect::<String>();
        let work = (0..count * 12)
            .map(|n| format!("line {n}{}\n", if n % 12 == 5 { " changed" } else { "" }))
            .collect::<String>();
        let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
        f.git(&["add", "--all"]);
        let before = f.basis();
        let review = desktop_git::review_staged_hunks(&f.root, &f.change());
        if count == 128 {
            assert_eq!(review.unwrap().hunks.len(), 128);
        } else {
            assert!(review.is_err());
        }
        assert_eq!(f.basis(), before);
    }
}

#[test]
fn staged_crlf_eof_and_quoted_paths_reverse_without_writing_the_working_file() {
    let mut names = vec![PathBuf::from("file"), PathBuf::from(":(glob)*\n\tfile")];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        names.push(PathBuf::from(OsStr::from_bytes(b"file-\xff")));
    }
    for name in names {
        for (base, work) in [
            (b"old".as_slice(), b"new".as_slice()),
            (b"a\r\nb\r\n", b"a\r\nc\r\n"),
            (b"", b"added\n"),
            (b"removed\n", b""),
        ] {
            let f = Fixture::new(&name, base, work);
            f.git(&["add", "--all"]);
            let review = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
            desktop_git::unstage_hunk(&review, 0).unwrap();
            assert_eq!(f.index(), base);
            assert_eq!(std::fs::read(f.file()).unwrap(), work);
        }
    }
}
