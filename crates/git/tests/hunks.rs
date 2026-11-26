//! Behavioral coverage for complete hunk reviews and index-only partial staging.
#![cfg(test)]
use desktop_core::{FileChange, HunkLine};
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

#[test]
fn multiple_nonadjacent_hunks_preserve_unselected_blocks_other_paths_and_working_bytes() {
    for side in [
        desktop_core::HunkSide::Unstaged,
        desktop_core::HunkSide::Staged,
    ] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[5, 20, 40]));
        if side == desktop_core::HunkSide::Staged {
            f.git(&["add", "--all"]);
        }
        std::fs::write(f.root.join("other"), b"keep staged").unwrap();
        f.git(&["add", "--", "other"]);
        let review = if side == desktop_core::HunkSide::Staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        assert_eq!(review.hunks.len(), 3);
        if side == desktop_core::HunkSide::Staged {
            desktop_git::unstage_hunks(&review, &[2, 0]).unwrap();
        } else {
            desktop_git::stage_hunks(&review, &[2, 0]).unwrap();
        }
        let expected = if side == desktop_core::HunkSide::Staged {
            contents(&[20])
        } else {
            contents(&[5, 40])
        };
        assert_eq!(f.index(), expected);
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[5, 20, 40]));
        assert_eq!(f.git(&["show", ":other"]), b"keep staged");
        desktop_git::commit(&f.root, "Commit selected index blocks", &f.basis()).unwrap();
        assert_eq!(f.git(&["show", "HEAD:file"]), expected);
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[5, 20, 40]));
    }
}

#[test]
fn multi_hunk_offsets_handle_selected_insertions_and_unselected_deletions() {
    let base = String::from_utf8(contents(&[])).unwrap();
    let working = base
        .replace("line 005\n", "inserted one\ninserted two\nline 005\n")
        .replace("line 020\nline 021\n", "")
        .replace("line 040\n", "line 040 edited\n");
    for staged in [false, true] {
        let f = Fixture::new("file", base.as_bytes(), working.as_bytes());
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        assert_eq!(review.hunks.len(), 3);
        let expected = if staged {
            base.replace("line 020\nline 021\n", "")
        } else {
            base.replace("line 005\n", "inserted one\ninserted two\nline 005\n")
                .replace("line 040\n", "line 040 edited\n")
        };
        if staged {
            desktop_git::unstage_hunks(&review, &[0, 2]).unwrap();
        } else {
            desktop_git::stage_hunks(&review, &[0, 2]).unwrap();
        }
        assert_eq!(f.index(), expected.as_bytes());
        assert_eq!(std::fs::read(f.file()).unwrap(), working.as_bytes());
    }
}

#[test]
fn invalid_multiple_selections_and_wrong_directions_never_update_a_prefix() {
    for staged in [false, true] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[5, 20, 40]));
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        let before = f.basis();
        for indices in [vec![], vec![0, 0], vec![0, 2, 99], vec![0, 1, 2, 2]] {
            let result = if staged {
                desktop_git::unstage_hunks(&review, &indices)
            } else {
                desktop_git::stage_hunks(&review, &indices)
            };
            assert!(result.is_err());
            assert_eq!(f.basis(), before);
            assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[5, 20, 40]));
        }
        let result = if staged {
            desktop_git::stage_hunks(&review, &[0, 2])
        } else {
            desktop_git::unstage_hunks(&review, &[0, 2])
        };
        assert!(result.is_err());
        assert_eq!(f.basis(), before);
    }
}

