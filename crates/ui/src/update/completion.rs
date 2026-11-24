//! Guarded completion state transitions.
use crate::{worker, Desktop, Message, Operation, Panel, RemoteEdit, RepositorySnapshot};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    Wrote(Operation, Result<String, String>),
    Refreshed(Result<Box<RepositorySnapshot>, String>),
}

impl Desktop {
    pub(super) fn update_completion(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Wrote(operation, result) => {
                self.delete_branch_pending = None;
                if matches!(
                    operation,
                    Operation::Merge
                        | Operation::Rebase
                        | Operation::ContinueRebase
                        | Operation::AbortMerge
                        | Operation::AbortRebase
                        | Operation::ApplyStash
                        | Operation::CreateBranch
                        | Operation::SwitchBranch
                ) {
                    self.panel = Panel::Changes;
                    self.show_history = false;
                }
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.merge_preview = None;
                self.rebase_preview = None;
                self.abort_merge_pending = false;
                self.abort_rebase_pending = false;
                if matches!(operation, Operation::Remote) {
                    self.finish_transfer();
                }
                self.loading = false;
                self.notice = match result {
                    Ok(output) => {
                        if matches!(operation, Operation::Commit) {
                            self.summary.clear();
                            self.description.clear();
                        }
                        if matches!(operation, Operation::SaveStash) {
                            self.stash_message.clear();
                        }
                        if matches!(operation, Operation::RenameBranch) {
                            self.rename_branch.clear();
                        }
                        if matches!(
                            operation,
                            Operation::CreateBranch | Operation::CreateBranchAtCommit
                        ) {
                            self.new_branch.clear();
                        }
                        if let Operation::RemoteConfig(action) = operation {
                            self.remote_after_refresh = match action {
                                RemoteEdit::Add => Some(self.remote_name().into()),
                                RemoteEdit::Remove => None,
                                _ => self.selected_remote.as_ref().map(|r| r.name.clone()),
                            };
                            if action == RemoteEdit::Add {
                                self.new_remote_name.clear();
                                self.new_remote_url.clear();
                            }
                            if action == RemoteEdit::Remove {
                                self.select_remote(None);
                            }
                        }
                        if output.is_empty() {
                            String::from("Index updated")
                        } else {
                            output
                        }
                    }
                    Err(error) => format!("Operation failed: {error}"),
                };
                self.target_branch = None;
                self.selected_stash = None;
                self.remote_remove_pending = false;
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.loading = true;
                    return worker(move || desktop_git::snapshot(&path), Message::Refreshed);
                }
            }
            Event::Refreshed(result) => {
                self.side_resolution_review = None;
                self.delete_branch_pending = None;
                self.loading = false;
                match result {
                    Ok(snapshot) => {
                        let snapshot = *snapshot;
                        self.detail = format!(
                            "{}\n\nRecent commits\n{}",
                            snapshot.status, snapshot.history
                        );
                        if self.summary.trim().is_empty() {
                            if let Some(message) = &snapshot.merge_message {
                                self.summary = message
                                    .lines()
                                    .next()
                                    .unwrap_or("Merge branches")
                                    .to_owned();
                            }
                        }
                        if let Some(name) = self.remote_after_refresh.take() {
                            self.select_remote(
                                snapshot.remotes.iter().find(|r| r.name == name).cloned(),
                            );
                        } else {
                            self.selected_remote =
                                self.selected_remote.take().and_then(|previous| {
                                    snapshot
                                        .remotes
                                        .iter()
                                        .find(|r| {
                                            r.name == previous.name
                                                && r.fetch_urls == previous.fetch_urls
                                                && r.push_urls == previous.push_urls
                                        })
                                        .cloned()
                                });
                            if self.selected_remote.is_none() {
                                self.select_remote(None);
                            }
                        }
                        self.selected_stash = self.selected_stash.take().and_then(|selected| {
                            snapshot
                                .stashes
                                .iter()
                                .find(|stash| stash.oid == selected.oid)
                                .cloned()
                        });
                        self.reset_history(&snapshot);
                        self.snapshot = Some(snapshot);
                    }
                    Err(error) => {
                        self.select_remote(None);
                        self.remote_after_refresh = None;
                        self.snapshot = None;
                        self.detail = format!("Refresh failed: {error}");
                    }
                }
            }
        }
        Task::none()
    }
}
