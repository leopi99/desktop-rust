mod capture;
mod clip;
mod layout;

use desktop_core::{
    CommitDetails, CommitFile, CommitSummary, ConflictContent, ConflictReview, FileChange,
    FileDiff, LocalBranch, MergeKind, MergePreview, RebaseKind, RebasePreview, Remote,
    RemoteAction, RemoteUrlKind, RepositorySnapshot, StashEntry,
};
use desktop_git::TransferControl;
use iced::futures::SinkExt;
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length, Task};
use std::{path::Path, time::Duration};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Panel {
    #[default]
    Changes,
    History,
    Branches,
    Remotes,
    Stashes,
    Repository,
}

#[derive(Default)]
struct Desktop {
    panel: Panel,
    capture: Option<std::path::PathBuf>,
    capture_frames: u8,
    capture_success: std::sync::Arc<std::sync::atomic::AtomicBool>,
    path: String,
    snapshot: Option<RepositorySnapshot>,
    detail: String,
    show_history: bool,
    history_commits: Vec<CommitSummary>,
    history_head: Option<String>,
    history_has_more: bool,
    commit_details: Option<CommitDetails>,
    loading: bool,
    summary: String,
    description: String,
    notice: String,
    new_branch: String,
    rename_branch: String,
    target_branch: Option<LocalBranch>,
    merge_preview: Option<MergePreview>,
    abort_merge_pending: bool,
    rebase_preview: Option<RebasePreview>,
    abort_rebase_pending: bool,
    selected_remote: Option<Remote>,
    new_remote_name: String,
    new_remote_url: String,
    fetch_url: String,
    push_url: String,
    remote_remove_pending: bool,
    remote_after_refresh: Option<std::ffi::OsString>,
    show_remote_settings: bool,
    conflict_review: Option<ConflictReview>,
    selected_stash: Option<StashEntry>,
    stash_message: String,
    show_stashes: bool,
    clone_source: String,
    clone_destination: String,
    init_destination: String,
    init_branch: String,
    transfer: Option<TransferControl>,
    transfer_serial: u64,
    transfer_progress: String,
}

#[derive(Debug, Clone)]
enum Message {
    PanelSelected(Panel),
    CaptureFrame,
    Captured(iced::window::Screenshot),
    PathChanged(String),
    Open,
    Loaded(Result<RepositorySnapshot, String>),
    ShowHistory(bool),
    SelectCommit(CommitSummary),
    CommitLoaded(Result<CommitDetails, String>),
    SelectCommitFile(CommitFile),
    CommitDiffLoaded(Result<String, String>),
    LoadOlderCommits,
    OlderCommitsLoaded(Result<Vec<CommitSummary>, String>),
    Select(FileChange),
    DiffLoaded(Result<FileDiff, String>),
    ReviewConflict(FileChange),
    ConflictReviewed(Result<ConflictReview, String>),
    ResolveConflict,
    Stage(FileChange),
    Unstage(FileChange),
    SummaryChanged(String),
    DescriptionChanged(String),
    Commit,
    BranchNameChanged(String),
    RenameBranchNameChanged(String),
    RenameCurrentBranch,
    BranchSelected(LocalBranch),
    CreateBranch,
    SwitchBranch,
    PreviewMerge,
    MergePreviewed(Result<MergePreview, String>),
    MergeBranch,
    RequestAbortMerge,
    CancelAbortMerge,
    AbortMerge,
    PreviewRebase,
    RebasePreviewed(Result<RebasePreview, String>),
    RebaseBranch,
    ContinueRebase,
    RequestAbortRebase,
    CancelAbortRebase,
    AbortRebase,
    RemoteSelected(Remote),
    RemoteAction(RemoteAction),
    RemoteNameChanged(String),
    RemoteUrlChanged(String),
    FetchUrlChanged(String),
    PushUrlChanged(String),
    ToggleRemoteSettings,
    RequestRemoveRemote,
    CancelRemoveRemote,
    ConfigureRemote(RemoteEdit),
    ToggleStashes,
    StashSelected(StashEntry),
    StashMessageChanged(String),
    SaveStash,
    ApplyStash,
    CloneSourceChanged(String),
    CloneDestinationChanged(String),
    Clone,
    Cloned(Result<std::path::PathBuf, String>),
    InitDestinationChanged(String),
    InitBranchChanged(String),
    Initialize,
    Initialized(Result<std::path::PathBuf, String>),
    CancelTransfer,
    TransferProgress(u64, String),
    Wrote(Operation, Result<String, String>),
    Refreshed(Result<RepositorySnapshot, String>),
}

#[derive(Debug, Clone, Copy)]
enum Operation {
    Index,
    ResolveConflict,
    Commit,
    CreateBranch,
    RenameBranch,
    SwitchBranch,
    Remote,
    RemoteConfig(RemoteEdit),
    SaveStash,
    ApplyStash,
    Merge,
    AbortMerge,
    Rebase,
    ContinueRebase,
    AbortRebase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteEdit {
    Add,
    FetchUrl,
    PushUrl,
    Remove,
}

fn worker<T: Send + 'static>(
    job: impl FnOnce() -> std::io::Result<T> + Send + 'static,
    done: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let _ = sender.send(job().map_err(|e| e.to_string()));
            });
            receiver.await.unwrap_or_else(|e| Err(e.to_string()))
        },
        done,
    )
}

fn transfer_worker<T: Send + 'static>(
    control: TransferControl,
    serial: u64,
    job: impl FnOnce(&TransferControl) -> std::io::Result<T> + Send + 'static,
    done: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    let (mut events, stream) = iced::futures::channel::mpsc::channel(2);
    let (completion, result) = std::sync::mpsc::channel();
    let worker_control = control.clone();
    std::thread::spawn(move || {
        let _ = completion.send(job(&worker_control).map_err(|e| e.to_string()));
    });
    std::thread::spawn(move || loop {
        match result.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => {
                let _ = iced::futures::executor::block_on(events.send(done(result)));
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Err(error) =
                    events.try_send(Message::TransferProgress(serial, control.progress()))
                {
                    if error.is_disconnected() {
                        control.cancel();
                        break;
                    }
                    // A full queue drops this intermediate update; final results are awaited.
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let _ = iced::futures::executor::block_on(events.send(done(Err(String::from(
                    "Transfer worker stopped unexpectedly",
                )))));
                break;
            }
        }
    });
    Task::run(stream, std::convert::identity)
}