#[test]
fn stale_or_locked_multi_hunk_operations_preserve_the_entire_index_and_working_file() {
    for staged in [false, true] {
        for mutation in ["working", "index", "lock"] {
            let f = Fixture::new("file", &contents(&[]), &contents(&[5, 20, 40]));
            if staged {
                f.git(&["add", "--all"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            match mutation {
                "working" => std::fs::write(f.file(), contents(&[5, 21, 40])).unwrap(),
                "index" => {
                    std::fs::write(f.root.join("other"), b"external staged").unwrap();
                    f.git(&["add", "--", "other"]);
                }
                _ => std::fs::write(f.root.join(".git/index.lock"), b"fixture lock").unwrap(),
            }
            let before = f.basis();
            let bytes = std::fs::read(f.file()).unwrap();
            let result = if staged {
                desktop_git::unstage_hunks(&review, &[0, 2])
            } else {
                desktop_git::stage_hunks(&review, &[0, 2])
            };
            assert!(result.is_err(), "{mutation}");
            assert_eq!(f.basis(), before);
            assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
        }
    }
}

#[test]
fn all_128_reviewed_hunks_can_be_staged_and_unstaged_in_one_selection() {
    let base = (0..128 * 12)
        .map(|n| format!("line {n}\n"))
        .collect::<String>();
    let work = (0..128 * 12)
        .map(|n| format!("line {n}{}\n", if n % 12 == 5 { " changed" } else { "" }))
        .collect::<String>();
    let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
    let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    assert_eq!(review.hunks.len(), 128);
    let indices = (0..128).rev().collect::<Vec<_>>();
    desktop_git::stage_hunks(&review, &indices).unwrap();
    assert_eq!(f.index(), work.as_bytes());
    let staged = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
    desktop_git::unstage_hunks(&staged, &indices).unwrap();
    assert_eq!(f.index(), base.as_bytes());
    assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
}

#[cfg(unix)]
#[test]
fn multi_hunk_native_paths_and_crlf_bytes_are_preserved_in_both_directions() {
    use std::os::unix::ffi::OsStrExt;
    let base = String::from_utf8(contents(&[]))
        .unwrap()
        .replace('\n', "\r\n");
    let work = String::from_utf8(contents(&[5, 20, 40]))
        .unwrap()
        .replace('\n', "\r\n");
    for name in [
        PathBuf::from(":(glob)*\n\tfile"),
        PathBuf::from(OsStr::from_bytes(b"multi-\xff")),
    ] {
        let f = Fixture::new(name, base.as_bytes(), work.as_bytes());
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        desktop_git::stage_hunks(&review, &[0, 2]).unwrap();
        let expected = String::from_utf8(contents(&[5, 40]))
            .unwrap()
            .replace('\n', "\r\n");
        assert_eq!(f.index(), expected.as_bytes());
        let staged = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
        desktop_git::unstage_hunks(&staged, &[1, 0]).unwrap();
        assert_eq!(f.index(), base.as_bytes());
        assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
    }
}

fn row(review: &desktop_core::HunkReview, text: &str) -> HunkLine {
    review
        .hunks
        .iter()
        .enumerate()
        .find_map(|(hunk, body)| {
            body.split_inclusive('\n')
                .position(|line| line == text)
                .map(|line| HunkLine { hunk, line })
        })
        .unwrap()
}

#[test]
fn individual_replacement_additions_and_deletions_have_explicit_index_semantics() {
    for staged in [false, true] {
        for choice in ["addition", "deletion", "both"] {
            let f = Fixture::new("file", b"before\nold\nend\n", b"before\nnew\nend\n");
            if staged {
                f.git(&["add", "--all"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            let add = row(&review, "+new\n");
            let delete = row(&review, "-old\n");
            let selection = match choice {
                "addition" => vec![add],
                "deletion" => vec![delete],
                _ => vec![delete, add],
            };
            if staged {
                desktop_git::unstage_lines(&review, &selection).unwrap();
            } else {
                desktop_git::stage_lines(&review, &selection).unwrap();
            }
            let expected = match (staged, choice) {
                (false, "addition") | (true, "deletion") => b"before\nold\nnew\nend\n".as_slice(),
                (false, "deletion") | (true, "addition") => b"before\nend\n",
                (false, _) => b"before\nnew\nend\n",
                _ => b"before\nold\nend\n",
            };
            assert_eq!(f.index(), expected);
            assert_eq!(std::fs::read(f.file()).unwrap(), b"before\nnew\nend\n");
        }
    }
}

#[test]
fn selected_line_pairs_within_one_hunk_preserve_other_edits_and_commit_only_the_index() {
    for staged in [false, true] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 12]));
        if staged {
            f.git(&["add", "--all"]);
        }
        std::fs::write(f.root.join("other"), b"keep staged").unwrap();
        f.git(&["add", "--", "other"]);
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        assert_eq!(review.hunks.len(), 1);
        let selection = [
            row(&review, "-line 010\n"),
            row(&review, "+line 010 edited\n"),
        ];
        if staged {
            desktop_git::unstage_lines(&review, &selection).unwrap();
        } else {
            desktop_git::stage_lines(&review, &selection).unwrap();
        }
        let expected = contents(if staged { &[12] } else { &[10] });
        assert_eq!(f.index(), expected);
        assert_eq!(f.git(&["show", ":other"]), b"keep staged");
        desktop_git::commit(&f.root, "Selected line pair", &f.basis()).unwrap();
        assert_eq!(f.git(&["show", "HEAD:file"]), expected);
        assert_eq!(std::fs::read(f.file()).unwrap(), contents(&[10, 12]));
    }
}

#[test]
fn partial_line_counts_and_offsets_remain_correct_across_separate_hunks() {
    let base = String::from_utf8(contents(&[])).unwrap();
    let work = base
        .replace("line 005\n", "line 005\ninsert A\ninsert B\n")
        .replace("line 020\n", "line 020 edited\n")
        .replace("line 035\n", "line 035\nlater A\nlater B\n");
    for staged in [false, true] {
        let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        let selection = [row(&review, "+later A\n"), row(&review, "+insert A\n")];
        if staged {
            desktop_git::unstage_lines(&review, &selection).unwrap();
        } else {
            desktop_git::stage_lines(&review, &selection).unwrap();
        }
        let expected = if staged {
            work.replace("insert A\n", "").replace("later A\n", "")
        } else {
            base.replace("line 005\n", "line 005\ninsert A\n")
                .replace("line 035\n", "line 035\nlater A\n")
        };
        assert_eq!(f.index(), expected.as_bytes());
        assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
    }
}

#[test]
fn empty_duplicate_context_and_opposite_direction_line_selections_are_refused() {
    for staged in [false, true] {
        let f = Fixture::new("file", &contents(&[]), &contents(&[10, 12]));
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        let add = row(&review, "+line 010 edited\n");
        let before = f.basis();
        for selection in [
            vec![],
            vec![add, add],
            vec![HunkLine { hunk: 0, line: 0 }],
            vec![HunkLine { hunk: 0, line: 1 }],
            vec![HunkLine { hunk: 99, line: 1 }],
            vec![HunkLine { hunk: 0, line: 999 }],
        ] {
            let result = if staged {
                desktop_git::unstage_lines(&review, &selection)
            } else {
                desktop_git::stage_lines(&review, &selection)
            };
            assert!(result.is_err());
            assert_eq!(f.basis(), before);
        }
        let result = if staged {
            desktop_git::stage_lines(&review, &[add])
        } else {
            desktop_git::unstage_lines(&review, &[add])
        };
        assert!(result.is_err());
        assert_eq!(f.basis(), before);
    }
}

#[test]
fn no_newline_hunks_refuse_implicit_line_joining_and_preserve_exact_bytes() {
    for staged in [false, true] {
        let f = Fixture::new("file", b"old", b"new");
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        let before = f.basis();
        let add = row(&review, "+new\n");
        let delete = row(&review, "-old\n");
        let result = if staged {
            desktop_git::unstage_lines(&review, &[delete])
        } else {
            desktop_git::stage_lines(&review, &[add])
        };
        assert!(result.is_err());
        assert_eq!(f.basis(), before);
        if staged {
            desktop_git::unstage_lines(&review, &[add, delete]).unwrap();
        } else {
            desktop_git::stage_lines(&review, &[add, delete]).unwrap();
        }
        assert_eq!(f.index(), if staged { b"old" } else { b"new" });
        assert_eq!(std::fs::read(f.file()).unwrap(), b"new");
    }
    for (base, work) in [(b"".as_slice(), b"new\n".as_slice()), (b"old\n", b"")] {
        let f = Fixture::new("file", base, work);
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        let selection = if base.is_empty() {
            row(&review, "+new\n")
        } else {
            row(&review, "-old\n")
        };
        desktop_git::stage_lines(&review, &[selection]).unwrap();
        assert_eq!(f.index(), work);
    }
}

#[test]
fn line_control_bounds_refuse_large_reviews_while_whole_hunk_staging_remains_available() {
    for count in [512, 513] {
        let base = (0..count).map(|n| format!("old {n}\n")).collect::<String>();
        let work = (0..count).map(|n| format!("new {n}\n")).collect::<String>();
        let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        assert_eq!(review.line_selection_available(), count == 512);
        if count == 513 {
            let before = f.basis();
            assert!(desktop_git::stage_lines(&review, &[row(&review, "+new 0\n")]).is_err());
            assert_eq!(f.basis(), before);
        }
        desktop_git::stage_hunk(&review, 0).unwrap();
        assert_eq!(f.index(), work.as_bytes());
    }
    let f = Fixture::new("file", &vec![b'a'; 40_000], &vec![b'b'; 40_000]);
    let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
    assert!(!review.line_selection_available());
    assert!(desktop_git::stage_lines(&review, &[HunkLine { hunk: 0, line: 1 }]).is_err());
    desktop_git::stage_hunk(&review, 0).unwrap();
    assert_eq!(f.index(), vec![b'b'; 40_000]);
}

#[test]
fn stale_or_locked_line_operations_cannot_apply_a_partial_selection() {
    for staged in [false, true] {
        for mutation in ["working", "index", "lock"] {
            let f = Fixture::new("file", &contents(&[]), &contents(&[10, 12]));
            if staged {
                f.git(&["add", "--all"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            let selection = [
                row(&review, "-line 010\n"),
                row(&review, "+line 010 edited\n"),
            ];
            match mutation {
                "working" => std::fs::write(f.file(), contents(&[11, 12])).unwrap(),
                "index" => {
                    std::fs::write(f.root.join("other"), b"new").unwrap();
                    f.git(&["add", "--", "other"]);
                }
                _ => std::fs::write(f.root.join(".git/index.lock"), b"fixture lock").unwrap(),
            }
            let before = f.basis();
            let bytes = std::fs::read(f.file()).unwrap();
            let result = if staged {
                desktop_git::unstage_lines(&review, &selection)
            } else {
                desktop_git::stage_lines(&review, &selection)
            };
            assert!(result.is_err());
            assert_eq!(f.basis(), before);
            assert_eq!(std::fs::read(f.file()).unwrap(), bytes);
        }
    }
}

#[cfg(unix)]
#[test]
fn individual_line_operations_preserve_quoted_native_names_and_crlf() {
    use std::os::unix::ffi::OsStrExt;
    for name in [
        PathBuf::from(":(glob)*\n\tfile"),
        PathBuf::from(OsStr::from_bytes(b"lines-\xff")),
    ] {
        let f = Fixture::new(
            name,
            b"before\r\nold\r\nend\r\n",
            b"before\r\nnew\r\nend\r\n",
        );
        let review = desktop_git::review_hunks(&f.root, &f.change()).unwrap();
        desktop_git::stage_lines(
            &review,
            &[row(&review, "-old\r\n"), row(&review, "+new\r\n")],
        )
        .unwrap();
        assert_eq!(f.index(), b"before\r\nnew\r\nend\r\n");
        let staged = desktop_git::review_staged_hunks(&f.root, &f.change()).unwrap();
        desktop_git::unstage_lines(
            &staged,
            &[row(&staged, "-old\r\n"), row(&staged, "+new\r\n")],
        )
        .unwrap();
        assert_eq!(f.index(), b"before\r\nold\r\nend\r\n");
        assert_eq!(
            std::fs::read(f.file()).unwrap(),
            b"before\r\nnew\r\nend\r\n"
        );
    }
}

#[test]
fn every_nonempty_subset_of_a_two_line_replacement_has_exact_forward_and_reverse_bytes() {
    for staged in [false, true] {
        let f = Fixture::new(
            "file",
            b"before\nold A\nold B\nafter\n",
            b"before\nnew A\nnew B\nafter\n",
        );
        for mask in 1u8..16 {
            if staged {
                f.git(&["add", "--all"]);
            } else {
                f.git(&["reset", "--quiet", "--", "file"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            let values = ["-old A\n", "-old B\n", "+new A\n", "+new B\n"];
            let selection = values
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, value)| row(&review, value))
                .collect::<Vec<_>>();
            let mut expected = String::from("before\n");
            for (index, value) in ["old A\n", "old B\n"].iter().enumerate() {
                if (mask & (1 << index) != 0) == staged {
                    expected.push_str(value);
                }
            }
            for (index, value) in ["new A\n", "new B\n"].iter().enumerate() {
                if (mask & (1 << (index + 2)) != 0) != staged {
                    expected.push_str(value);
                }
            }
            expected.push_str("after\n");
            if staged {
                desktop_git::unstage_lines(&review, &selection).unwrap();
            } else {
                desktop_git::stage_lines(&review, &selection).unwrap();
            }
            assert_eq!(
                f.index(),
                expected.as_bytes(),
                "staged={staged} mask={mask}"
            );
            assert_eq!(
                std::fs::read(f.file()).unwrap(),
                b"before\nnew A\nnew B\nafter\n"
            );
        }
    }
}

#[test]
fn eof_replacement_subsets_preserve_bytes_or_refuse_implicit_newline_insertion() {
    for old_nl in [false, true] {
        for new_nl in [false, true] {
            for staged in [false, true] {
                let old = format!("old{}", if old_nl { "\n" } else { "" });
                let new = format!("new{}", if new_nl { "\n" } else { "" });
                let f = Fixture::new("file", old.as_bytes(), new.as_bytes());
                for choice in ["addition", "deletion", "both"] {
                    if staged {
                        f.git(&["add", "--all"]);
                    } else {
                        f.git(&["reset", "--quiet", "--", "file"]);
                    }
                    let review = if staged {
                        desktop_git::review_staged_hunks(&f.root, &f.change())
                    } else {
                        desktop_git::review_hunks(&f.root, &f.change())
                    }
                    .unwrap();
                    let add = row(&review, "+new\n");
                    let delete = row(&review, "-old\n");
                    let selection = match choice {
                        "addition" => vec![add],
                        "deletion" => vec![delete],
                        _ => vec![delete, add],
                    };
                    let before = f.basis();
                    let result = if staged {
                        desktop_git::unstage_lines(&review, &selection)
                    } else {
                        desktop_git::stage_lines(&review, &selection)
                    };
                    let ambiguous = !old_nl
                        && ((staged && choice == "deletion") || (!staged && choice == "addition"));
                    if ambiguous {
                        assert!(
                            result.is_err(),
                            "staged={staged} old_nl={old_nl} new_nl={new_nl} {choice}"
                        );
                        assert_eq!(f.basis(), before);
                    } else {
                        result.unwrap();
                        let expected = match (staged, choice) {
                            (false, "addition") | (true, "deletion") => {
                                format!("{old}{new}").into_bytes()
                            }
                            (false, "deletion") | (true, "addition") => Vec::new(),
                            (false, _) => new.as_bytes().to_vec(),
                            _ => old.as_bytes().to_vec(),
                        };
                        assert_eq!(
                            f.index(),
                            expected,
                            "staged={staged} old_nl={old_nl} new_nl={new_nl} {choice}"
                        );
                    }
                    assert_eq!(std::fs::read(f.file()).unwrap(), new.as_bytes());
                }
            }
        }
    }
}

#[test]
fn partial_edits_before_unchanged_unterminated_context_are_selectable() {
    for ending in ["\n", "\r\n"] {
        for staged in [false, true] {
            let base = format!("before{ending}old A{ending}old B{ending}tail");
            let work = format!("before{ending}new A{ending}new B{ending}tail");
            let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
            if staged {
                f.git(&["add", "--all"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            assert!(review.hunks[0].contains("\\ No newline"));
            let selection = [
                row(&review, &format!("-old A{ending}")),
                row(&review, &format!("+new A{ending}")),
            ];
            if staged {
                desktop_git::unstage_lines(&review, &selection).unwrap();
            } else {
                desktop_git::stage_lines(&review, &selection).unwrap();
            }
            let expected = if staged {
                format!("before{ending}old A{ending}new B{ending}tail")
            } else {
                // Retained deletions become context before selected additions in the same change run.
                format!("before{ending}old B{ending}new A{ending}tail")
            };
            assert_eq!(f.index(), expected.as_bytes());
            assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
        }
    }
}

#[test]
fn eof_selection_and_an_earlier_partial_hunk_keep_offsets_and_unselected_rows() {
    let base = String::from_utf8(contents(&[]))
        .unwrap()
        .trim_end_matches('\n')
        .to_owned();
    let work = base
        .replace("line 005\n", "line 005\ninsert A\ninsert B\n")
        .replace("line 049", "line 049 edited");
    for staged in [false, true] {
        let f = Fixture::new("file", base.as_bytes(), work.as_bytes());
        if staged {
            f.git(&["add", "--all"]);
        }
        let review = if staged {
            desktop_git::review_staged_hunks(&f.root, &f.change())
        } else {
            desktop_git::review_hunks(&f.root, &f.change())
        }
        .unwrap();
        assert_eq!(review.hunks.len(), 2);
        let end = if staged {
            row(&review, "+line 049 edited\n")
        } else {
            row(&review, "-line 049\n")
        };
        let selection = [row(&review, "+insert A\n"), end];
        if staged {
            desktop_git::unstage_lines(&review, &selection).unwrap();
        } else {
            desktop_git::stage_lines(&review, &selection).unwrap();
        }
        let expected = if staged {
            work.replace("insert A\n", "")
                .strip_suffix("line 049 edited")
                .unwrap()
                .to_owned()
        } else {
            base.replace("line 005\n", "line 005\ninsert A\n")
                .strip_suffix("line 049")
                .unwrap()
                .to_owned()
        };
        assert_eq!(f.index(), expected.as_bytes());
        assert_eq!(std::fs::read(f.file()).unwrap(), work.as_bytes());
    }
}

#[test]
fn selecting_only_a_newline_change_or_empty_file_transition_preserves_exact_eof() {
    for (base, work) in [
        (b"same".as_slice(), b"same\n".as_slice()),
        (b"same\n", b"same"),
        (b"", b"added"),
        (b"removed", b""),
    ] {
        for staged in [false, true] {
            let f = Fixture::new("file", base, work);
            if staged {
                f.git(&["add", "--all"]);
            }
            let review = if staged {
                desktop_git::review_staged_hunks(&f.root, &f.change())
            } else {
                desktop_git::review_hunks(&f.root, &f.change())
            }
            .unwrap();
            let selection = review
                .hunks
                .iter()
                .enumerate()
                .flat_map(|(hunk, body)| {
                    body.split_inclusive('\n')
                        .enumerate()
                        .filter(|(_, row)| matches!(row.as_bytes().first(), Some(b'+' | b'-')))
                        .map(move |(line, _)| HunkLine { hunk, line })
                })
                .collect::<Vec<_>>();
            if staged {
                desktop_git::unstage_lines(&review, &selection).unwrap();
            } else {
                desktop_git::stage_lines(&review, &selection).unwrap();
            }
            assert_eq!(f.index(), if staged { base } else { work });
            assert_eq!(std::fs::read(f.file()).unwrap(), work);
        }
    }
}
