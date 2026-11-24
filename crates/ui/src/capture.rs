//! Developer launch options and captures of the application's own window.
use super::{Desktop, Message, Panel};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

type Error = Box<dyn std::error::Error>;

pub(super) struct Start {
    repository: Option<PathBuf>,
    pub capture: Option<PathBuf>,
    panel: Panel,
    review_delete: bool,
    pub size: iced::Size,
}

impl Start {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Option<Self>, Error> {
        let mut start = Self {
            repository: None,
            capture: None,
            panel: Panel::Changes,
            review_delete: false,
            size: iced::Size::new(1200.0, 800.0),
        };
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            if flag == "--help" {
                println!("desktop-rust [--repository PATH] [--panel changes|history|branches|remotes|stash|repository|branch-delete-review] [--size WIDTHxHEIGHT] [--capture NEW.png]\nCapture saves only this app's window, then exits. Requires a graphical session.");
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
                    start.panel = match value.to_str() {
                        Some("changes") => Panel::Changes,
                        Some("history") => Panel::History,
                        Some("branches" | "branch-delete-review") => Panel::Branches,
                        Some("remotes") => Panel::Remotes,
                        Some("stash") => Panel::Stashes,
                        Some("repository") => Panel::Repository,
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
        if start.review_delete && (start.capture.is_none() || start.repository.is_none()) {
            return Err("branch-delete-review requires --repository and --capture".into());
        }
        if start.capture.as_ref().is_some_and(|path| path.exists()) {
            return Err("Capture output already exists; choose a fresh path".into());
        }
        Ok(Some(start))
    }

    pub fn initialize(self) -> Result<Desktop, Error> {
        let mut app = Desktop {
            capture: self.capture,
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
    fn help_does_not_launch() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }
    #[test]
    fn existing_capture_destination_is_rejected() {
        let path = std::env::current_exe().unwrap();
        assert!(Start::parse([OsString::from("--capture"), path.into_os_string()]).is_err());
    }
}