impl Desktop {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PanelSelected(panel) if !self.loading => {
                self.panel = panel;
                self.show_history = panel == Panel::History;
                self.show_stashes = panel == Panel::Stashes;
                self.show_remote_settings = panel == Panel::Remotes;
                self.abort_merge_pending = false;
                self.abort_rebase_pending = false;
                self.remote_remove_pending = false;
                if matches!(panel, Panel::Changes | Panel::History) {
                    return self.update(Message::ShowHistory(panel == Panel::History));
                }
                self.conflict_review = None;
                self.merge_preview = None;
                self.rebase_preview = None;
            }
            Message::CaptureFrame => {
                self.capture_frames += 1;
                if self.capture_frames == 4 {
                    return iced::window::get_latest()
                        .and_then(iced::window::screenshot)
                        .map(Message::Captured);
                }
            }
            Message::Captured(screenshot) => {
                if let Some(path) = self.capture.take() {
                    match capture::save(&path, &screenshot) {
                        Ok(()) => self
                            .capture_success
                            .store(true, std::sync::atomic::Ordering::SeqCst),
                        Err(error) => eprintln!("Capture failed: {error}"),
                    }
                }
                return iced::exit();
            }
            Message::ShowHistory(show) if !self.loading => {
                self.panel = if show { Panel::History } else { Panel::Changes };
                self.show_history = show;
                self.commit_details = None;
                self.conflict_review = None;
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
            Message::SelectCommit(commit) if !self.loading && self.show_history => {
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
            Message::CommitLoaded(result) => {
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
            Message::SelectCommitFile(file) if !self.loading && self.show_history => {
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
            Message::CommitDiffLoaded(result) => {
                self.loading = false;
                self.detail = result
                    .unwrap_or_else(|error| format!("Unable to read committed diff: {error}"));
            }
            Message::LoadOlderCommits
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
            Message::OlderCommitsLoaded(result) => {
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
            Message::ToggleStashes => self.show_stashes = !self.show_stashes,
            Message::StashSelected(stash) if !self.loading => self.selected_stash = Some(stash),
            Message::StashMessageChanged(value) if !self.loading => self.stash_message = value,
            Message::SaveStash if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let message = self.stash_message.clone();
                    self.loading = true;
                    self.notice = String::from("Saving tracked changes…");
                    return worker(
                        move || desktop_git::save_stash(&path, &message, &expected),
                        |result| Message::Wrote(Operation::SaveStash, result),
                    );
                }
            }
            Message::ApplyStash if !self.loading => {
                if let (Some(snapshot), Some(stash)) = (&self.snapshot, &self.selected_stash) {
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let stash = stash.clone();
                    self.loading = true;
                    self.notice = String::from("Restoring saved changes…");
                    return worker(
                        move || desktop_git::apply_stash(&path, &stash, &expected),
                        |result| Message::Wrote(Operation::ApplyStash, result),
                    );
                }
            }
            Message::CancelTransfer => {
                if let Some(control) = &self.transfer {
                    control.cancel();
                    self.notice = String::from("Cancellation requested… waiting for Git to stop");
                }
            }
            Message::TransferProgress(serial, output)
                if self.transfer.is_some() && serial == self.transfer_serial =>
            {
                self.transfer_progress = output.replace('\r', "\n");
            }
            Message::PathChanged(path) if !self.loading => {
                self.path = path;
                self.snapshot = None;
                self.clear_history();
                self.conflict_review = None;
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
            Message::InitDestinationChanged(value) if !self.loading => {
                self.init_destination = value
            }
            Message::InitBranchChanged(value) if !self.loading => self.init_branch = value,
            Message::RemoteSelected(remote) if !self.loading => self.select_remote(Some(remote)),
            Message::RemoteNameChanged(value) if !self.loading => self.new_remote_name = value,
            Message::RemoteUrlChanged(value) if !self.loading => self.new_remote_url = value,
            Message::FetchUrlChanged(value) if !self.loading => {
                self.fetch_url = value;
                self.remote_remove_pending = false;
            }
            Message::PushUrlChanged(value) if !self.loading => {
                self.push_url = value;
                self.remote_remove_pending = false;
            }
            Message::ToggleRemoteSettings => self.show_remote_settings = !self.show_remote_settings,
            Message::RequestRemoveRemote if !self.loading && self.selected_remote.is_some() => {
                self.remote_remove_pending = true
            }
            Message::CancelRemoveRemote => self.remote_remove_pending = false,
            Message::ConfigureRemote(action) if !self.loading => {
                return self.configure_remote(action)
            }
            Message::CloneSourceChanged(value) if !self.loading => self.clone_source = value,
            Message::CloneDestinationChanged(value) if !self.loading => {
                self.clone_destination = value
            }
            Message::BranchNameChanged(value) if !self.loading => self.new_branch = value,
            Message::RenameBranchNameChanged(value) if !self.loading => self.rename_branch = value,
            Message::BranchSelected(branch) if !self.loading => {
                self.target_branch = Some(branch);
                self.merge_preview = None;
                self.rebase_preview = None;
            }
            Message::SummaryChanged(value) if !self.loading => self.summary = value,
            Message::DescriptionChanged(value) if !self.loading => self.description = value,
            Message::Open if !self.loading => {
                if self.snapshot.is_none() {
                    self.panel = Panel::Changes;
                    self.show_history = false;
                }
                self.loading = true;
                self.snapshot = None;
                self.target_branch = None;
                self.conflict_review = None;
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
            Message::Loaded(result) => {
                self.loading = false;
                match result {
                    Ok(snapshot) => {
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
            Message::Select(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.loading = true;
                    self.conflict_review = None;
                    self.detail = format!("Loading {}…", change.path.display());
                    return worker(
                        move || desktop_git::diff(&path, &change),
                        Message::DiffLoaded,
                    );
                }
            }
            Message::DiffLoaded(result) => {
                self.loading = false;
                self.detail = match result {
                    Ok(diff) => format!(
                        "Staged changes\n{}\n\nWorking tree changes\n{}",
                        if diff.staged.is_empty() {
                            "(none)"
                        } else {
                            &diff.staged
                        },
                        if diff.unstaged.is_empty() {
                            "(none)"
                        } else {
                            &diff.unstaged
                        }
                    ),
                    Err(error) => format!("Unable to load diff: {error}"),
                };
            }
            Message::ReviewConflict(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.conflict_review = None;
                    self.loading = true;
                    self.detail = format!("Reviewing conflict: {}", change.path.display());
                    return worker(
                        move || desktop_git::inspect_conflict(&path, &change),
                        Message::ConflictReviewed,
                    );
                }
            }
            Message::ConflictReviewed(result) => {
                self.loading = false;
                match result {
                    Ok(review) => {
                        let preview = match &review.content {
                            ConflictContent::Deleted => String::from(
                                "The file is deleted locally. Resolving will stage its deletion.",
                            ),
                            ConflictContent::Text { bytes, .. } => {
                                let end = bytes.len().min(64 * 1024);
                                format!(
                                    "{}{}",
                                    String::from_utf8_lossy(&bytes[..end]),
                                    if end < bytes.len() {
                                        "\n[Preview truncated at 64 KiB]"
                                    } else {
                                        ""
                                    }
                                )
                            }
                        };
                        self.detail = format!("Conflict review: {}\nMarker-like lines remaining: {}\nEdit the file outside the app, then review again.\n\n{}",
                            review.change.path.display(), review.marker_count, preview);
                        self.conflict_review = Some(review);
                    }
                    Err(error) => {
                        self.conflict_review = None;
                        self.detail = format!("Unable to review conflict: {error}");
                    }
                }
            }
            Message::ResolveConflict if !self.loading => {
                if let Some(review) = &self.conflict_review {
                    if review.marker_count != 0 {
                        return Task::none();
                    }
                    let review = review.clone();
                    self.loading = true;
                    self.notice = String::from("Marking reviewed file as resolved…");
                    return worker(
                        move || desktop_git::resolve_conflict(&review),
                        |result| Message::Wrote(Operation::ResolveConflict, result),
                    );
                }
            }
            Message::Stage(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    return self.dispatch_write(snapshot.repository.path.clone(), change, true);
                }
            }
            Message::Unstage(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    return self.dispatch_write(snapshot.repository.path.clone(), change, false);
                }
            }
            Message::Commit if !self.loading && !self.summary.trim().is_empty() => {
                if let Some(snapshot) = &self.snapshot {
                    if snapshot.commit_basis.rebase.is_some() {
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let message = if self.description.trim().is_empty() {
                        self.summary.clone()
                    } else {
                        format!("{}\n\n{}", self.summary, self.description)
                    };
                    self.loading = true;
                    self.notice = String::from("Committing staged changes…");
                    return worker(
                        move || desktop_git::commit(&path, &message, &expected),
                        |result| Message::Wrote(Operation::Commit, result),
                    );
                }
            }
            Message::CreateBranch if !self.loading && !self.new_branch.is_empty() => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let name = self.new_branch.clone();
                    self.loading = true;
                    self.notice = String::from("Creating branch…");
                    return worker(
                        move || desktop_git::create_branch(&path, &name, &expected),
                        |result| Message::Wrote(Operation::CreateBranch, result),
                    );
                }
            }
            Message::RenameCurrentBranch if !self.loading && !self.rename_branch.is_empty() => {
                if let Some(snapshot) = &self.snapshot {
                    if snapshot.commit_basis.branch.is_none()
                        || snapshot.commit_basis.head.is_none()
                        || snapshot.commit_basis.merge_heads.is_some()
                        || snapshot.commit_basis.rebase.is_some()
                    {
                        self.notice = String::from(
                            "Select a committed local branch with no merge or rebase in progress",
                        );
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let name = self.rename_branch.clone();
                    self.loading = true;
                    self.notice = String::from("Renaming current branch…");
                    return worker(
                        move || desktop_git::rename_current_branch(&path, &name, &expected),
                        |result| Message::Wrote(Operation::RenameBranch, result),
                    );
                }
            }
            Message::SwitchBranch if !self.loading => {
                if let (Some(snapshot), Some(branch)) = (&self.snapshot, &self.target_branch) {
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let branch = branch.clone();
                    if branch.is_current {
                        return Task::none();
                    }
                    self.loading = true;
                    self.notice = String::from("Switching branch…");
                    return worker(
                        move || desktop_git::switch_branch(&path, &branch, &expected),
                        |result| Message::Wrote(Operation::SwitchBranch, result),
                    );
                }
            }
            Message::PreviewMerge if !self.loading => {
                if let (Some(snapshot), Some(target)) = (&self.snapshot, &self.target_branch) {
                    let path = snapshot.repository.path.clone();
                    let target = target.clone();
                    let basis = snapshot.commit_basis.clone();
                    self.loading = true;
                    self.merge_preview = None;
                    self.rebase_preview = None;
                    return worker(
                        move || desktop_git::preview_merge(&path, &target, &basis),
                        Message::MergePreviewed,
                    );
                }
            }
            Message::MergePreviewed(result) => {
                self.loading = false;
                match result {
                    Ok(preview) => {
                        let outcome = match preview.kind {
                            MergeKind::AlreadyUpToDate => "Already up to date",
                            MergeKind::FastForward => {
                                "Fast-forward: updates the current branch immediately"
                            }
                            MergeKind::CleanMerge => {
                                "Merge commit: stages the merge result for review before commit"
                            }
                            MergeKind::Conflicts => {
                                "Conflicts predicted: merge will pause for file resolution"
                            }
                        };
                        self.detail = format!(
                            "Merge {} into the current branch\n{}\n\n{}",
                            preview.target.name.to_string_lossy(),
                            outcome,
                            preview
                                .conflicts
                                .iter()
                                .map(|path| path.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        self.merge_preview = Some(preview);
                    }
                    Err(error) => {
                        self.merge_preview = None;
                        self.rebase_preview = None;
                        self.detail = format!("Unable to preview merge: {error}");
                    }
                }
            }
            Message::MergeBranch if !self.loading => {
                if let Some(preview) = &self.merge_preview {
                    let preview = preview.clone();
                    self.loading = true;
                    self.notice = String::from("Merging reviewed branch…");
                    return worker(
                        move || desktop_git::execute_merge(&preview),
                        |result| Message::Wrote(Operation::Merge, result),
                    );
                }
            }
            Message::RequestAbortMerge
                if !self.loading
                    && self
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.commit_basis.merge_heads.is_some()) =>
            {
                self.abort_merge_pending = true
            }
            Message::CancelAbortMerge => self.abort_merge_pending = false,
            Message::AbortMerge if !self.loading && self.abort_merge_pending => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    let basis = snapshot.commit_basis.clone();
                    self.abort_merge_pending = false;
                    self.abort_rebase_pending = false;
                    self.loading = true;
                    return worker(
                        move || desktop_git::abort_merge(&path, &basis),
                        |result| Message::Wrote(Operation::AbortMerge, result),
                    );
                }
            }
            Message::PreviewRebase if !self.loading => {
                if let (Some(snapshot), Some(target)) = (&self.snapshot, &self.target_branch) {
                    let path = snapshot.repository.path.clone();
                    let target = target.clone();
                    let basis = snapshot.commit_basis.clone();
                    self.loading = true;
                    self.merge_preview = None;
                    self.rebase_preview = None;
                    return worker(
                        move || desktop_git::preview_rebase(&path, &target, &basis),
                        Message::RebasePreviewed,
                    );
                }
            }
            Message::RebasePreviewed(result) => {
                self.loading = false;
                match result {
                    Ok(preview) => {
                        let outcome = match preview.kind { RebaseKind::AlreadyUpToDate => "Already based on this branch",
                            RebaseKind::FastForward => "Fast-forward: moves the current branch to the selected commit",
                            RebaseKind::Replay => "Replays local commits with new IDs; may pause for conflicts. Existing remote history is not force-pushed." };
                        self.detail = format!(
                            "Rebase onto {}\n{}\n{} commit(s) to replay\n\n{}{}",
                            preview.target.name.to_string_lossy(),
                            outcome,
                            preview.commits.len(),
                            preview
                                .commits
                                .iter()
                                .take(100)
                                .map(|commit| format!("{} {}", commit.oid, commit.subject))
                                .collect::<Vec<_>>()
                                .join("\n"),
                            if preview.commits.len() > 100 {
                                "\n[Additional commits omitted from preview]"
                            } else {
                                ""
                            }
                        );
                        self.rebase_preview = Some(preview);
                    }
                    Err(error) => {
                        self.rebase_preview = None;
                        self.detail = format!("Unable to preview rebase: {error}");
                    }
                }
            }
            Message::RebaseBranch if !self.loading => {
                if let Some(preview) = &self.rebase_preview {
                    let preview = preview.clone();
                    self.loading = true;
                    self.notice = String::from("Rebasing reviewed commits…");
                    return worker(
                        move || desktop_git::execute_rebase(&preview),
                        |result| Message::Wrote(Operation::Rebase, result),
                    );
                }
            }
            Message::ContinueRebase if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    if snapshot
                        .commit_basis
                        .rebase
                        .as_ref()
                        .is_none_or(|state| !state.supported)
                    {
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    self.loading = true;
                    self.notice = String::from("Continuing rebase…");
                    return worker(
                        move || desktop_git::continue_rebase(&path, &expected),
                        |result| Message::Wrote(Operation::ContinueRebase, result),
                    );
                }
            }
            Message::RequestAbortRebase
                if !self.loading
                    && self.snapshot.as_ref().is_some_and(|s| {
                        s.commit_basis.rebase.as_ref().is_some_and(|r| r.supported)
                    }) =>
            {
                self.abort_rebase_pending = true
            }
            Message::CancelAbortRebase => self.abort_rebase_pending = false,
            Message::AbortRebase if !self.loading && self.abort_rebase_pending => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    self.abort_rebase_pending = false;
                    self.loading = true;
                    self.notice = String::from("Aborting rebase…");
                    return worker(
                        move || desktop_git::abort_rebase(&path, &expected),
                        |result| Message::Wrote(Operation::AbortRebase, result),
                    );
                }
            }
            Message::RemoteAction(action) if !self.loading => {
                if let (Some(snapshot), Some(remote)) = (&self.snapshot, &self.selected_remote) {
                    let path = snapshot.repository.path.clone();
                    let remote = remote.clone();
                    let expected = snapshot.commit_basis.clone();
                    self.loading = true;
                    self.notice = format!("{action:?} in progress…");
                    let control = self.begin_transfer();
                    return transfer_worker(
                        control,
                        self.transfer_serial,
                        move |control| {
                            desktop_git::remote_operation_with_control(
                                &path, &remote, action, &expected, control,
                            )
                        },
                        |result| Message::Wrote(Operation::Remote, result),
                    );
                }
            }
            Message::Clone
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
            Message::Initialize if !self.loading && !self.init_destination.trim().is_empty() => {
                let destination = self.init_destination.clone();
                let branch = if self.init_branch.is_empty() {
                    String::from("main")
                } else {
                    self.init_branch.clone()
                };
                self.loading = true;
                self.notice = String::from("Initializing repository…");
                return worker(
                    move || desktop_git::init_repository(Path::new(&destination), &branch),
                    Message::Initialized,
                );
            }
            Message::Initialized(result) => {
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
            Message::Cloned(result) => {
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
            Message::Wrote(operation, result) => {
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
                        if matches!(operation, Operation::CreateBranch) {
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
            Message::Refreshed(result) => {
                self.loading = false;
                match result {
                    Ok(snapshot) => {
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
            _ => {}
        }
        Task::none()
    }

    fn clear_history(&mut self) {
        self.show_history = false;
        self.history_commits.clear();
        self.history_head = None;
        self.history_has_more = false;
        self.commit_details = None;
    }

    fn reset_history(&mut self, snapshot: &RepositorySnapshot) {
        self.history_head = snapshot.commit_basis.head.clone();
        self.history_commits = snapshot.commits.clone();
        self.history_has_more = snapshot.commits.len() == 100;
        self.commit_details = None;
        if self.show_history {
            self.detail = String::from("Select a commit to inspect its message and files");
        }
    }

    fn remote_name(&self) -> &str {
        if self.new_remote_name.is_empty() {
            "origin"
        } else {
            &self.new_remote_name
        }
    }

    fn select_remote(&mut self, remote: Option<Remote>) {
        self.fetch_url = remote
            .as_ref()
            .and_then(|r| r.fetch_urls.first())
            .and_then(|u| u.to_str())
            .unwrap_or_default()
            .to_owned();
        self.push_url = remote
            .as_ref()
            .and_then(|r| r.push_urls.first())
            .and_then(|u| u.to_str())
            .unwrap_or_default()
            .to_owned();
        self.selected_remote = remote;
        self.remote_remove_pending = false;
    }

    fn configure_remote(&mut self, action: RemoteEdit) -> Task<Message> {
        let Some(snapshot) = &self.snapshot else {
            return Task::none();
        };
        if action == RemoteEdit::Remove && !self.remote_remove_pending {
            return Task::none();
        }
        let remote = self.selected_remote.clone();
        if action != RemoteEdit::Add && remote.is_none() {
            return Task::none();
        }
        let name = self.remote_name().to_owned();
        let url = match action {
            RemoteEdit::Add => self.new_remote_url.clone(),
            RemoteEdit::FetchUrl => self.fetch_url.clone(),
            RemoteEdit::PushUrl => self.push_url.clone(),
            RemoteEdit::Remove => String::new(),
        };
        if action != RemoteEdit::Remove && url.trim().is_empty() {
            return Task::none();
        }
        let path = snapshot.repository.path.clone();
        self.remote_remove_pending = false;
        self.loading = true;
        self.notice = String::from("Updating remote configuration…");
        worker(
            move || {
                if action == RemoteEdit::Add {
                    return desktop_git::add_remote(&path, &name, &url);
                }
                let remote =
                    remote.ok_or_else(|| std::io::Error::other("Select a remote first"))?;
                match action {
                    RemoteEdit::FetchUrl => {
                        desktop_git::set_remote_url(&path, &remote, RemoteUrlKind::Fetch, &url)
                    }
                    RemoteEdit::PushUrl => {
                        desktop_git::set_remote_url(&path, &remote, RemoteUrlKind::Push, &url)
                    }
                    RemoteEdit::Remove => desktop_git::remove_remote(&path, &remote),
                    RemoteEdit::Add => unreachable!(),
                }
            },
            move |result| Message::Wrote(Operation::RemoteConfig(action), result),
        )
    }

    fn stash_view(&self) -> Element<'_, Message> {
        let Some(snapshot) = &self.snapshot else {
            return column![].into();
        };
        let mut controls =
            column![
                button(text(format!("Saved changes ({})", snapshot.stashes.len())))
                    .on_press(Message::ToggleStashes)
            ]
            .spacing(6);
        if self.show_stashes {
            let mut save = button("Save tracked changes");
            let mut apply = button("Restore (keep saved copy)");
            if !self.loading
                && snapshot.commit_basis.head.is_some()
                && snapshot.commit_basis.merge_heads.is_none()
                && snapshot.commit_basis.rebase.is_none()
            {
                if snapshot.changes.iter().any(|c| c.index != '?')
                    && !snapshot.changes.iter().any(FileChange::is_conflicted)
                {
                    save = save.on_press(Message::SaveStash);
                }
                if self.selected_stash.is_some() && snapshot.changes.is_empty() {
                    apply = apply.on_press(Message::ApplyStash);
                }
            }
            controls = controls.push(row![text_input("Saved changes description (optional)", &self.stash_message)
                .on_input(Message::StashMessageChanged), save].spacing(8))
                .push(row![pick_list(snapshot.stashes.as_slice(), self.selected_stash.as_ref(), Message::StashSelected)
                    .placeholder("Select saved changes"), apply].spacing(8))
                .push(text("New files are saved only if staged first. Restore requires a clean working tree."));
        }
        controls.into()
    }

    fn remote_settings_view(&self) -> Element<'_, Message> {
        let toggle = button(if self.show_remote_settings {
            "Hide remote settings"
        } else {
            "Remote settings"
        })
        .on_press(Message::ToggleRemoteSettings);
        let mut settings = column![toggle].spacing(8);
        if !self.show_remote_settings {
            return settings.into();
        }
        let mut add = button("Add remote");
        if !self.loading && !self.new_remote_url.trim().is_empty() {
            add = add.on_press(Message::ConfigureRemote(RemoteEdit::Add));
        }
        let mut form = column![row![
            text_input("Remote name (default: origin)", &self.new_remote_name)
                .on_input(Message::RemoteNameChanged),
            text_input("URL or local path", &self.new_remote_url)
                .on_input(Message::RemoteUrlChanged),
            add
        ]
        .spacing(8),]
        .spacing(8);
        if let Some(remote) = &self.selected_remote {
            let mut fetch = button("Save fetch URL");
            let mut push = button("Save push URL");
            let mut remove = button("Remove selected remote");
            if !self.loading {
                if remote.fetch_urls.len() == 1 && !self.fetch_url.trim().is_empty() {
                    fetch = fetch.on_press(Message::ConfigureRemote(RemoteEdit::FetchUrl));
                }
                if remote.push_urls.len() == 1 && !self.push_url.trim().is_empty() {
                    push = push.on_press(Message::ConfigureRemote(RemoteEdit::PushUrl));
                }
                remove = remove.on_press(Message::RequestRemoveRemote);
            }
            form = form
                .push(text(format!("Editing {}", remote.name.to_string_lossy())))
                .push(
                    row![
                        text_input("Fetch URL", &self.fetch_url).on_input(Message::FetchUrlChanged),
                        fetch
                    ]
                    .spacing(8),
                )
                .push(
                    row![
                        text_input("Push URL", &self.push_url).on_input(Message::PushUrlChanged),
                        push
                    ]
                    .spacing(8),
                );
            if remote.fetch_urls.len() != 1 || remote.push_urls.len() != 1 {
                form = form.push(text("Multi-URL endpoints must be edited using Git."));
            }
            if self.remote_remove_pending {
                let mut confirm = button("Confirm removal");
                if !self.loading {
                    confirm = confirm.on_press(Message::ConfigureRemote(RemoteEdit::Remove));
                }
                form = form.push(text(format!("Remove {} and its remote-tracking refs? Local branches and files are kept.", remote.name.to_string_lossy())))
                    .push(row![confirm, button("Cancel").on_press(Message::CancelRemoveRemote)].spacing(8));
            } else {
                form = form.push(remove);
            }
        }
        settings = settings.push(scrollable(form).height(Length::Fixed(210.0)));
        settings.into()
    }

    fn open_created_repository(&mut self, path: std::path::PathBuf, notice: &str) -> Task<Message> {
        self.panel = Panel::Changes;
        self.show_history = false;
        self.path = path.to_string_lossy().into_owned();
        self.snapshot = None;
        self.clear_history();
        self.conflict_review = None;
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

    fn begin_transfer(&mut self) -> TransferControl {
        let control = TransferControl::default();
        self.transfer = Some(control.clone());
        self.transfer_serial = self.transfer_serial.wrapping_add(1);
        self.transfer_progress.clear();
        control
    }

    fn finish_transfer(&mut self) {
        if let Some(control) = self.transfer.take() {
            self.transfer_progress = control.progress().replace('\r', "\n");
        }
    }

    fn dispatch_write(
        &mut self,
        path: std::path::PathBuf,
        change: FileChange,
        stage: bool,
    ) -> Task<Message> {
        self.loading = true;
        self.notice = String::from("Updating index…");
        worker(
            move || {
                if stage {
                    desktop_git::stage(&path, &change)?;
                } else {
                    desktop_git::unstage(&path, &change)?;
                }
                Ok(String::new())
            },
            |result| Message::Wrote(Operation::Index, result),
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(start) = capture::Start::parse(std::env::args_os().skip(1))? else {
        return Ok(());
    };
    let size = start.size;
    let capturing = start.capture.is_some();
    let app = start.initialize()?;
    let success = app.capture_success.clone();
    iced::application("Desktop Rust", Desktop::update, Desktop::view)
        .window(iced::window::Settings {
            size,
            min_size: Some(iced::Size::new(960.0, 640.0)),
            ..Default::default()
        })
        .subscription(|app: &Desktop| {
            if app.capture.is_some() && !app.loading && app.capture_frames < 4 {
                iced::window::frames().map(|_| Message::CaptureFrame)
            } else {
                iced::Subscription::none()
            }
        })
        .run_with(|| (app, Task::none()))?;
    if capturing && !success.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Window closed before a successful capture".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_core::{CommitBasis, Repository};

    fn loaded() -> Desktop {
        Desktop {
            path: String::from("/old"),
            summary: String::from("Summary"),
            description: String::from("Details"),
            snapshot: Some(RepositorySnapshot {
                repository: Repository {
                    path: "/old".into(),
                },
                history: String::new(),
                commits: Vec::new(),
                status: String::new(),
                changes: Vec::new(),
                branches: Vec::new(),
                remotes: Vec::new(),
                stashes: Vec::new(),
                merge_message: None,
                commit_basis: CommitBasis {
                    index: Vec::new(),
                    head: None,
                    branch: None,
                    merge_heads: None,
                    rebase: None,
                },
            }),
            ..Desktop::default()
        }
    }

    #[test]
    fn editing_repository_path_invalidates_write_target() {
        let mut app = loaded();
        let _ = app.update(Message::PathChanged(String::from("/new")));
        assert!(app.snapshot.is_none());
        assert!(app.summary.is_empty());
        let _ = app.update(Message::Commit);
        assert!(!app.loading);
    }

    #[test]
    fn failed_commit_preserves_draft_and_refreshes() {
        let mut app = loaded();
        let _ = app.update(Message::Wrote(
            Operation::Commit,
            Err(String::from("hook failed")),
        ));
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        assert!(app.notice.contains("hook failed"));
        assert!(app.loading);
        let _ = app.update(Message::Refreshed(Err(String::from("repository removed"))));
        assert!(app.snapshot.is_none());
        assert!(!app.loading);
        assert_eq!(app.summary, "Summary");
    }

    #[test]
    fn successful_commit_clears_draft_even_if_refresh_fails() {
        let mut app = loaded();
        let _ = app.update(Message::Wrote(
            Operation::Commit,
            Ok(String::from("commit created")),
        ));
        let _ = app.update(Message::Refreshed(Err(String::from("repository removed"))));
        assert!(app.summary.is_empty());
        assert!(app.description.is_empty());
        assert_eq!(app.notice, "commit created");
    }

    #[test]
    fn busy_operation_cannot_change_repository_or_draft() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::SummaryChanged(String::from("changed")));
        assert_eq!(app.path, "/old");
        assert_eq!(app.summary, "Summary");
    }
    fn target() -> LocalBranch {
        LocalBranch {
            name: "feature".into(),
            oid: "012345".into(),
            is_current: false,
        }
    }

    #[test]
    fn failed_branch_creation_preserves_both_drafts() {
        let mut app = loaded();
        app.new_branch = String::from("feature");
        app.target_branch = Some(target());
        let _ = app.update(Message::Wrote(
            Operation::CreateBranch,
            Err(String::from("already exists")),
        ));
        assert_eq!(app.new_branch, "feature");
        assert_eq!(app.summary, "Summary");
        assert!(app.target_branch.is_none());
        assert!(app.notice.contains("already exists"));
    }

    #[test]
    fn successful_branch_creation_clears_only_branch_name() {
        let mut app = loaded();
        app.new_branch = String::from("feature");
        let _ = app.update(Message::Wrote(
            Operation::CreateBranch,
            Ok(String::from("created")),
        ));
        assert!(app.new_branch.is_empty());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
    }

    #[test]
    fn busy_operation_ignores_branch_edits_and_actions() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::BranchNameChanged(String::from("new")));
        let _ = app.update(Message::RenameBranchNameChanged(String::from("renamed")));
        let _ = app.update(Message::RenameCurrentBranch);
        assert!(app.rename_branch.is_empty());
        let _ = app.update(Message::BranchSelected(target()));
        let _ = app.update(Message::CreateBranch);
        let _ = app.update(Message::SwitchBranch);
        assert!(app.new_branch.is_empty());
        assert!(app.target_branch.is_none());
        assert!(app.loading);
        assert!(app.notice.is_empty());
    }

    #[test]
    fn path_edit_clears_branch_selection_and_prevents_branch_writes() {
        let mut app = loaded();
        app.new_branch = String::from("feature");
        app.rename_branch = String::from("renamed");
        app.target_branch = Some(target());
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::RenameCurrentBranch);
        assert!(app.rename_branch.is_empty());
        let _ = app.update(Message::CreateBranch);
        let _ = app.update(Message::SwitchBranch);
        assert!(app.new_branch.is_empty());
        assert!(app.target_branch.is_none());
        assert!(app.snapshot.is_none());
        assert!(!app.loading);
    }
    fn remote() -> Remote {
        Remote {
            name: "origin".into(),
            fetch_urls: vec!["/local/remote".into()],
            push_urls: vec!["/local/remote".into()],
            target_branch: Some("refs/heads/main".into()),
        }
    }

    #[test]
    fn clone_failure_preserves_open_repository_and_clone_inputs() {
        let mut app = loaded();
        app.clone_source = String::from("source");
        app.clone_destination = String::from("destination");
        let _ = app.update(Message::Cloned(Err(String::from("permission denied"))));
        assert_eq!(app.path, "/old");
        assert!(app.snapshot.is_some());
        assert_eq!(app.clone_source, "source");
        assert_eq!(app.clone_destination, "destination");
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("permission denied"));
        assert!(!app.loading);
    }

