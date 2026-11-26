//! Developer launch options and captures of the application's own window.
use super::{ConflictSide, Desktop, HunkSide, Message, Panel};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

type Error = Box<dyn std::error::Error>;

#[derive(Debug)]
pub(super) struct Start {
    repository: Option<PathBuf>,
    pub(super) capture: Option<PathBuf>,
    panel: Panel,
    review_delete: bool,
    review_navigation: bool,
    review_scroll: bool,
    review_hunks: Option<HunkSide>,
    select_hunks: bool,
    select_lines: bool,
    review_side: Option<ConflictSide>,
    pub(super) size: iced::Size,
}

impl Start {
    #[expect(
        clippy::print_stdout,
        reason = "The developer CLI help is explicit stdout output"
    )]
    pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Option<Self>, Error> {
        let mut start = Self {
            repository: None,
            capture: None,
            panel: Panel::Changes,
            review_delete: false,
            review_navigation: false,
            review_scroll: false,
            review_hunks: None,
            select_hunks: false,
            select_lines: false,
            review_side: None,
            size: iced::Size::new(1200.0, 800.0),
        };
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            if flag == "--help" {
                println!("desktop-rust [--repository PATH] [--panel changes|history|branches|remotes|stash|repository|git|keyboard-scroll-review|keyboard-nav-review|hunk-review|hunk-unstage-review|line-selection-review|line-unstage-selection-review|hunk-selection-review|hunk-unstage-selection-review|branch-delete-review|conflict-ours-review|conflict-theirs-review] [--size WIDTHxHEIGHT] [--capture NEW.png]\nCapture saves only this app's window, then exits. Requires a graphical session.");
                return Ok(None);
            }
            if !["--repository", "--panel", "--size", "--capture"]
                .iter()
                .any(|s| flag == *s)
            {
                return Err(format!("Unknown option: {}", flag.to_string_lossy()).into());
            }
            let value = args.next().ok_or("Missing option value")?;
            match flag.to_str() {
                Some("--repository") => start.repository = Some(value.into()),
                Some("--capture") => start.capture = Some(value.into()),
                Some("--panel") => {
                    start.review_delete = value == "branch-delete-review";
                    start.review_navigation = value == "keyboard-nav-review";
                    start.review_scroll = value == "keyboard-scroll-review";
                    start.select_lines = matches!(
                        value.to_str(),
                        Some("line-selection-review" | "line-unstage-selection-review")
                    );
                    start.select_hunks = matches!(
                        value.to_str(),
                        Some("hunk-selection-review" | "hunk-unstage-selection-review")
                    );
                    start.review_hunks = match value.to_str() {
                        Some("hunk-review" | "hunk-selection-review" | "line-selection-review") => {
                            Some(HunkSide::Unstaged)
                        }
                        Some(
                            "hunk-unstage-review"
                            | "hunk-unstage-selection-review"
                            | "line-unstage-selection-review",
                        ) => Some(HunkSide::Staged),
                        _ => None,
                    };
                    start.review_side = match value.to_str() {
                        Some("conflict-ours-review") => Some(ConflictSide::Ours),
                        Some("conflict-theirs-review") => Some(ConflictSide::Theirs),
                        _ => None,
                    };
                    start.panel = match value.to_str() {
                        Some(
                            "changes"
                            | "keyboard-nav-review"
                            | "hunk-review"
                            | "line-selection-review"
                            | "line-unstage-selection-review"
                            | "hunk-unstage-review"
                            | "hunk-selection-review"
                            | "hunk-unstage-selection-review"
                            | "conflict-ours-review"
                            | "conflict-theirs-review",
                        ) => Panel::Changes,
                        Some("history") => Panel::History,
                        Some("branches" | "branch-delete-review") => Panel::Branches,
                        Some("remotes") => Panel::Remotes,
                        Some("stash") => Panel::Stashes,
                        Some("repository") => Panel::Repository,
                        Some("git" | "keyboard-scroll-review") => Panel::Git,
                        _ => return Err("Unknown panel".into()),
                    }
                }
                Some("--size") => {
                    let (width, height) = value
                        .to_str()
                        .and_then(|v| v.split_once('x'))
                        .ok_or("Size must be WIDTHxHEIGHT")?;
                    let width: u32 = width.parse()?;
                    let height: u32 = height.parse()?;
                    if !(960..=3840).contains(&width) || !(640..=2160).contains(&height) {
                        return Err("Window size must be between 960x640 and 3840x2160".into());
                    }
                    start.size = iced::Size::new(width as f32, height as f32);
                }
                _ => unreachable!(),
            }
        }
        if (start.review_delete
            || start.review_navigation
            || start.review_scroll
            || start.review_hunks.is_some()
            || start.review_side.is_some())
            && (start.capture.is_none() || start.repository.is_none())
        {
            return Err("Review captures require --repository and --capture".into());
        }
        if start.capture.as_ref().is_some_and(|path| path.exists()) {
            return Err("Capture output already exists; choose a fresh path".into());
        }
        Ok(Some(start))
    }

    pub(super) fn initialize(self) -> Result<Desktop, Error> {
        let mut app = Desktop {
            capture: self.capture,
            capture_navigation_focus: self.review_navigation,
            capture_scroll_focus: self.review_scroll,
            ..Desktop::default()
        };
        // Launch-time inspection is read-only; no transfer or Git write is triggered.
        if let Some(path) = self.repository {
            app.path = path.to_string_lossy().into_owned();
            let snapshot = desktop_git::snapshot(&path)?;
            let _ = app.update(Message::Loaded(Ok(snapshot)));
        }
        let _ = app.update(Message::PanelSelected(self.panel));
        if let Some(snapshot) = &app.snapshot {
            if self.panel == Panel::Changes {
                if let Some(change) = snapshot.changes.first() {
                    let diff = desktop_git::diff(&snapshot.repository.path, change)?;
                    let _ = app.update(Message::DiffLoaded(Ok(diff)));
                }
            } else if self.panel == Panel::History {
                if let Some(commit) = app.history_commits.first() {
                    let details =
                        desktop_git::commit_details(&snapshot.repository.path, &commit.oid)?;
                    let _ = app.update(Message::CommitLoaded(Ok(details)));
                }
            } else if self.panel == Panel::Remotes {
                let remote = snapshot.remotes.first().cloned();
                app.select_remote(remote);
            }
        }
        if let Some(side) = self.review_hunks {
            let snapshot = app
                .snapshot
                .as_ref()
                .ok_or("Hunk capture requires a repository")?;
            let change = snapshot
                .changes
                .iter()
                .find(|change| match side {
                    HunkSide::Unstaged => change.can_review_unstaged_hunks(),
                    HunkSide::Staged => change.can_review_staged_hunks(),
                })
                .ok_or("Hunk capture needs a supported regular text change")?;
            let review = match side {
                HunkSide::Unstaged => desktop_git::review_hunks(&snapshot.repository.path, change)?,
                HunkSide::Staged => {
                    desktop_git::review_staged_hunks(&snapshot.repository.path, change)?
                }
            };
            // Capture prepares only; it never sends StageHunk or UnstageHunk.
            let _ = app.update(Message::HunksReviewed(Ok(review)));
            if self.select_lines {
                let rows = app
                    .hunk_review
                    .as_ref()
                    .and_then(|review| review.hunks.first())
                    .map(|hunk| {
                        hunk.split_inclusive('\n')
                            .enumerate()
                            .filter(|(_, row)| matches!(row.as_bytes().first(), Some(b'+' | b'-')))
                            .take(2)
                            .map(|(line, _)| crate::HunkLine { hunk: 0, line })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for line in rows {
                    let _ = app.update(Message::ToggleHunkLine(line, true));
                }
            }
            if self.select_hunks {
                for index in [0, 2] {
                    let _ = app.update(Message::ToggleHunk(index, true));
                }
            }
        }
        if let Some(side) = self.review_side {
            let snapshot = app
                .snapshot
                .as_ref()
                .ok_or("Conflict review needs a repository")?;
            let change = snapshot
                .changes
                .iter()
                .find(|change| change.is_conflicted())
                .ok_or("Conflict review capture needs an unmerged file")?;
            let review = desktop_git::review_conflict_side_resolution(
                &snapshot.repository.path,
                change,
                side,
            )?;
            // Prepare only; never send ApplyConflictSideResolution during capture.
            let _ = app.update(Message::ConflictSideResolutionReviewed(Ok(review)));
        }
        if self.review_delete {
            let branch = app
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.branches.iter().find(|branch| !branch.is_current))
                .cloned()
                .ok_or("Deletion review capture needs a non-current local branch")?;
            let _ = app.update(Message::BranchSelected(branch));
            // Render the review only. Never send Message::DeleteBranch here.
            let _ = app.update(Message::RequestDeleteBranch);
            if app.delete_branch_pending.is_none() {
                return Err("The repository cannot currently review branch deletion".into());
            }
        }
        Ok(app)
    }
}

