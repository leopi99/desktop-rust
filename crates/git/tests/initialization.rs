//! Git integration coverage for initialization.
#![cfg(test)]

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Area(PathBuf);
impl Area {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "desktop-init-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Area {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

#[test]
fn new_repository_supports_the_complete_first_commit_flow() {
    let area = Area::new();
    let path = desktop_git::init_repository(&area.0.join("new project"), "main").unwrap();
    let empty = desktop_git::snapshot(&path).unwrap();
    assert!(empty.commit_basis.head.is_none());
    assert!(empty.changes.is_empty());
    assert_eq!(
        empty.commit_basis.branch.as_deref(),
        Some(std::ffi::OsStr::new("refs/heads/main"))
    );
    for (key, value) in [
        ("user.name", "Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(&path, &["config", key, value]);
    }
    std::fs::write(path.join("README.md"), b"New project\n").unwrap();
    let state = desktop_git::snapshot(&path).unwrap();
    desktop_git::stage(&path, &state.changes[0]).unwrap();
    let state = desktop_git::snapshot(&path).unwrap();
    desktop_git::commit(&path, "Initial commit", &state.commit_basis).unwrap();
    assert!(desktop_git::snapshot(&path).unwrap().changes.is_empty());
    assert_eq!(git(&path, &["show", "HEAD:README.md"]), b"New project\n");
}

#[test]
fn existing_project_files_are_untouched_and_unstaged() {
    let area = Area::new();
    let project = area.0.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("source"), b"keep me").unwrap();
    desktop_git::init_repository(&project, "feature/initial").unwrap();
    assert_eq!(std::fs::read(project.join("source")).unwrap(), b"keep me");
    let state = desktop_git::snapshot(&project).unwrap();
    assert_eq!(state.changes.len(), 1);
    assert_eq!(state.changes[0].index, '?');
    assert_eq!(
        state.commit_basis.branch.as_deref(),
        Some(std::ffi::OsStr::new("refs/heads/feature/initial"))
    );
}

#[test]
fn existing_repositories_and_nested_destinations_are_rejected() {
    let area = Area::new();
    let project = area.0.join("project");
    desktop_git::init_repository(&project, "main").unwrap();
    let head = std::fs::read(project.join(".git/HEAD")).unwrap();
    assert!(desktop_git::init_repository(&project, "other").is_err());
    assert_eq!(std::fs::read(project.join(".git/HEAD")).unwrap(), head);
    assert!(desktop_git::init_repository(&project.join("nested"), "main").is_err());
    assert!(!project.join("nested").exists());
    let existing = project.join("existing");
    std::fs::create_dir(&existing).unwrap();
    assert!(desktop_git::init_repository(&existing, "main").is_err());
    assert!(!existing.join(".git").exists());
}

#[test]
fn existing_bare_repository_is_not_reinitialized() {
    let area = Area::new();
    git(
        &area.0,
        &["init", "--bare", "--initial-branch=main", "bare"],
    );
    let bare = area.0.join("bare");
    let head = std::fs::read(bare.join("HEAD")).unwrap();
    assert!(desktop_git::init_repository(&bare, "other").is_err());
    assert_eq!(std::fs::read(bare.join("HEAD")).unwrap(), head);
    assert!(!bare.join(".git").exists());
}

#[test]
fn malformed_gitfile_blocks_initialization_without_overwrite() {
    let area = Area::new();
    let project = area.0.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join(".git"), b"gitdir: missing-worktree-metadata\n").unwrap();
    assert!(desktop_git::init_repository(&project, "main").is_err());
    assert_eq!(
        std::fs::read(project.join(".git")).unwrap(),
        b"gitdir: missing-worktree-metadata\n"
    );
}

#[test]
fn invalid_names_do_not_create_directories_or_metadata() {
    let area = Area::new();
    let target = area.0.join("invalid");
    for name in ["", "--force", "HEAD", "@{-1}", "bad name", "bad.lock"] {
        assert!(desktop_git::init_repository(&target, name).is_err());
        assert!(!target.exists());
    }
    std::fs::create_dir(&target).unwrap();
    assert!(desktop_git::init_repository(&target, "bad name").is_err());
    assert!(!target.join(".git").exists());
}

#[test]
fn files_missing_parents_and_empty_destinations_are_rejected() {
    let area = Area::new();
    let file = area.0.join("file");
    std::fs::write(&file, b"keep").unwrap();
    assert!(desktop_git::init_repository(&file, "main").is_err());
    assert_eq!(std::fs::read(file).unwrap(), b"keep");
    assert!(desktop_git::init_repository(&area.0.join("missing/project"), "main").is_err());
    assert!(!area.0.join("missing").exists());
    assert!(desktop_git::init_repository(Path::new(""), "main").is_err());
}

#[cfg(unix)]
#[test]
fn symlink_destinations_and_broken_git_markers_are_rejected() {
    let area = Area::new();
    let real = area.0.join("real");
    std::fs::create_dir(&real).unwrap();
    let link = area.0.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(desktop_git::init_repository(&link, "main").is_err());
    assert!(!real.join(".git").exists());
    std::os::unix::fs::symlink("missing", real.join(".git")).unwrap();
    assert!(desktop_git::init_repository(&real, "main").is_err());
    assert!(std::fs::symlink_metadata(real.join(".git"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn inherited_git_environment_cannot_redirect_initialization() {
    const ROOT: &str = "DESKTOP_INIT_ENV_TEST_ROOT";
    if let Some(root) = std::env::var_os(ROOT) {
        let root = PathBuf::from(root);
        let project = desktop_git::init_repository(&root.join("project"), "main").unwrap();
        assert_eq!(
            desktop_git::snapshot(&project).unwrap().repository.path,
            project
        );
        return;
    }
    let area = Area::new();
    git(
        &area.0,
        &["init", "--bare", "--initial-branch=decoy", "decoy.git"],
    );
    let decoy = area.0.join("decoy.git");
    let before = std::fs::read(decoy.join("HEAD")).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "inherited_git_environment_cannot_redirect_initialization",
            "--nocapture",
        ])
        .env(ROOT, &area.0)
        .env("GIT_DIR", &decoy)
        .env("GIT_WORK_TREE", &area.0)
        .env("GIT_INDEX_FILE", area.0.join("foreign-index"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(decoy.join("HEAD")).unwrap(), before);
    assert!(!area.0.join("foreign-index").exists());
    assert!(area.0.join("project/.git/HEAD").is_file());
}