    #[test]
    fn clone_success_opens_new_repository_and_clears_old_selections() {
        let mut app = loaded();
        app.clone_source = String::from("source");
        app.clone_destination = String::from("destination");
        app.selected_remote = Some(remote());
        let _ = app.update(Message::Cloned(Ok("/new-clone".into())));
        assert_eq!(app.path, "/new-clone");
        assert!(app.snapshot.is_none());
        assert!(app.selected_remote.is_none());
        assert!(app.summary.is_empty());
        assert!(app.clone_source.is_empty());
        assert!(app.clone_destination.is_empty());
        assert!(app.loading);
        let _ = app.update(Message::Refreshed(Err(String::from("cannot read clone"))));
        assert_eq!(app.notice, "Clone completed");
    }

    #[test]
    fn busy_remote_action_cannot_be_redirected_or_repeated() {
        let mut app = loaded();
        app.loading = true;
        app.selected_remote = Some(remote());
        let _ = app.update(Message::CloneSourceChanged(String::from("new-source")));
        let _ = app.update(Message::CloneDestinationChanged(String::from(
            "new-destination",
        )));
        let _ = app.update(Message::RemoteAction(RemoteAction::Push));
        let _ = app.update(Message::Clone);
        assert!(app.clone_source.is_empty());
        assert!(app.clone_destination.is_empty());
        assert!(app.notice.is_empty());
        assert!(app.loading);
    }

