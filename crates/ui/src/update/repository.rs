//! Guarded repository state transitions.
use crate::{transfer_worker, worker, Desktop, Message, Panel, Path, RepositorySnapshot};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    PathChanged(String),
    Open,
    Loaded(Result<Box<RepositorySnapshot>, String>),
    CloneSourceChanged(String),
    CloneDestinationChanged(String),
    Clone,
    Cloned(Result<std::path::PathBuf, String>),
    InitDestinationChanged(String),
    InitBranchChanged(String),
    Initialize,
    Initialized(Result<std::path::PathBuf, String>),
}

impl Desktop {
    pub(super) fn update_repository(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::PathChanged(path) if !self.loading => {
                self.selected_change = None;
                self.delete_branch_pending = None;
                self.path = path;
                self.snapshot = None;
                self.clear_history();
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
                self.merge_preview = None;
                self.rebase_preview = None;
                self.abort_merge_pending = false;
                self.abort_rebase_pending = false;
                self.detail.clear();
                self.notice.clear();
                self.summary.clear();
                self.description.clear();
                self.new_branch.clear();
                self.rename_branch.clear();
                self.target_branch = None;
                self.select_remote(None);
                self.remote_after_refresh = None;
                self.new_remote_name.clear();
                self.new_remote_url.clear();
                self.show_remote_settings = false;
                self.selected_stash = None;
                self.stash_message.clear();
                self.show_stashes = false;
                self.transfer_progress.clear();
            }
            Event::Open if !self.loading => {
                self.selected_change = None;
                self.delete_branch_pending = None;
                if self.snapshot.is_none() {
                    self.panel = Panel::Changes;
                    self.show_history = false;
                }
                self.loading = true;
                self.snapshot = None;
                self.target_branch = None;
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
                self.merge_preview = None;
                self.rebase_preview = None;
                self.abort_merge_pending = false;
                self.abort_rebase_pending = false;
                self.selected_stash = None;
                self.select_remote(None);
                self.remote_after_refresh = None;
                self.notice.clear();
                self.detail.clear();
                let path = self.path.clone();
                return worker(
                    move || desktop_git::snapshot(Path::new(&path)),
                    Message::Loaded,
                );
            }
            Event::Loaded(result) => {
                self.selected_change = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
                self.delete_branch_pending = None;
                self.loading = false;
                match result {
                    Ok(snapshot) => {
                        let snapshot = *snapshot;
                        if self.summary.trim().is_empty() {
                            if let Some(message) = &snapshot.merge_message {
                                self.summary = message
                                    .lines()
                                    .next()
                                    .unwrap_or("Merge branches")
                                    .to_owned();
                            }
                        }
                        self.detail = format!(
                            "{}\n\nRecent commits\n{}",
                            snapshot.status, snapshot.history
                        );
                        self.reset_history(&snapshot);
                        self.snapshot = Some(snapshot);
                    }
                    Err(error) => self.detail = format!("Unable to open repository: {error}"),
                }
            }
            Event::CloneSourceChanged(value) if !self.loading => self.clone_source = value,
            Event::CloneDestinationChanged(value) if !self.loading => {
                self.clone_destination = value
            }
            Event::Clone
                if !self.loading
                    && !self.clone_source.trim().is_empty()
                    && !self.clone_destination.trim().is_empty() =>
            {
                let source = self.clone_source.clone();
                let destination = self.clone_destination.clone();
                self.loading = true;
                self.notice = String::from("Cloning repository…");
                let control = self.begin_transfer();
                return transfer_worker(
                    control,
                    self.transfer_serial,
                    move |control| {
                        desktop_git::clone_repository_with_control(
                            &source,
                            Path::new(&destination),
                            control,
                        )
                    },
                    Message::Cloned,
                );
            }
            Event::Cloned(result) => {
                self.finish_transfer();
                self.loading = false;
                match result {
                    Ok(path) => {
                        self.clone_source.clear();
                        self.clone_destination.clear();
                        return self.open_created_repository(path, "Clone completed");
                    }
                    Err(error) => self.notice = format!("Clone failed: {error}"),
                }
            }
            Event::InitDestinationChanged(value) if !self.loading => self.init_destination = value,
            Event::InitBranchChanged(value) if !self.loading => self.init_branch = value,
            Event::Initialize if !self.loading && !self.init_destination.trim().is_empty() => {
                let destination = self.init_destination.clone();
                let branch = self.init_branch.clone();
                self.loading = true;
                self.notice = String::from("Initializing repository…");
                return worker(
                    move || {
                        desktop_git::init_repository_with_default_branch(
                            Path::new(&destination),
                            if branch.is_empty() {
                                None
                            } else {
                                Some(&branch)
                            },
                        )
                    },
                    Message::Initialized,
                );
            }
            Event::Initialized(result) => {
                self.loading = false;
                match result {
                    Ok(path) => {
                        self.init_destination.clear();
                        self.init_branch.clear();
                        self.transfer_progress.clear();
                        return self.open_created_repository(path, "Repository initialized");
                    }
                    Err(error) => self.notice = format!("Initialization failed: {error}"),
                }
            }
            Event::PathChanged(..)
            | Event::Open
            | Event::CloneSourceChanged(..)
            | Event::CloneDestinationChanged(..)
            | Event::Clone
            | Event::InitDestinationChanged(..)
            | Event::InitBranchChanged(..)
            | Event::Initialize => {}
        }
        Task::none()
    }
}

impl Desktop {
    pub(crate) fn open_created_repository(
        &mut self,
        path: std::path::PathBuf,
        notice: &str,
    ) -> Task<Message> {
        self.delete_branch_pending = None;
        self.panel = Panel::Changes;
        self.show_history = false;
        self.path = path.to_string_lossy().into_owned();
        self.snapshot = None;
        self.clear_history();
        self.conflict_review = None;
        self.side_resolution_review = None;
        self.hunk_review = None;
        self.selected_hunks.clear();
        self.selected_lines.clear();
        self.merge_preview = None;
        self.rebase_preview = None;
        self.abort_merge_pending = false;
        self.abort_rebase_pending = false;
        self.target_branch = None;
        self.select_remote(None);
        self.remote_after_refresh = None;
        self.new_remote_name.clear();
        self.new_remote_url.clear();
        self.show_remote_settings = false;
        self.selected_stash = None;
        self.stash_message.clear();
        self.show_stashes = false;
        self.summary.clear();
        self.description.clear();
        self.new_branch.clear();
        self.rename_branch.clear();
        self.detail.clear();
        self.notice = notice.to_owned();
        self.loading = true;
        worker(move || desktop_git::snapshot(&path), Message::Refreshed)
    }
}
