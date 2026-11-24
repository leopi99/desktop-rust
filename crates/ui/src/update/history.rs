//! Guarded history state transitions.
use crate::{
    worker, CommitDetails, CommitFile, CommitSummary, Desktop, Message, Panel, RepositorySnapshot,
};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    ShowHistory(bool),
    SelectCommit(CommitSummary),
    CommitLoaded(Result<CommitDetails, String>),
    SelectCommitFile(CommitFile),
    CommitDiffLoaded(Result<String, String>),
    LoadOlderCommits,
    CreateBranchAtCommit,
    OlderCommitsLoaded(Result<Vec<CommitSummary>, String>),
}

impl Desktop {
    pub(super) fn update_history(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::ShowHistory(show) if !self.loading => {
                self.delete_branch_pending = None;
                self.panel = if show { Panel::History } else { Panel::Changes };
                self.show_history = show;
                self.commit_details = None;
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.merge_preview = None;
                self.rebase_preview = None;
                self.detail = if show {
                    String::from("Select a commit to inspect its message and files")
                } else {
                    self.snapshot
                        .as_ref()
                        .map(|s| format!("{}\n\nRecent commits\n{}", s.status, s.history))
                        .unwrap_or_default()
                };
            }
            Event::SelectCommit(commit) if !self.loading && self.show_history => {
                if let Some(snapshot) = &self.snapshot {
                    if !self.history_commits.contains(&commit) {
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    self.loading = true;
                    self.commit_details = None;
                    self.detail = format!("Loading commit {}…", commit.oid);
                    return worker(
                        move || desktop_git::commit_details(&path, &commit.oid),
                        Message::CommitLoaded,
                    );
                }
            }
            Event::CommitLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(details) => {
                        self.detail = format!(
                            "Commit {}\nAuthor: {}\nDate: {}\nParents: {}\n\n{}",
                            details.commit.oid,
                            details.commit.author,
                            details.commit.authored_at,
                            details.parents.join(", "),
                            details.message
                        );
                        self.commit_details = Some(details);
                    }
                    Err(error) => {
                        self.commit_details = None;
                        self.detail = format!("Unable to read commit: {error}");
                    }
                }
            }
            Event::SelectCommitFile(file) if !self.loading && self.show_history => {
                if let (Some(snapshot), Some(details)) = (&self.snapshot, &self.commit_details) {
                    let path = snapshot.repository.path.clone();
                    let oid = details.commit.oid.clone();
                    self.loading = true;
                    self.detail = format!("Loading committed diff for {}…", file.path.display());
                    return worker(
                        move || desktop_git::commit_file_diff(&path, &oid, &file),
                        Message::CommitDiffLoaded,
                    );
                }
            }
            Event::CommitDiffLoaded(result) => {
                self.loading = false;
                self.detail = result
                    .unwrap_or_else(|error| format!("Unable to read committed diff: {error}"));
            }
            Event::CreateBranchAtCommit
                if !self.loading
                    && self.show_history
                    && self.panel == Panel::History
                    && !self.new_branch.is_empty() =>
            {
                if let (Some(snapshot), Some(details)) = (&self.snapshot, &self.commit_details) {
                    if snapshot.commit_basis.merge_heads.is_some()
                        || snapshot.commit_basis.rebase.is_some()
                        || !self.history_commits.contains(&details.commit)
                    {
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let oid = details.commit.oid.clone();
                    let name = self.new_branch.clone();
                    self.loading = true;
                    self.notice = format!("Creating branch {name} at {oid}…");
                    return worker(
                        move || desktop_git::create_branch_at_commit(&path, &name, &oid, &expected),
                        |result| Message::Wrote(crate::Operation::CreateBranchAtCommit, result),
                    );
                }
            }
            Event::LoadOlderCommits
                if !self.loading && self.show_history && self.history_has_more =>
            {
                if let (Some(snapshot), Some(head)) = (&self.snapshot, &self.history_head) {
                    let path = snapshot.repository.path.clone();
                    let head = head.clone();
                    let offset = self.history_commits.len();
                    self.loading = true;
                    self.notice = String::from("Loading older commits…");
                    return worker(
                        move || desktop_git::commit_history(&path, head.trim(), offset),
                        Message::OlderCommitsLoaded,
                    );
                }
            }
            Event::OlderCommitsLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(commits) => {
                        self.history_has_more = commits.len() == 100;
                        self.history_commits.extend(commits);
                        self.notice.clear();
                    }
                    Err(error) => self.notice = format!("Unable to load older commits: {error}"),
                }
            }
            Event::ShowHistory(..)
            | Event::SelectCommit(..)
            | Event::SelectCommitFile(..)
            | Event::CreateBranchAtCommit
            | Event::LoadOlderCommits => {}
        }
        Task::none()
    }
}

impl Desktop {
    pub(crate) fn clear_history(&mut self) {
        self.show_history = false;
        self.history_commits.clear();
        self.history_head = None;
        self.history_has_more = false;
        self.commit_details = None;
    }
}

impl Desktop {
    pub(crate) fn reset_history(&mut self, snapshot: &RepositorySnapshot) {
        self.history_head = snapshot.commit_basis.head.clone();
        self.history_commits = snapshot.commits.clone();
        self.history_has_more = snapshot.commits.len() == 100;
        self.commit_details = None;
        if self.show_history {
            self.detail = String::from("Select a commit to inspect its message and files");
        }
    }
}