    #[test]
    fn repository_edit_invalidates_selected_remote() {
        let mut app = loaded();
        app.selected_remote = Some(remote());
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::RemoteAction(RemoteAction::Push));
        assert!(app.selected_remote.is_none());
        assert!(app.snapshot.is_none());
        assert!(!app.loading);
    }

    #[test]
    fn remote_failure_preserves_commit_draft_and_refreshes() {
        let mut app = loaded();
        app.selected_remote = Some(remote());
        let _ = app.update(Message::Wrote(
            Operation::Remote,
            Err(String::from("push rejected")),
        ));
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        assert!(app.notice.contains("push rejected"));
        assert!(app.loading);
    }
    #[test]
    fn changed_remote_endpoint_requires_new_selection_after_refresh() {
        let mut app = loaded();
        app.selected_remote = Some(remote());
        let mut snapshot = app.snapshot.clone().unwrap();
        let mut changed = remote();
        changed.push_urls = vec!["/different/remote".into()];
        snapshot.remotes.push(changed);
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert!(app.selected_remote.is_none());
        assert!(!app.loading);
    }
    #[test]
    fn cancel_keeps_ui_busy_until_the_process_result_arrives() {
        let mut app = loaded();
        app.loading = true;
        let _control = app.begin_transfer();
        let _ = app.update(Message::CancelTransfer);
        assert!(app.loading);
        assert!(app.transfer.is_some());
        assert!(app.notice.contains("Cancellation requested"));
        let _ = app.update(Message::Wrote(
            Operation::Remote,
            Err(String::from("Transfer cancelled")),
        ));
        assert!(app.transfer.is_none());
        assert!(app.loading); // Refresh still pending.
        assert_eq!(app.summary, "Summary");
        let snapshot = app.snapshot.clone().unwrap();
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert!(!app.loading);
    }

    #[test]
    fn late_progress_from_a_finished_transfer_is_ignored() {
        let mut app = loaded();
        let _ = app.begin_transfer();
        let old_serial = app.transfer_serial;
        app.finish_transfer();
        let _ = app.begin_transfer();
        let _ = app.update(Message::TransferProgress(old_serial, String::from("old")));
        assert!(app.transfer_progress.is_empty());
        let _ = app.update(Message::TransferProgress(
            app.transfer_serial,
            String::from("new\rprogress"),
        ));
        assert_eq!(app.transfer_progress, "new\nprogress");
        app.finish_transfer();
        let _ = app.update(Message::TransferProgress(
            app.transfer_serial,
            String::from("late"),
        ));
        assert!(!app.transfer_progress.contains("late"));
    }

    #[test]
    fn cancelled_clone_preserves_inputs_and_the_previous_repository() {
        let mut app = loaded();
        app.clone_source = String::from("source");
        app.clone_destination = String::from("destination");
        app.loading = true;
        let _ = app.begin_transfer();
        let _ = app.update(Message::CancelTransfer);
        let _ = app.update(Message::Cloned(Err(String::from("Transfer cancelled"))));
        assert!(app.transfer.is_none());
        assert!(!app.loading);
        assert_eq!(app.path, "/old");
        assert!(app.snapshot.is_some());
        assert_eq!(app.clone_source, "source");
        assert_eq!(app.clone_destination, "destination");
    }
    #[test]
    fn failed_initialization_preserves_form_and_open_repository() {
        let mut app = loaded();
        app.init_destination = String::from("/project");
        app.init_branch = String::from("develop");
        app.selected_remote = Some(remote());
        app.loading = true;
        let _ = app.update(Message::Initialized(Err(String::from(
            "already a repository",
        ))));
        assert_eq!(app.path, "/old");
        assert!(app.snapshot.is_some());
        assert!(app.selected_remote.is_some());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.init_destination, "/project");
        assert_eq!(app.init_branch, "develop");
        assert!(app.notice.contains("already a repository"));
        assert!(!app.loading);
    }

    #[test]
    fn initialized_repository_opens_and_clears_old_write_targets() {
        let mut app = loaded();
        app.init_destination = String::from("/project");
        app.init_branch = String::from("main");
        app.selected_remote = Some(remote());
        app.target_branch = Some(target());
        let _ = app.update(Message::Initialized(Ok("/project".into())));
        assert_eq!(app.path, "/project");
        assert!(app.snapshot.is_none());
        assert!(app.selected_remote.is_none());
        assert!(app.target_branch.is_none());
        assert!(app.summary.is_empty());
        assert!(app.init_destination.is_empty());
        assert!(app.init_branch.is_empty());
        assert!(app.loading);
        let _ = app.update(Message::Refreshed(Err(String::from(
            "cannot read new repository",
        ))));
        assert_eq!(app.notice, "Repository initialized");
        assert!(!app.loading);
    }

    #[test]
    fn busy_operation_ignores_initialization_form_and_submission() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::InitDestinationChanged(String::from("/project")));
        let _ = app.update(Message::InitBranchChanged(String::from("develop")));
        let _ = app.update(Message::Initialize);
        assert!(app.init_destination.is_empty());
        assert!(app.init_branch.is_empty());
        assert!(app.notice.is_empty());
        assert!(app.loading);
    }

    #[test]
    fn empty_initialization_destination_does_not_start_a_worker() {
        let mut app = loaded();
        app.init_destination = String::from("  ");
        let _ = app.update(Message::Initialize);
        assert!(!app.loading);
        assert!(app.snapshot.is_some());
        assert!(app.notice.is_empty());
    }
    #[test]
    fn selecting_a_remote_loads_its_urls_and_cancels_pending_removal() {
        let mut app = loaded();
        app.remote_remove_pending = true;
        let _ = app.update(Message::RemoteSelected(remote()));
        assert_eq!(app.fetch_url, "/local/remote");
        assert_eq!(app.push_url, "/local/remote");
        assert!(!app.remote_remove_pending);
        let _ = app.update(Message::RequestRemoveRemote);
        let mut other = remote();
        other.name = "upstream".into();
        let _ = app.update(Message::RemoteSelected(other));
        let _ = app.update(Message::ConfigureRemote(RemoteEdit::Remove));
        assert!(!app.loading);
        assert!(!app.remote_remove_pending);
    }

    #[test]
    fn remote_removal_requires_current_confirmation() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        let _ = app.update(Message::ConfigureRemote(RemoteEdit::Remove));
        assert!(!app.loading);
        let _ = app.update(Message::RequestRemoveRemote);
        assert!(app.remote_remove_pending);
        let _ = app.update(Message::CancelRemoveRemote);
        let _ = app.update(Message::ConfigureRemote(RemoteEdit::Remove));
        assert!(!app.loading);
        let _ = app.update(Message::RequestRemoveRemote);
        let _ = app.update(Message::ConfigureRemote(RemoteEdit::Remove));
        assert!(app.loading);
        assert!(!app.remote_remove_pending);
    }

    #[test]
    fn failed_url_save_preserves_the_edit_and_commit_drafts_after_refresh() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        app.fetch_url = String::from("/draft/url");
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.remotes.push(remote());
        let _ = app.update(Message::Wrote(
            Operation::RemoteConfig(RemoteEdit::FetchUrl),
            Err(String::from("config locked")),
        ));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert_eq!(app.fetch_url, "/draft/url");
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("config locked"));
        assert!(app.selected_remote.is_some());
    }

    #[test]
    fn adding_remote_selects_it_after_refresh_and_clears_the_form() {
        let mut app = loaded();
        app.new_remote_url = String::from("/local/remote");
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.remotes.push(remote());
        let _ = app.update(Message::Wrote(
            Operation::RemoteConfig(RemoteEdit::Add),
            Ok(String::from("Added origin")),
        ));
        assert!(app.new_remote_name.is_empty());
        assert!(app.new_remote_url.is_empty());
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert_eq!(app.selected_remote.unwrap().name, "origin");
        assert_eq!(app.fetch_url, "/local/remote");
        assert_eq!(app.summary, "Summary");
    }

    #[test]
    fn successful_url_save_selects_the_new_endpoint_after_refresh() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        let mut updated = remote();
        updated.push_urls = vec!["/new/push".into()];
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.remotes.push(updated);
        let _ = app.update(Message::Wrote(
            Operation::RemoteConfig(RemoteEdit::PushUrl),
            Ok(String::from("URL updated")),
        ));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert!(app.selected_remote.is_some());
        assert_eq!(app.push_url, "/new/push");
        assert_eq!(app.fetch_url, "/local/remote");
    }

    #[test]
    fn removed_remote_is_no_longer_available_for_push() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        let snapshot = app.snapshot.clone().unwrap();
        let _ = app.update(Message::Wrote(
            Operation::RemoteConfig(RemoteEdit::Remove),
            Ok(String::from("Removed origin")),
        ));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        let _ = app.update(Message::RemoteAction(RemoteAction::Push));
        assert!(app.selected_remote.is_none());
        assert!(app.fetch_url.is_empty());
        assert!(app.push_url.is_empty());
        assert!(!app.loading);
        assert_eq!(app.summary, "Summary");
    }

    #[test]
    fn busy_operation_ignores_remote_settings_edits_and_actions() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        app.loading = true;
        let _ = app.update(Message::RemoteNameChanged(String::from("new")));
        let _ = app.update(Message::RemoteUrlChanged(String::from("/new")));
        let _ = app.update(Message::FetchUrlChanged(String::from("/new")));
        let _ = app.update(Message::ConfigureRemote(RemoteEdit::Add));
        let _ = app.update(Message::RequestRemoveRemote);
        assert!(app.new_remote_name.is_empty());
        assert!(app.new_remote_url.is_empty());
        assert_eq!(app.fetch_url, "/local/remote");
        assert!(!app.remote_remove_pending);
        assert!(app.notice.is_empty());
    }

    #[test]
    fn repository_edit_clears_remote_settings_and_drafts() {
        let mut app = loaded();
        app.select_remote(Some(remote()));
        app.new_remote_name = String::from("upstream");
        app.new_remote_url = String::from("/draft");
        app.remote_remove_pending = true;
        app.show_remote_settings = true;
        let _ = app.update(Message::PathChanged(String::from("/new")));
        assert!(app.selected_remote.is_none());
        assert!(app.new_remote_url.is_empty());
        assert!(app.new_remote_name.is_empty());
        assert!(app.fetch_url.is_empty());
        assert!(!app.remote_remove_pending);
        assert!(!app.show_remote_settings);
    }
    fn saved_entry() -> StashEntry {
        StashEntry {
            oid: "0123456789abcdef0123456789abcdef01234567".into(),
            subject: "On main: saved".into(),
        }
    }

    #[test]
    fn successful_stash_save_clears_its_note_but_preserves_commit_draft() {
        let mut app = loaded();
        app.stash_message = String::from("work in progress");
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.stashes.push(saved_entry());
        let _ = app.update(Message::Wrote(
            Operation::SaveStash,
            Ok(String::from("saved")),
        ));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert!(app.stash_message.is_empty());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.snapshot.unwrap().stashes.len(), 1);
        assert!(!app.loading);
    }

    #[test]
    fn failed_stash_save_keeps_both_user_drafts() {
        let mut app = loaded();
        app.stash_message = String::from("work in progress");
        let _ = app.update(Message::Wrote(
            Operation::SaveStash,
            Err(String::from("index locked")),
        ));
        assert_eq!(app.stash_message, "work in progress");
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("index locked"));
        assert!(app.loading);
    }

    #[test]
    fn failed_stash_restore_keeps_the_saved_entry_visible_after_refresh() {
        let mut app = loaded();
        app.selected_stash = Some(saved_entry());
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.stashes.push(saved_entry());
        let _ = app.update(Message::Wrote(
            Operation::ApplyStash,
            Err(String::from("conflict")),
        ));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert!(app.selected_stash.is_none());
        assert_eq!(app.snapshot.as_ref().unwrap().stashes.len(), 1);
        assert!(app.notice.contains("conflict"));
        assert_eq!(app.summary, "Summary");
    }

    #[test]
    fn changing_repository_invalidates_stash_selection_and_draft() {
        let mut app = loaded();
        app.selected_stash = Some(saved_entry());
        app.stash_message = String::from("draft");
        app.show_stashes = true;
        let _ = app.update(Message::PathChanged(String::from("/other")));
        let _ = app.update(Message::SaveStash);
        let _ = app.update(Message::ApplyStash);
        assert!(app.selected_stash.is_none());
        assert!(app.stash_message.is_empty());
        assert!(!app.show_stashes);
        assert!(!app.loading);
    }

    #[test]
    fn busy_operations_ignore_stash_edits_and_actions() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::StashMessageChanged(String::from("new")));
        let _ = app.update(Message::StashSelected(saved_entry()));
        let _ = app.update(Message::SaveStash);
        let _ = app.update(Message::ApplyStash);
        assert!(app.selected_stash.is_none());
        assert!(app.stash_message.is_empty());
        assert!(app.notice.is_empty());
    }

    #[test]
    fn removed_stash_is_deselected_after_refresh() {
        let mut app = loaded();
        app.selected_stash = Some(saved_entry());
        let snapshot = app.snapshot.clone().unwrap();
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        let _ = app.update(Message::ApplyStash);
        assert!(app.selected_stash.is_none());
        assert!(!app.loading);
    }
    fn reviewed_conflict(markers: usize) -> ConflictReview {
        let app = loaded();
        let snapshot = app.snapshot.unwrap();
        ConflictReview {
            repository: snapshot.repository,
            change: FileChange {
                path: "file".into(),
                original_path: None,
                index: 'U',
                worktree: 'U',
            },
            basis: snapshot.commit_basis,
            content: ConflictContent::Text {
                bytes: b"reviewed text".to_vec(),
                executable: false,
            },
            marker_size: 7,
            marker_count: markers,
        }
    }

    #[test]
    fn unresolved_markers_block_the_resolve_action() {
        let mut app = loaded();
        let _ = app.update(Message::ConflictReviewed(Ok(reviewed_conflict(3))));
        let _ = app.update(Message::ResolveConflict);
        assert!(!app.loading);
        assert!(app.notice.is_empty());
        assert!(app.detail.contains("Marker-like lines remaining: 3"));
    }

    #[test]
    fn cleaned_review_can_be_staged_and_is_invalidated_after_completion() {
        let mut app = loaded();
        let _ = app.update(Message::ConflictReviewed(Ok(reviewed_conflict(0))));
        let _ = app.update(Message::ResolveConflict);
        assert!(app.loading);
        let _ = app.update(Message::Wrote(
            Operation::ResolveConflict,
            Ok(String::from("Marked resolved")),
        ));
        assert!(app.conflict_review.is_none());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.notice, "Marked resolved");
        assert!(app.loading); // Refresh pending.
    }

    #[test]
    fn rejected_resolution_requires_review_again_and_keeps_commit_draft() {
        let mut app = loaded();
        app.conflict_review = Some(reviewed_conflict(0));
        let _ = app.update(Message::Wrote(
            Operation::ResolveConflict,
            Err(String::from("File changed; review again")),
        ));
        assert!(app.conflict_review.is_none());
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("File changed"));
    }

    #[test]
    fn changing_repository_invalidates_conflict_review() {
        let mut app = loaded();
        app.conflict_review = Some(reviewed_conflict(0));
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::ResolveConflict);
        assert!(app.conflict_review.is_none());
        assert!(!app.loading);
    }

    #[test]
    fn busy_operation_cannot_resolve_a_reviewed_conflict() {
        let mut app = loaded();
        app.conflict_review = Some(reviewed_conflict(0));
        app.loading = true;
        let _ = app.update(Message::ResolveConflict);
        let _ = app.update(Message::ReviewConflict(
            app.conflict_review.as_ref().unwrap().change.clone(),
        ));
        assert!(app.notice.is_empty());
        assert!(app.loading);
    }

    #[test]
    fn failed_conflict_review_does_not_leave_an_older_review_actionable() {
        let mut app = loaded();
        app.conflict_review = Some(reviewed_conflict(0));
        let _ = app.update(Message::ConflictReviewed(Err(String::from("binary file"))));
        let _ = app.update(Message::ResolveConflict);
        assert!(app.conflict_review.is_none());
        assert!(app.detail.contains("binary file"));
        assert!(!app.loading);
    }

    #[test]
    fn conflict_preview_is_bounded_without_losing_the_full_reviewed_content() {
        let mut app = loaded();
        let mut review = reviewed_conflict(0);
        review.content = ConflictContent::Text {
            bytes: vec![b'x'; 70 * 1024],
            executable: false,
        };
        let _ = app.update(Message::ConflictReviewed(Ok(review)));
        assert!(app.detail.len() < 66 * 1024);
        assert!(app.detail.contains("Preview truncated"));
        match app.conflict_review.unwrap().content {
            ConflictContent::Text { bytes, .. } => assert_eq!(bytes.len(), 70 * 1024),
            _ => panic!("expected text review"),
        }
    }
    fn preview(kind: MergeKind) -> MergePreview {
        let app = loaded();
        let snapshot = app.snapshot.unwrap();
        MergePreview {
            repository: snapshot.repository,
            target: target(),
            basis: snapshot.commit_basis,
            kind,
            tree_oid: None,
            conflicts: Vec::new(),
        }
    }

    fn pending_merge() -> Desktop {
        let mut app = loaded();
        let snapshot = app.snapshot.as_mut().unwrap();
        snapshot.commit_basis.merge_heads =
            Some(b"0123456789abcdef0123456789abcdef01234567\n".to_vec());
        snapshot.merge_message = Some(String::from("Merge feature into main\n"));
        app
    }

    #[test]
    fn selecting_another_branch_invalidates_the_merge_preview() {
        let mut app = loaded();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        let _ = app.update(Message::BranchSelected(target()));
        let _ = app.update(Message::MergeBranch);
        assert!(app.merge_preview.is_none());
        assert!(!app.loading);
    }

    #[test]
    fn merge_failure_clears_preview_and_preserves_user_commit_draft() {
        let mut app = loaded();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        let _ = app.update(Message::Wrote(
            Operation::Merge,
            Err(String::from("target changed")),
        ));
        assert!(app.merge_preview.is_none());
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("target changed"));
    }

    #[test]
    fn merge_preview_explains_conflicts_without_starting_the_merge() {
        let mut app = loaded();
        let mut result = preview(MergeKind::Conflicts);
        result.conflicts.push("file".into());
        let _ = app.update(Message::MergePreviewed(Ok(result)));
        assert!(!app.loading);
        assert!(app.notice.is_empty());
        assert!(app.detail.contains("Conflicts predicted"));
        assert!(app.detail.contains("file"));
        assert!(app.merge_preview.is_some());
    }

    #[test]
    fn abort_requires_confirmation_and_can_be_cancelled() {
        let mut app = pending_merge();
        let _ = app.update(Message::AbortMerge);
        assert!(!app.loading);
        let _ = app.update(Message::RequestAbortMerge);
        assert!(app.abort_merge_pending);
        let _ = app.update(Message::CancelAbortMerge);
        let _ = app.update(Message::AbortMerge);
        assert!(!app.loading);
        let _ = app.update(Message::RequestAbortMerge);
        let _ = app.update(Message::AbortMerge);
        assert!(app.loading);
        assert!(!app.abort_merge_pending);
    }

    #[test]
    fn merge_message_prefills_only_an_empty_commit_draft() {
        let mut app = pending_merge();
        let snapshot = app.snapshot.clone().unwrap();
        app.summary.clear();
        let _ = app.update(Message::Refreshed(Ok(snapshot.clone())));
        assert_eq!(app.summary, "Merge feature into main");
        let _ = app.update(Message::SummaryChanged(String::from("My merge title")));
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert_eq!(app.summary, "My merge title");
        let _ = app.update(Message::Commit);
        assert!(app.loading); // Empty-tree merge still permits completion.
    }

    #[test]
    fn changing_repository_clears_merge_preview_and_abort_confirmation() {
        let mut app = pending_merge();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        app.abort_merge_pending = true;
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::MergeBranch);
        let _ = app.update(Message::AbortMerge);
        assert!(app.merge_preview.is_none());
        assert!(!app.abort_merge_pending);
        assert!(!app.loading);
    }

    #[test]
    fn busy_operations_ignore_merge_and_abort_actions() {
        let mut app = pending_merge();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        app.loading = true;
        let _ = app.update(Message::PreviewMerge);
        let _ = app.update(Message::MergeBranch);
        let _ = app.update(Message::RequestAbortMerge);
        assert!(app.merge_preview.is_some());
        assert!(!app.abort_merge_pending);
        assert!(app.notice.is_empty());
    }
    fn reviewed_rebase() -> RebasePreview {
        let app = loaded();
        let snapshot = app.snapshot.unwrap();
        RebasePreview {
            repository: snapshot.repository,
            target: target(),
            basis: snapshot.commit_basis,
            kind: RebaseKind::Replay,
            commits: vec![desktop_core::RebaseCommit {
                oid: "0123456789abcdef0123456789abcdef01234567".into(),
                subject: "Local work".into(),
            }],
        }
    }

    fn pending_rebase() -> Desktop {
        let mut app = loaded();
        let basis = &mut app.snapshot.as_mut().unwrap().commit_basis;
        basis.branch = None;
        basis.rebase = Some(desktop_core::RebaseState {
            branch: "refs/heads/main".into(),
            original_head: "0123456789abcdef0123456789abcdef01234567".into(),
            onto: "abcdef0123456789abcdef0123456789abcdef0123".into(),
            step: 1,
            total: 2,
            supported: true,
            commits: Vec::new(),
            metadata: Vec::new(),
        });
        app
    }

    #[test]
    fn rebase_preview_lists_rewritten_commits_without_starting_the_operation() {
        let mut app = loaded();
        let _ = app.update(Message::RebasePreviewed(Ok(reviewed_rebase())));
        assert!(!app.loading);
        assert!(app.notice.is_empty());
        assert!(app.detail.contains("Local work"));
        assert!(app.detail.contains("new IDs"));
        assert!(app.rebase_preview.is_some());
        let _ = app.update(Message::BranchSelected(target()));
        let _ = app.update(Message::RebaseBranch);
        assert!(app.rebase_preview.is_none());
        assert!(!app.loading);
    }

    #[test]
    fn pending_rebase_uses_continue_instead_of_the_regular_commit_action() {
        let mut app = pending_rebase();
        let _ = app.update(Message::Commit);
        assert!(!app.loading);
        let _ = app.update(Message::ContinueRebase);
        assert!(app.loading);
        let _ = app.update(Message::Wrote(
            Operation::ContinueRebase,
            Err(String::from("conflicts remain")),
        ));
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("conflicts remain"));
    }

    #[test]
    fn abort_rebase_requires_confirmation_and_can_be_cancelled() {
        let mut app = pending_rebase();
        let _ = app.update(Message::AbortRebase);
        assert!(!app.loading);
        let _ = app.update(Message::RequestAbortRebase);
        assert!(app.abort_rebase_pending);
        let _ = app.update(Message::CancelAbortRebase);
        let _ = app.update(Message::AbortRebase);
        assert!(!app.loading);
        let _ = app.update(Message::RequestAbortRebase);
        let _ = app.update(Message::AbortRebase);
        assert!(app.loading);
        assert!(!app.abort_rebase_pending);
    }

    #[test]
    fn unsupported_rebase_cannot_be_continued_or_aborted_from_ui() {
        let mut app = pending_rebase();
        app.snapshot
            .as_mut()
            .unwrap()
            .commit_basis
            .rebase
            .as_mut()
            .unwrap()
            .supported = false;
        let _ = app.update(Message::ContinueRebase);
        let _ = app.update(Message::RequestAbortRebase);
        let _ = app.update(Message::AbortRebase);
        assert!(!app.loading);
        assert!(!app.abort_rebase_pending);
    }

    #[test]
    fn changing_repository_clears_rebase_preview_and_abort_confirmation() {
        let mut app = pending_rebase();
        app.rebase_preview = Some(reviewed_rebase());
        app.abort_rebase_pending = true;
        let _ = app.update(Message::PathChanged(String::from("/new")));
        let _ = app.update(Message::RebaseBranch);
        let _ = app.update(Message::AbortRebase);
        assert!(app.rebase_preview.is_none());
        assert!(!app.abort_rebase_pending);
        assert!(!app.loading);
    }

    #[test]
    fn busy_operations_ignore_rebase_actions() {
        let mut app = pending_rebase();
        app.loading = true;
        app.rebase_preview = Some(reviewed_rebase());
        let _ = app.update(Message::PreviewRebase);
        let _ = app.update(Message::RebaseBranch);
        let _ = app.update(Message::ContinueRebase);
        let _ = app.update(Message::RequestAbortRebase);
        assert!(app.notice.is_empty());
        assert!(!app.abort_rebase_pending);
        assert!(app.rebase_preview.is_some());
    }
    fn history_commit(number: usize) -> CommitSummary {
        CommitSummary {
            oid: format!("{number:040x}"),
            subject: format!("Commit {number}"),
            author: "Test Author".into(),
            authored_at: "2026-11-24T12:00:00+01:00".into(),
        }
    }

    fn history_app() -> Desktop {
        let mut app = loaded();
        let snapshot = app.snapshot.as_mut().unwrap();
        snapshot.commits = vec![history_commit(1)];
        snapshot.commit_basis.head = Some(format!("{}\n", snapshot.commits[0].oid));
        let snapshot = snapshot.clone();
        app.reset_history(&snapshot);
        let _ = app.update(Message::ShowHistory(true));
        app
    }

    #[test]
    fn history_navigation_preserves_drafts_and_invalidates_hidden_write_previews() {
        let mut app = history_app();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        let _ = app.update(Message::ShowHistory(true));
        assert!(app.merge_preview.is_none());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        let _ = app.update(Message::ShowHistory(false));
        assert!(!app.show_history);
        assert_eq!(app.summary, "Summary");
    }

    #[test]
    fn history_refresh_reanchors_the_list_and_invalidates_selected_commit() {
        let mut app = history_app();
        app.commit_details = Some(CommitDetails {
            commit: history_commit(1),
            parents: Vec::new(),
            message: "body".into(),
            files: Vec::new(),
        });
        let mut snapshot = app.snapshot.clone().unwrap();
        snapshot.commits = vec![history_commit(2)];
        snapshot.commit_basis.head = Some(history_commit(2).oid);
        let _ = app.update(Message::Refreshed(Ok(snapshot)));
        assert_eq!(app.history_commits, vec![history_commit(2)]);
        assert_eq!(app.history_head, Some(history_commit(2).oid));
        assert!(app.commit_details.is_none());
        assert!(!app.history_has_more);
    }

    #[test]
    fn older_page_appends_without_moving_the_anchor() {
        let mut app = history_app();
        app.history_commits = (0..100).map(history_commit).collect();
        app.history_has_more = true;
        let anchor = app.history_head.clone();
        let _ = app.update(Message::OlderCommitsLoaded(Ok(vec![
            history_commit(100),
            history_commit(101),
        ])));
        assert_eq!(app.history_commits.len(), 102);
        assert_eq!(app.history_head, anchor);
        assert!(!app.history_has_more);
    }

    #[test]
    fn failed_older_page_preserves_entries_and_allows_retry() {
        let mut app = history_app();
        app.history_has_more = true;
        let before = app.history_commits.clone();
        let _ = app.update(Message::OlderCommitsLoaded(Err(String::from(
            "object unavailable",
        ))));
        assert_eq!(app.history_commits, before);
        assert!(app.history_has_more);
        assert!(app.notice.contains("object unavailable"));
    }

    #[test]
    fn repository_change_clears_history_cursor_and_prevents_stale_selection() {
        let mut app = history_app();
        let _ = app.update(Message::PathChanged("/new".into()));
        let _ = app.update(Message::SelectCommit(history_commit(1)));
        assert!(app.history_commits.is_empty());
        assert!(app.history_head.is_none());
        assert!(app.commit_details.is_none());
        assert!(!app.show_history);
        assert!(!app.loading);
    }

    #[test]
    fn busy_operation_ignores_history_navigation_and_selection() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::ShowHistory(true));
        let _ = app.update(Message::SelectCommit(history_commit(1)));
        assert!(!app.show_history);
        assert!(app.detail.is_empty());
        assert!(app.loading);
    }

    #[test]
    fn failed_commit_load_clears_old_files_without_discarding_the_history() {
        let mut app = history_app();
        let before = app.history_commits.clone();
        app.commit_details = Some(CommitDetails {
            commit: history_commit(1),
            parents: Vec::new(),
            message: "old".into(),
            files: Vec::new(),
        });
        let _ = app.update(Message::CommitLoaded(Err(String::from(
            "object unavailable",
        ))));
        assert!(app.commit_details.is_none());
        assert_eq!(app.history_commits, before);
        assert!(app.detail.contains("object unavailable"));
    }
    #[test]
    fn panel_navigation_preserves_drafts_and_revokes_hidden_confirmations() {
        let mut app = loaded();
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        app.rebase_preview = Some(reviewed_rebase());
        app.remote_remove_pending = true;
        app.abort_merge_pending = true;
        app.abort_rebase_pending = true;
        app.show_history = true;
        let _ = app.update(Message::PanelSelected(Panel::Remotes));
        assert!(!app.show_history);
        assert_eq!(app.panel, Panel::Remotes);
        assert!(app.show_remote_settings);
        assert!(app.merge_preview.is_none());
        assert!(app.rebase_preview.is_none());
        assert!(!app.remote_remove_pending);
        assert!(!app.abort_merge_pending);
        assert!(!app.abort_rebase_pending);
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
    }

    #[test]
    fn busy_operation_keeps_the_current_panel() {
        let mut app = loaded();
        app.loading = true;
        let _ = app.update(Message::PanelSelected(Panel::Repository));
        assert_eq!(app.panel, Panel::Changes);
    }

    #[test]
    fn conflicted_operation_returns_to_changes_without_discarding_drafts() {
        let mut app = loaded();
        app.panel = Panel::Branches;
        app.show_history = true;
        let _ = app.update(Message::Wrote(
            Operation::Rebase,
            Err("Resolve conflicts".into()),
        ));
        assert_eq!(app.panel, Panel::Changes);
        assert!(!app.show_history);
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("Resolve conflicts"));
    }
    #[test]
    fn refresh_preserves_history_panel_and_drafts() {
        let mut app = history_app();
        let _ = app.update(Message::Open);
        assert_eq!(app.panel, Panel::History);
        assert!(app.show_history);
        assert!(app.loading);
        assert_eq!(app.summary, "Summary");
    }
    #[test]
    fn failed_rename_preserves_drafts_and_refreshes_on_the_branches_panel() {
        let mut app = loaded();
        app.panel = Panel::Branches;
        app.rename_branch = "renamed".into();
        app.new_branch = "creation draft".into();
        let _ = app.update(Message::Wrote(
            Operation::RenameBranch,
            Err("Destination exists".into()),
        ));
        assert!(app.loading);
        assert_eq!(app.panel, Panel::Branches);
        assert_eq!(app.rename_branch, "renamed");
        assert_eq!(app.new_branch, "creation draft");
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("Destination exists"));
    }

    #[test]
    fn successful_rename_clears_only_its_draft_and_old_write_previews() {
        let mut app = loaded();
        app.panel = Panel::Branches;
        app.rename_branch = "renamed".into();
        app.new_branch = "creation draft".into();
        app.target_branch = Some(target());
        app.merge_preview = Some(preview(MergeKind::CleanMerge));
        app.rebase_preview = Some(reviewed_rebase());
        let _ = app.update(Message::Wrote(
            Operation::RenameBranch,
            Ok("Renamed".into()),
        ));
        assert!(app.rename_branch.is_empty());
        assert_eq!(app.new_branch, "creation draft");
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        assert_eq!(app.panel, Panel::Branches);
        assert!(app.merge_preview.is_none());
        assert!(app.rebase_preview.is_none());
        assert!(app.target_branch.is_none());
    }

    #[test]
    fn unborn_detached_and_active_operations_cannot_start_a_rename() {
        for state in ["unborn", "detached", "merge", "rebase"] {
            let mut app = if state == "rebase" {
                pending_rebase()
            } else {
                loaded()
            };
            let basis = &mut app.snapshot.as_mut().unwrap().commit_basis;
            if state != "unborn" {
                basis.head = Some(format!("{:040}", 1));
            }
            if state != "detached" {
                basis.branch = Some("refs/heads/main".into());
            }
            if state == "merge" {
                basis.merge_heads = Some(b"parent\n".to_vec());
            }
            app.rename_branch = "renamed".into();
            let _ = app.update(Message::RenameCurrentBranch);
            assert!(!app.loading, "{state}");
            assert_eq!(app.rename_branch, "renamed");
            assert!(app.notice.contains("committed local branch"));
        }
    }
}