pub(super) fn save(path: &Path, screenshot: &iced::window::Screenshot) -> Result<(), Error> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut encoder = png::Encoder::new(file, screenshot.size.width, screenshot.size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&screenshot.bytes)?;
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Start>, Error> {
        Start::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn rejects_invalid_launch_options() {
        for args in [
            vec!["--capture"],
            vec!["--wat"],
            vec!["--panel", "invalid"],
            vec!["--panel", "branch-delete-review"],
            vec![
                "--panel",
                "branch-delete-review",
                "--repository",
                "/tmp/repo",
            ],
            vec!["--size", "NaNx800"],
            vec!["--size", "799x600"],
            vec!["--size", "1200x599"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn conflict_review_capture_requires_explicit_repository_and_capture() {
        for panel in ["conflict-ours-review", "conflict-theirs-review"] {
            assert!(parse(&["--panel", panel]).is_err());
            assert!(parse(&["--panel", panel, "--repository", "/tmp/repo"]).is_err());
            assert!(parse(&["--panel", panel, "--capture", "/tmp/no-repo.png"]).is_err());
            let start = parse(&[
                "--panel",
                panel,
                "--repository",
                "/tmp/repo",
                "--capture",
                "/tmp/review-new.png",
            ])
            .unwrap()
            .unwrap();
            assert_eq!(start.panel, Panel::Changes);
            assert_eq!(
                start.review_side,
                Some(if panel == "conflict-ours-review" {
                    ConflictSide::Ours
                } else {
                    ConflictSide::Theirs
                })
            );
        }
    }

    #[test]
    fn navigation_focus_capture_requires_explicit_read_only_context() {
        for args in [
            vec!["--panel", "keyboard-scroll-review"],
            vec![
                "--panel",
                "keyboard-scroll-review",
                "--repository",
                "/tmp/repo",
            ],
            vec![
                "--panel",
                "keyboard-scroll-review",
                "--capture",
                "/tmp/new-scroll.png",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
        let scroll = parse(&[
            "--panel",
            "keyboard-scroll-review",
            "--repository",
            "/tmp/repo",
            "--capture",
            "/tmp/new-scroll.png",
        ])
        .unwrap()
        .unwrap();
        assert!(scroll.review_scroll);
        assert!(!scroll.review_navigation);
        assert_eq!(scroll.panel, Panel::Git);

        for args in [
            vec!["--panel", "keyboard-nav-review"],
            vec![
                "--panel",
                "keyboard-nav-review",
                "--repository",
                "/tmp/repo",
            ],
            vec![
                "--panel",
                "keyboard-nav-review",
                "--capture",
                "/tmp/new-focus.png",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
        let start = parse(&[
            "--panel",
            "keyboard-nav-review",
            "--repository",
            "/tmp/repo",
            "--capture",
            "/tmp/new-focus.png",
        ])
        .unwrap()
        .unwrap();
        assert!(start.review_navigation);
        assert_eq!(start.panel, Panel::Changes);
        assert!(start.review_hunks.is_none());
        assert!(start.review_side.is_none());
        let replaced = parse(&["--panel", "keyboard-nav-review", "--panel", "history"])
            .unwrap()
            .unwrap();
        assert!(!replaced.review_navigation);
    }

    #[test]
    fn parses_panel_size_and_native_repository_path() {
        let start = parse(&[
            "--repository",
            "/tmp/my repository",
            "--panel",
            "history",
            "--size",
            "960x640",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(start.panel, Panel::History);
        assert_eq!(start.size, iced::Size::new(960.0, 640.0));
        assert_eq!(start.repository, Some(PathBuf::from("/tmp/my repository")));
    }

    #[test]
    fn hunk_capture_requires_both_repository_and_destination() {
        for args in [
            vec!["--panel", "hunk-review"],
            vec!["--panel", "hunk-review", "--repository", "/tmp/repo"],
            vec!["--panel", "hunk-review", "--capture", "/tmp/new-hunks.png"],
        ] {
            assert!(parse(&args).is_err());
        }
        let start = parse(&[
            "--panel",
            "hunk-review",
            "--repository",
            "/tmp/repo",
            "--capture",
            "/tmp/new-hunks.png",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(start.review_hunks, Some(HunkSide::Unstaged));
        assert_eq!(start.panel, Panel::Changes);
    }

    #[test]
    fn staged_hunk_capture_requires_explicit_repository_and_destination() {
        for args in [
            vec!["--panel", "hunk-unstage-review"],
            vec![
                "--panel",
                "hunk-unstage-review",
                "--repository",
                "/tmp/repo",
            ],
            vec![
                "--panel",
                "hunk-unstage-review",
                "--capture",
                "/tmp/new-staged.png",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
        let start = parse(&[
            "--panel",
            "hunk-unstage-review",
            "--repository",
            "/tmp/repo",
            "--capture",
            "/tmp/new-staged.png",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(start.review_hunks, Some(HunkSide::Staged));
        assert_eq!(start.panel, Panel::Changes);
    }

    #[test]
    fn selected_hunk_captures_are_explicit_read_only_review_modes() {
        for (panel, side) in [
            ("hunk-selection-review", HunkSide::Unstaged),
            ("hunk-unstage-selection-review", HunkSide::Staged),
        ] {
            assert!(parse(&["--panel", panel]).is_err());
            assert!(parse(&["--panel", panel, "--repository", "/tmp/repo"]).is_err());
            let start = parse(&[
                "--panel",
                panel,
                "--repository",
                "/tmp/repo",
                "--capture",
                "/tmp/new-selection.png",
            ])
            .unwrap()
            .unwrap();
            assert!(start.select_hunks);
            assert_eq!(start.review_hunks, Some(side));
        }
    }

    #[test]
    fn line_capture_modes_require_repository_and_destination_without_writing() {
        for (panel, side) in [
            ("line-selection-review", HunkSide::Unstaged),
            ("line-unstage-selection-review", HunkSide::Staged),
        ] {
            assert!(parse(&["--panel", panel]).is_err());
            assert!(parse(&["--panel", panel, "--repository", "/tmp/repo"]).is_err());
            let start = parse(&[
                "--panel",
                panel,
                "--repository",
                "/tmp/repo",
                "--capture",
                "/tmp/new-lines.png",
            ])
            .unwrap()
            .unwrap();
            assert!(start.select_lines);
            assert!(!start.select_hunks);
            assert_eq!(start.review_hunks, Some(side));
        }
    }

    #[test]
    fn help_does_not_launch() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }
    #[test]
    fn existing_capture_destination_is_rejected() {
        let path = std::env::current_exe().unwrap();
        assert!(Start::parse([OsString::from("--capture"), path.into_os_string()]).is_err());
    }
}
