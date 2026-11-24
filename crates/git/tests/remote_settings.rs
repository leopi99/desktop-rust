use desktop_core::{Remote, RemoteAction, RemoteUrlKind};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    server: PathBuf,
}
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
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "desktop-remotes-{}-{}",
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
        let server = root.join("remote.git");
        git(
            &root,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                server.to_str().unwrap(),
            ],
        );
        Self { root, repo, server }
    }
    fn remote(&self) -> Remote {
        desktop_git::snapshot(&self.repo)
            .unwrap()
            .remotes
            .into_iter()
            .find(|r| r.name == "origin")
            .unwrap()
    }
    fn add(&self) {
        desktop_git::add_remote(&self.repo, "origin", self.server.to_str().unwrap()).unwrap();
    }
    fn config(&self) -> Vec<u8> {
        std::fs::read(self.repo.join(".git/config")).unwrap()
    }
    fn commit(&self) {
        std::fs::write(self.repo.join("file"), b"project\n").unwrap();
        let snapshot = desktop_git::snapshot(&self.repo).unwrap();
        desktop_git::stage(&self.repo, &snapshot.changes[0]).unwrap();
        let snapshot = desktop_git::snapshot(&self.repo).unwrap();
        desktop_git::commit(&self.repo, "Initial commit", &snapshot.commit_basis).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn newly_created_project_can_be_connected_and_published() {
    let fixture = Fixture::new();
    fixture.commit();
    let before = desktop_git::snapshot(&fixture.repo).unwrap().commit_basis;
    fixture.add();
    let snapshot = desktop_git::snapshot(&fixture.repo).unwrap();
    assert_eq!(snapshot.commit_basis, before);
    assert_eq!(
        snapshot.remotes[0].fetch_urls,
        snapshot.remotes[0].push_urls
    );
    desktop_git::remote_operation(
        &fixture.repo,
        &fixture.remote(),
        RemoteAction::Push,
        &snapshot.commit_basis,
    )
    .unwrap();
    assert_eq!(git(&fixture.server, &["show", "main:file"]), b"project\n");
    assert_eq!(
        git(&fixture.repo, &["config", "branch.main.remote"]),
        b"origin\n"
    );
}

#[test]
fn remote_add_never_fetches_or_changes_the_index() {
    let fixture = Fixture::new();
    let before = desktop_git::snapshot(&fixture.repo).unwrap().commit_basis;
    fixture.add();
    assert_eq!(
        desktop_git::snapshot(&fixture.repo).unwrap().commit_basis,
        before
    );
    assert!(git(&fixture.repo, &["for-each-ref", "refs/remotes/"]).is_empty());
    assert!(!fixture.repo.join(".git/FETCH_HEAD").exists());
}

#[test]
fn duplicate_or_invalid_remote_names_leave_configuration_unchanged() {
    let fixture = Fixture::new();
    fixture.add();
    let before = fixture.config();
    for name in [
        "origin", "", "--force", ".", "bad name", "@{-1}", "bad.lock",
    ] {
        assert!(
            desktop_git::add_remote(&fixture.repo, name, "/new/remote").is_err(),
            "{name}"
        );
        assert_eq!(fixture.config(), before);
    }
}

#[test]
fn fetch_url_change_updates_fallback_push_but_preserves_explicit_push_url() {
    let fixture = Fixture::new();
    fixture.add();
    desktop_git::set_remote_url(
        &fixture.repo,
        &fixture.remote(),
        RemoteUrlKind::Fetch,
        "/new/fetch",
    )
    .unwrap();
    assert_eq!(fixture.remote().fetch_urls, fixture.remote().push_urls);
    desktop_git::set_remote_url(
        &fixture.repo,
        &fixture.remote(),
        RemoteUrlKind::Push,
        "/separate/push",
    )
    .unwrap();
    desktop_git::set_remote_url(
        &fixture.repo,
        &fixture.remote(),
        RemoteUrlKind::Fetch,
        "/other/fetch",
    )
    .unwrap();
    assert_eq!(
        fixture.remote().fetch_urls,
        vec![std::ffi::OsString::from("/other/fetch")]
    );
    assert_eq!(
        fixture.remote().push_urls,
        vec![std::ffi::OsString::from("/separate/push")]
    );
}

#[test]
fn stale_remote_cannot_be_edited_or_removed() {
    let fixture = Fixture::new();
    fixture.add();
    let stale = fixture.remote();
    git(
        &fixture.repo,
        &["remote", "set-url", "origin", "/external/change"],
    );
    let before = fixture.config();
    assert!(
        desktop_git::set_remote_url(&fixture.repo, &stale, RemoteUrlKind::Fetch, "/new/url")
            .is_err()
    );
    assert!(desktop_git::remove_remote(&fixture.repo, &stale).is_err());
    assert_eq!(fixture.config(), before);
}

#[test]
fn multi_url_endpoints_are_rejected_without_partial_edits() {
    let fixture = Fixture::new();
    fixture.add();
    git(
        &fixture.repo,
        &["remote", "set-url", "--add", "origin", "/second/url"],
    );
    let before = fixture.config();
    for kind in [RemoteUrlKind::Fetch, RemoteUrlKind::Push] {
        assert!(
            desktop_git::set_remote_url(&fixture.repo, &fixture.remote(), kind, "/new/url")
                .is_err()
        );
        assert_eq!(fixture.config(), before);
    }
    git(
        &fixture.repo,
        &["config", "remote.origin.pushurl", "/explicit/push"],
    );
    desktop_git::set_remote_url(
        &fixture.repo,
        &fixture.remote(),
        RemoteUrlKind::Push,
        "/updated/push",
    )
    .unwrap();
    assert_eq!(fixture.remote().fetch_urls.len(), 2);
    assert_eq!(
        fixture.remote().push_urls,
        vec![std::ffi::OsString::from("/updated/push")]
    );
}

#[test]
fn removing_remote_keeps_local_branches_index_files_and_server() {
    let fixture = Fixture::new();
    fixture.commit();
    fixture.add();
    let before = desktop_git::snapshot(&fixture.repo).unwrap().commit_basis;
    desktop_git::remote_operation(
        &fixture.repo,
        &fixture.remote(),
        RemoteAction::Push,
        &before,
    )
    .unwrap();
    let server_head = git(&fixture.server, &["rev-parse", "HEAD"]);
    assert!(!git(&fixture.repo, &["for-each-ref", "refs/remotes/origin/"]).is_empty());
    let before = desktop_git::snapshot(&fixture.repo).unwrap().commit_basis;
    desktop_git::remove_remote(&fixture.repo, &fixture.remote()).unwrap();
    let after = desktop_git::snapshot(&fixture.repo).unwrap();
    assert!(after.remotes.is_empty());
    assert_eq!(after.commit_basis, before);
    assert_eq!(after.branches[0].name, "main");
    assert!(git(&fixture.repo, &["for-each-ref", "refs/remotes/origin/"]).is_empty());
    assert_eq!(
        std::fs::read(fixture.repo.join("file")).unwrap(),
        b"project\n"
    );
    assert_eq!(git(&fixture.server, &["rev-parse", "HEAD"]), server_head);
}

#[test]
fn invalid_urls_never_change_configuration() {
    let fixture = Fixture::new();
    fixture.add();
    let before = fixture.config();
    for url in [
        "",
        "  ",
        "--upload-pack=bad",
        "ext::command",
        "unknown://host/repo",
        "line\nbreak",
        "nul\0byte",
    ] {
        assert!(desktop_git::add_remote(&fixture.repo, "other", url).is_err());
        assert!(desktop_git::set_remote_url(
            &fixture.repo,
            &fixture.remote(),
            RemoteUrlKind::Push,
            url
        )
        .is_err());
        assert_eq!(fixture.config(), before);
    }
}

#[test]
fn ssh_ipv6_and_paths_with_spaces_are_preserved_without_connection() {
    let fixture = Fixture::new();
    desktop_git::add_remote(&fixture.repo, "origin", "ssh://git@[::1]/owner/repo.git").unwrap();
    assert_eq!(
        fixture.remote().fetch_urls[0],
        "ssh://git@[::1]/owner/repo.git"
    );
    desktop_git::set_remote_url(
        &fixture.repo,
        &fixture.remote(),
        RemoteUrlKind::Fetch,
        "../repo with spaces",
    )
    .unwrap();
    assert_eq!(fixture.remote().fetch_urls[0], "../repo with spaces");
}

#[cfg(unix)]
#[test]
fn existing_non_utf8_remote_names_can_be_edited_and_removed() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"remote-\xff");
    let output = Command::new("git")
        .arg("-C")
        .arg(&fixture.repo)
        .args([
            std::ffi::OsStr::new("remote"),
            std::ffi::OsStr::new("add"),
            name,
            fixture.server.as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let selected = desktop_git::snapshot(&fixture.repo)
        .unwrap()
        .remotes
        .into_iter()
        .find(|r| r.name == name)
        .unwrap();
    desktop_git::set_remote_url(
        &fixture.repo,
        &selected,
        RemoteUrlKind::Fetch,
        "/updated/url",
    )
    .unwrap();
    let selected = desktop_git::snapshot(&fixture.repo)
        .unwrap()
        .remotes
        .into_iter()
        .find(|r| r.name == name)
        .unwrap();
    desktop_git::remove_remote(&fixture.repo, &selected).unwrap();
    assert!(desktop_git::snapshot(&fixture.repo)
        .unwrap()
        .remotes
        .is_empty());
}
