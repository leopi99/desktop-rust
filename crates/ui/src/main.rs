//! Iced entry point and guarded UI state transitions for the Rust preview.
mod appearance;
mod capture;
mod clip;
mod diff_view;
mod focus_region;
mod keyboard;
mod layout;
mod navigation;
#[cfg(test)]
mod presentation_tests;
mod selection;
mod update;

use desktop_core::{
    CommitDetails, CommitFile, CommitSummary, ConflictContent, ConflictReview, ConflictSide,
    ConflictSideContent, ConflictSidePreview, ConflictSideResolutionReview, ConflictWorkingContent,
    FileChange, FileDiff, GitRuntime, HunkLine, HunkReview, HunkSide, LocalBranch, MergeKind,
    MergePreview, RebaseKind, RebasePreview, Remote, RemoteAction, RemoteUrlKind,
    RepositorySnapshot, StashEntry,
};
use desktop_git::TransferControl;
use focus_region::{field, FieldId};
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
    Git,
}

#[derive(Default)]
struct Desktop {
    git_runtime: Option<Result<GitRuntime, String>>,
    checking_git: bool,
    panel: Panel,
    capture: Option<std::path::PathBuf>,
    capture_frames: u8,
    capture_navigation_focus: bool,
    capture_scroll_focus: bool,
    capture_success: std::sync::Arc<std::sync::atomic::AtomicBool>,
    path: String,
    snapshot: Option<RepositorySnapshot>,
    selected_change: Option<FileChange>,
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
    delete_branch_pending: Option<LocalBranch>,
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
    hunk_review: Option<std::sync::Arc<HunkReview>>,
    selected_hunks: std::collections::BTreeSet<usize>,
    selected_lines: std::collections::BTreeSet<HunkLine>,
    side_resolution_review: Option<std::sync::Arc<ConflictSideResolutionReview>>,
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
    DismissReview,
    MoveFocus(keyboard::FocusDirection),
    PanelSelected(Panel),
    CheckGitRuntime,
    GitRuntimeLoaded(Result<GitRuntime, String>),
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
    CreateBranchAtCommit,
    OlderCommitsLoaded(Result<Vec<CommitSummary>, String>),
    Select(FileChange),
    ReviewHunks(FileChange),
    ReviewStagedHunks(FileChange),
    HunksReviewed(Result<HunkReview, String>),
    StageHunk(usize),
    UnstageHunk(usize),
    CancelHunks,
    ToggleHunk(usize, bool),
    ToggleHunkLine(HunkLine, bool),
    ApplySelectedHunks(HunkSide),
    DiffLoaded(Result<FileDiff, String>),
    ReviewConflict(FileChange),
    ReviewConflictSide(FileChange, ConflictSide),
    RequestConflictSideResolution(FileChange, ConflictSide),
    ConflictSideResolutionReviewed(Result<ConflictSideResolutionReview, String>),
    ApplyConflictSideResolution,
    CancelConflictSideResolution,
    ConflictSidePreviewed(Result<ConflictSidePreview, String>),
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
    RenameSelectedBranch,
    RequestDeleteBranch,
    CancelDeleteBranch,
    DeleteBranch,
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
    CreateBranchAtCommit,
    RenameBranch,
    DeleteBranch,
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
            controls = controls.push(row![field(FieldId::StashDescription, text_input("Saved changes description (optional)", &self.stash_message).on_input(Message::StashMessageChanged)), save].spacing(8))
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
            field(
                FieldId::RemoteName,
                text_input("Remote name (default: origin)", &self.new_remote_name)
                    .on_input(Message::RemoteNameChanged)
            ),
            field(
                FieldId::RemoteNewUrl,
                text_input("URL or local path", &self.new_remote_url)
                    .on_input(Message::RemoteUrlChanged)
            ),
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
                        field(
                            FieldId::RemoteFetchUrl,
                            text_input("Fetch URL", &self.fetch_url)
                                .on_input(Message::FetchUrlChanged)
                        ),
                        fetch
                    ]
                    .spacing(8),
                )
                .push(
                    row![
                        field(
                            FieldId::RemotePushUrl,
                            text_input("Push URL", &self.push_url)
                                .on_input(Message::PushUrlChanged)
                        ),
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(start) = capture::Start::parse(std::env::args_os().skip(1))? else {
        return Ok(());
    };
    let size = start.size;
    let capturing = start.capture.is_some();
    let mut app = start.initialize()?;
    let startup = app.update(Message::CheckGitRuntime);
    let success = app.capture_success.clone();
    iced::application("Desktop Rust", Desktop::update, Desktop::view)
        .window(iced::window::Settings {
            size,
            min_size: Some(iced::Size::new(960.0, 640.0)),
            ..Default::default()
        })
        .theme(appearance::theme)
        .settings(iced::Settings {
            default_text_size: 13.into(),
            ..Default::default()
        })
        .subscription(keyboard::subscription)
        .run_with(|| (app, startup))?;
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
    fn moving_focus_preserves_drafts_hunk_authority_and_busy_operation_state() {
        for busy in [false, true] {
            let mut app = hunk_app();
            app.loading = busy;
            app.summary = "keep summary".into();
            app.description = "keep body".into();
            app.new_branch = "keep branch draft".into();
            app.selected_hunks.insert(0);
            let line = HunkLine { hunk: 0, line: 1 };
            app.selected_lines.insert(line);
            let review = app.hunk_review.clone().unwrap();
            let basis = app.snapshot.as_ref().unwrap().commit_basis.clone();
            for direction in [
                keyboard::FocusDirection::Next,
                keyboard::FocusDirection::Previous,
            ] {
                let _ = app.update(Message::MoveFocus(direction));
                assert_eq!(app.loading, busy);
                assert_eq!(app.summary, "keep summary");
                assert_eq!(app.description, "keep body");
                assert_eq!(app.new_branch, "keep branch draft");
                assert_eq!(app.panel, Panel::Changes);
                assert!(std::sync::Arc::ptr_eq(
                    &review,
                    app.hunk_review.as_ref().unwrap()
                ));
                assert_eq!(
                    app.selected_hunks.iter().copied().collect::<Vec<_>>(),
                    vec![0]
                );
                assert_eq!(
                    app.selected_lines.iter().copied().collect::<Vec<_>>(),
                    vec![line]
                );
                assert_eq!(app.snapshot.as_ref().unwrap().commit_basis, basis);
            }
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
        let _ = app.update(Message::RenameSelectedBranch);
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
        let _ = app.update(Message::RenameSelectedBranch);
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
    pub(super) fn hunk_app() -> Desktop {
        let mut app = loaded();
        let snapshot = app.snapshot.as_ref().unwrap();
        let review = HunkReview {
            side: HunkSide::Unstaged,
            repository: snapshot.repository.clone(),
            change: FileChange {
                path: "file".into(),
                original_path: None,
                index: ' ',
                worktree: 'M',
            },
            basis: snapshot.commit_basis.clone(),
            working: ConflictWorkingContent::File {
                bytes: b"edited\n".to_vec(),
                executable: false,
            },
            attributes: Vec::new(),
            settings: Vec::new(),
            header: "header".into(),
            hunks: vec!["@@ -1 +1 @@\n-base\n+edited\n".into()],
        };
        let _ = app.update(Message::HunksReviewed(Ok(review)));
        app
    }

    #[test]
    fn staging_a_hunk_consumes_the_review_and_preserves_drafts_after_failure_refresh() {
        let mut app = hunk_app();
        let _ = app.update(Message::StageHunk(9));
        assert!(!app.loading);
        assert!(app.hunk_review.is_some());
        let _ = app.update(Message::StageHunk(0));
        assert!(app.loading);
        assert!(app.hunk_review.is_none());
        let _ = app.update(Message::StageHunk(0));
        let _ = app.update(Message::CancelHunks);
        let _ = app.update(Message::PanelSelected(Panel::Branches));
        assert!(app.loading);
        assert_eq!(app.panel, Panel::Changes);
        let _ = app.update(Message::Wrote(Operation::Index, Err("index locked".into())));
        assert!(app.loading);
        assert!(app.notice.contains("index locked"));
        assert!(app.hunk_review.is_none());
        let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
        assert!(!app.loading);
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
    }

    #[test]
    fn navigation_selection_preview_and_refresh_revoke_hunk_authority() {
        for action in 0..5 {
            let mut app = hunk_app();
            let change = app.hunk_review.as_ref().unwrap().change.clone();
            let message = match action {
                0 => Message::CancelHunks,
                1 => Message::PanelSelected(Panel::Branches),
                2 => Message::Select(change),
                3 => Message::Refreshed(Err("refresh failed".into())),
                _ => Message::DiffLoaded(Ok(FileDiff {
                    staged: String::new(),
                    unstaged: "display only".into(),
                })),
            };
            let _ = app.update(message);
            assert!(app.hunk_review.is_none(), "{action}");
            assert_eq!(app.summary, "Summary");
            assert_eq!(app.description, "Details");
        }
    }

    #[test]
    fn ordinary_diff_and_off_panel_or_busy_requests_cannot_stage_hunks() {
        let mut app = loaded();
        let _ = app.update(Message::StageHunk(0));
        assert!(!app.loading);
        for panel in [Panel::History, Panel::Branches] {
            let mut app = hunk_app();
            app.panel = panel;
            let change = app.hunk_review.as_ref().unwrap().change.clone();
            let _ = app.update(Message::ReviewHunks(change));
            let _ = app.update(Message::StageHunk(0));
            assert!(!app.loading);
            assert!(app.hunk_review.is_some());
        }
        let mut app = hunk_app();
        app.loading = true;
        let change = app.hunk_review.as_ref().unwrap().change.clone();
        let _ = app.update(Message::ReviewHunks(change));
        let _ = app.update(Message::StageHunk(0));
        let _ = app.update(Message::CancelHunks);
        assert!(app.loading);
        assert!(app.hunk_review.is_some());
    }

    fn staged_hunk_app() -> Desktop {
        let mut app = hunk_app();
        let mut review = (*app.hunk_review.take().unwrap()).clone();
        review.side = HunkSide::Staged;
        review.change.index = 'M';
        let _ = app.update(Message::HunksReviewed(Ok(review)));
        app
    }

    #[test]
    fn staged_hunk_review_only_authorizes_unstaging_and_preserves_drafts_on_failure() {
        let mut app = staged_hunk_app();
        let _ = app.update(Message::StageHunk(0));
        assert!(!app.loading);
        assert!(app.hunk_review.is_some());
        let _ = app.update(Message::UnstageHunk(99));
        assert!(!app.loading);
        let _ = app.update(Message::UnstageHunk(0));
        assert!(app.loading);
        assert!(app.hunk_review.is_none());
        let _ = app.update(Message::CancelHunks);
        let _ = app.update(Message::UnstageHunk(0));
        let _ = app.update(Message::Wrote(Operation::Index, Err("index locked".into())));
        assert!(app.loading);
        assert!(app.notice.contains("index locked"));
        let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
        assert!(!app.loading);
        assert!(app.hunk_review.is_none());
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        let mut app = hunk_app();
        let _ = app.update(Message::UnstageHunk(0));
        assert!(!app.loading);
        assert!(app.hunk_review.is_some());
    }

    #[test]
    fn staged_hunk_requests_respect_navigation_busy_state_and_review_invalidation() {
        for state in ["navigation", "refresh", "busy", "off-panel"] {
            let mut app = staged_hunk_app();
            let change = app.hunk_review.as_ref().unwrap().change.clone();
            match state {
                "navigation" => {
                    let _ = app.update(Message::PanelSelected(Panel::History));
                }
                "refresh" => {
                    let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
                }
                "busy" => app.loading = true,
                _ => app.panel = Panel::Branches,
            }
            let _ = app.update(Message::UnstageHunk(0));
            if matches!(state, "busy" | "off-panel") {
                let _ = app.update(Message::ReviewStagedHunks(change));
                assert_eq!(app.loading, state == "busy");
                assert!(app.hunk_review.is_some());
            } else {
                assert!(!app.loading);
                assert!(app.hunk_review.is_none());
            }
            assert_eq!(app.summary, "Summary");
            assert_eq!(app.description, "Details");
        }
    }

    fn multi_hunk_app(side: HunkSide) -> Desktop {
        let mut app = hunk_app();
        let mut review = (*app.hunk_review.take().unwrap()).clone();
        review.side = side;
        review.hunks = vec!["first".into(), "second".into(), "third".into()];
        let _ = app.update(Message::HunksReviewed(Ok(review)));
        app
    }

    #[test]
    fn multi_hunk_selection_is_unique_and_only_the_matching_nonempty_action_consumes_review() {
        for side in [HunkSide::Unstaged, HunkSide::Staged] {
            let mut app = multi_hunk_app(side);
            let _ = app.update(Message::ApplySelectedHunks(side));
            assert!(!app.loading);
            for index in [2, 0, 2, 99] {
                let _ = app.update(Message::ToggleHunk(index, true));
            }
            assert_eq!(
                app.selected_hunks.iter().copied().collect::<Vec<_>>(),
                vec![0, 2]
            );
            let _ = app.update(Message::ToggleHunk(2, false));
            assert_eq!(app.selected_hunks.len(), 1);
            let wrong = if side == HunkSide::Staged {
                HunkSide::Unstaged
            } else {
                HunkSide::Staged
            };
            let _ = app.update(Message::ApplySelectedHunks(wrong));
            assert!(!app.loading);
            assert!(app.hunk_review.is_some());
            let _ = app.update(Message::ApplySelectedHunks(side));
            assert!(app.loading);
            assert!(app.hunk_review.is_none());
            assert!(app.selected_hunks.is_empty());
        }
    }

    #[test]
    fn multi_hunk_busy_actions_and_failure_refresh_preserve_commit_drafts() {
        let mut app = multi_hunk_app(HunkSide::Unstaged);
        let _ = app.update(Message::ToggleHunk(0, true));
        let _ = app.update(Message::ToggleHunk(2, true));
        let _ = app.update(Message::ApplySelectedHunks(HunkSide::Unstaged));
        assert!(app.loading);
        let _ = app.update(Message::ToggleHunk(1, true));
        let _ = app.update(Message::ApplySelectedHunks(HunkSide::Unstaged));
        let _ = app.update(Message::CancelHunks);
        assert!(app.selected_hunks.is_empty());
        assert!(app.loading);
        let _ = app.update(Message::Wrote(Operation::Index, Err("index locked".into())));
        assert!(app.loading);
        assert!(app.selected_hunks.is_empty());
        let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
        assert!(!app.loading);
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
    }

    #[test]
    fn hunk_review_replacement_navigation_and_single_actions_clear_multiple_selection() {
        for action in 0..6 {
            let mut app = multi_hunk_app(HunkSide::Unstaged);
            let _ = app.update(Message::ToggleHunk(0, true));
            let _ = app.update(Message::ToggleHunk(2, true));
            let message = match action {
                0 => Message::PanelSelected(Panel::Branches),
                1 => Message::CancelHunks,
                2 => Message::Refreshed(Err("refresh failed".into())),
                3 => Message::HunksReviewed(Err("unsupported".into())),
                4 => Message::StageHunk(1),
                _ => Message::Select(app.hunk_review.as_ref().unwrap().change.clone()),
            };
            let _ = app.update(message);
            assert!(app.selected_hunks.is_empty(), "{action}");
            assert!(app.hunk_review.is_none());
            assert_eq!(app.summary, "Summary");
        }
    }

    fn line_app(side: HunkSide) -> Desktop {
        let mut app = hunk_app();
        let mut review = (*app.hunk_review.take().unwrap()).clone();
        review.side = side;
        review.hunks = vec![
            "@@ -1,2 +1,2 @@\n context\n-old\n+new\n".into(),
            "@@ -10 +10 @@\n-x\n+y\n".into(),
        ];
        let _ = app.update(Message::HunksReviewed(Ok(review)));
        app
    }

    #[test]
    fn line_selection_can_refine_a_whole_hunk_and_combine_with_another_block() {
        let mut app = line_app(HunkSide::Unstaged);
        let _ = app.update(Message::ToggleHunk(0, true));
        let _ = app.update(Message::ToggleHunkLine(
            HunkLine { hunk: 0, line: 3 },
            false,
        ));
        assert!(app.selected_hunks.is_empty());
        assert_eq!(
            app.selected_lines.iter().copied().collect::<Vec<_>>(),
            vec![HunkLine { hunk: 0, line: 2 }]
        );
        for line in [0, 1, 999] {
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line }, true));
        }
        assert_eq!(app.selected_lines.len(), 1);
        let _ = app.update(Message::ToggleHunk(1, true));
        assert_eq!(
            app.combined_hunk_lines(app.hunk_review.as_ref().unwrap()),
            vec![
                HunkLine { hunk: 0, line: 2 },
                HunkLine { hunk: 1, line: 1 },
                HunkLine { hunk: 1, line: 2 }
            ]
        );
        let _ = app.update(Message::ToggleHunk(0, true));
        assert!(app.selected_lines.is_empty());
        assert_eq!(app.selected_hunks.len(), 2);
    }

    #[test]
    fn applying_selected_lines_is_direction_bound_consumes_authority_and_keeps_drafts() {
        for side in [HunkSide::Unstaged, HunkSide::Staged] {
            let mut app = line_app(side);
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 2 }, true));
            let wrong = if side == HunkSide::Staged {
                HunkSide::Unstaged
            } else {
                HunkSide::Staged
            };
            let _ = app.update(Message::ApplySelectedHunks(wrong));
            assert!(!app.loading);
            assert_eq!(app.selected_lines.len(), 1);
            let _ = app.update(Message::ApplySelectedHunks(side));
            assert!(app.loading);
            assert!(app.hunk_review.is_none());
            assert!(app.selected_lines.is_empty());
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 3 }, true));
            let _ = app.update(Message::CancelHunks);
            assert!(app.selected_lines.is_empty());
            let _ = app.update(Message::Wrote(Operation::Index, Err("index locked".into())));
            assert!(app.loading);
            let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
            assert!(!app.loading);
            assert!(app.selected_lines.is_empty());
            assert_eq!(app.summary, "Summary");
            assert_eq!(app.description, "Details");
        }
    }

    #[test]
    fn line_choices_obey_review_invalidation_eof_bounds_and_busy_guards() {
        for action in 0..5 {
            let mut app = line_app(HunkSide::Unstaged);
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 2 }, true));
            let message = match action {
                0 => Message::CancelHunks,
                1 => Message::PanelSelected(Panel::Branches),
                2 => Message::Refreshed(Err("failed".into())),
                3 => Message::HunksReviewed(Err("failed".into())),
                _ => Message::StageHunk(1),
            };
            let _ = app.update(message);
            assert!(app.selected_lines.is_empty());
            assert!(app.selected_hunks.is_empty());
            assert_eq!(app.summary, "Summary");
        }
        for body in [
            "@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n"
                .to_owned(),
            format!("@@ -1 +1 @@\n-{}\n+new\n", "x".repeat(70_000)),
        ] {
            let mut app = line_app(HunkSide::Unstaged);
            let mut review = (*app.hunk_review.take().unwrap()).clone();
            review.hunks = vec![body];
            let _ = app.update(Message::HunksReviewed(Ok(review)));
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 1 }, true));
            let expected =
                usize::from(app.hunk_review.as_ref().unwrap().line_selection_available());
            assert_eq!(app.selected_lines.len(), expected);
            let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 2 }, true));
            assert_eq!(app.selected_lines.len(), expected); // A no-newline marker is not a changed row.
            let _ = app.update(Message::ToggleHunk(0, true));
            assert_eq!(app.selected_hunks.len(), 1);
        }
        let mut app = line_app(HunkSide::Unstaged);
        app.loading = true;
        let _ = app.update(Message::ToggleHunkLine(HunkLine { hunk: 0, line: 2 }, true));
        assert!(app.selected_lines.is_empty());
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

    fn branch_from_history_app() -> Desktop {
        let mut app = history_app();
        app.new_branch = "from-history".into();
        app.rename_branch = "rename draft".into();
        app.commit_details = Some(CommitDetails {
            commit: history_commit(1),
            parents: Vec::new(),
            message: "body".into(),
            files: Vec::new(),
        });
        app
    }

    #[test]
    fn history_branch_creation_keeps_the_panel_and_blocks_busy_redirection() {
        let mut app = branch_from_history_app();
        let _ = app.update(Message::CreateBranchAtCommit);
        assert!(app.loading);
        assert!(app.show_history);
        assert_eq!(app.panel, Panel::History);
        assert!(app.notice.contains(&history_commit(1).oid));
        let _ = app.update(Message::BranchNameChanged("redirect".into()));
        let _ = app.update(Message::SelectCommit(history_commit(2)));
        let _ = app.update(Message::CreateBranchAtCommit);
        assert_eq!(app.new_branch, "from-history");
        assert_eq!(
            app.commit_details.as_ref().unwrap().commit,
            history_commit(1)
        );
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
    }

    #[test]
    fn history_branch_creation_requires_observed_selection_name_and_idle_history() {
        for state in [
            "missing",
            "unobserved",
            "empty",
            "panel",
            "hidden",
            "merge",
            "rebase",
            "repository",
        ] {
            let mut app = branch_from_history_app();
            match state {
                "missing" => app.commit_details = None,
                "unobserved" => app.history_commits.clear(),
                "empty" => app.new_branch.clear(),
                "panel" => app.panel = Panel::Branches,
                "hidden" => app.show_history = false,
                "merge" => {
                    app.snapshot.as_mut().unwrap().commit_basis.merge_heads =
                        Some(b"parent".to_vec())
                }
                "rebase" => {
                    app.snapshot.as_mut().unwrap().commit_basis.rebase =
                        pending_rebase().snapshot.unwrap().commit_basis.rebase
                }
                _ => app.snapshot = None,
            }
            let _ = app.update(Message::CreateBranchAtCommit);
            assert!(!app.loading, "{state}");
            assert_eq!(app.summary, "Summary");
        }
    }

    #[test]
    fn history_branch_completion_preserves_panel_and_unrelated_drafts_and_refreshes() {
        for success in [false, true] {
            let mut app = branch_from_history_app();
            let _ = app.update(Message::Wrote(
                Operation::CreateBranchAtCommit,
                if success {
                    Ok("Created".into())
                } else {
                    Err("occupied".into())
                },
            ));
            assert!(app.loading);
            assert!(app.show_history);
            assert_eq!(app.panel, Panel::History);
            assert_eq!(app.new_branch, if success { "" } else { "from-history" });
            assert_eq!(app.rename_branch, "rename draft");
            assert_eq!(app.summary, "Summary");
            assert_eq!(app.description, "Details");
        }
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
    #[test]
    fn selected_rename_starts_without_switching_and_preserves_other_drafts() {
        let mut app = delete_app();
        app.rename_branch = "new-topic".into();
        app.new_branch = "creation draft".into();
        app.delete_branch_pending = app.target_branch.clone();
        let _ = app.update(Message::RenameSelectedBranch);
        assert!(app.loading);
        assert_eq!(app.panel, Panel::Branches);
        assert!(app.notice.contains("selected branch"));
        assert!(app.delete_branch_pending.is_none());
        assert_eq!(app.rename_branch, "new-topic");
        assert_eq!(app.new_branch, "creation draft");
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.description, "Details");
        let _ = app.update(Message::RenameBranchNameChanged("redirect".into()));
        let _ = app.update(Message::BranchSelected(target()));
        let _ = app.update(Message::RenameSelectedBranch);
        assert_eq!(app.rename_branch, "new-topic");
        assert!(app.loading);
    }

    #[test]
    fn selected_rename_needs_an_observed_non_current_branch_and_an_idle_branches_panel() {
        for state in [
            "missing", "current", "stale", "empty", "panel", "busy", "merge", "rebase", "detached",
            "unborn",
        ] {
            let mut app = delete_app();
            app.rename_branch = "renamed".into();
            match state {
                "missing" => app.target_branch = None,
                "current" => app.target_branch.as_mut().unwrap().is_current = true,
                "stale" => app.snapshot.as_mut().unwrap().branches.clear(),
                "empty" => app.rename_branch.clear(),
                "panel" => app.panel = Panel::Changes,
                "busy" => app.loading = true,
                "merge" => {
                    app.snapshot.as_mut().unwrap().commit_basis.merge_heads =
                        Some(b"parent".to_vec())
                }
                "rebase" => {
                    app.snapshot.as_mut().unwrap().commit_basis.rebase =
                        pending_rebase().snapshot.unwrap().commit_basis.rebase
                }
                "detached" => app.snapshot.as_mut().unwrap().commit_basis.branch = None,
                _ => app.snapshot.as_mut().unwrap().commit_basis.head = None,
            }
            let _ = app.update(Message::RenameSelectedBranch);
            assert_eq!(app.loading, state == "busy", "{state}");
            assert_eq!(app.summary, "Summary");
            assert_eq!(app.description, "Details");
        }
    }

    fn delete_app() -> Desktop {
        let mut app = loaded();
        app.panel = Panel::Branches;
        let snapshot = app.snapshot.as_mut().unwrap();
        snapshot.commit_basis.branch = Some("refs/heads/main".into());
        snapshot.commit_basis.head = Some("a".repeat(40));
        snapshot.branches.push(target());
        app.target_branch = Some(target());
        app
    }

    #[test]
    fn deletion_requires_a_current_confirmation_and_cancel_revokes_it() {
        let mut app = delete_app();
        let _ = app.update(Message::DeleteBranch);
        assert!(!app.loading);
        let _ = app.update(Message::RequestDeleteBranch);
        assert_eq!(app.delete_branch_pending, Some(target()));
        let _ = app.update(Message::CancelDeleteBranch);
        let _ = app.update(Message::DeleteBranch);
        assert!(!app.loading);
        let _ = app.update(Message::RequestDeleteBranch);
        let _ = app.update(Message::DeleteBranch);
        assert!(app.loading);
        assert!(app.delete_branch_pending.is_none());
    }

    #[test]
    fn selection_navigation_and_refresh_revoke_branch_deletion_confirmation() {
        for change in ["selection", "panel", "refresh", "path"] {
            let mut app = delete_app();
            let _ = app.update(Message::RequestDeleteBranch);
            match change {
                "selection" => {
                    let _ = app.update(Message::BranchSelected(target()));
                }
                "panel" => {
                    let _ = app.update(Message::PanelSelected(Panel::Remotes));
                }
                "refresh" => {
                    let snapshot = app.snapshot.clone().unwrap();
                    let _ = app.update(Message::Refreshed(Ok(snapshot)));
                }
                _ => {
                    let _ = app.update(Message::PathChanged("/new".into()));
                }
            }
            assert!(app.delete_branch_pending.is_none(), "{change}");
            let _ = app.update(Message::DeleteBranch);
            assert!(!app.loading);
        }
    }

    #[test]
    fn busy_ui_cannot_confirm_or_repeat_branch_deletion() {
        let mut app = delete_app();
        app.loading = true;
        let _ = app.update(Message::RequestDeleteBranch);
        let _ = app.update(Message::DeleteBranch);
        assert!(app.delete_branch_pending.is_none());
        assert!(app.notice.is_empty());
    }

    #[test]
    fn changed_selection_cannot_reuse_a_reviewed_branch_even_without_navigation() {
        let mut app = delete_app();
        let _ = app.update(Message::RequestDeleteBranch);
        app.target_branch.as_mut().unwrap().oid = "b".repeat(40);
        let _ = app.update(Message::DeleteBranch);
        assert!(!app.loading);
        assert!(app.delete_branch_pending.is_none());
    }

    #[test]
    fn deletion_failure_refreshes_without_clearing_other_drafts_or_reusing_confirmation() {
        let mut app = delete_app();
        app.new_branch = "create draft".into();
        app.rename_branch = "rename draft".into();
        let _ = app.update(Message::RequestDeleteBranch);
        let _ = app.update(Message::Wrote(
            Operation::DeleteBranch,
            Err("Not merged".into()),
        ));
        assert!(app.loading);
        assert_eq!(app.panel, Panel::Branches);
        assert!(app.delete_branch_pending.is_none());
        assert!(app.target_branch.is_none());
        assert_eq!(app.new_branch, "create draft");
        assert_eq!(app.rename_branch, "rename draft");
        assert_eq!(app.summary, "Summary");
        assert!(app.notice.contains("Not merged"));
    }

    #[test]
    fn current_unborn_detached_and_active_branch_states_cannot_request_deletion() {
        for state in ["current", "unborn", "detached", "merge", "rebase"] {
            let mut app = delete_app();
            match state {
                "current" => app.target_branch.as_mut().unwrap().is_current = true,
                "unborn" => app.snapshot.as_mut().unwrap().commit_basis.head = None,
                "detached" => app.snapshot.as_mut().unwrap().commit_basis.branch = None,
                "merge" => {
                    app.snapshot.as_mut().unwrap().commit_basis.merge_heads = Some(Vec::new())
                }
                _ => {
                    app.snapshot.as_mut().unwrap().commit_basis.rebase =
                        pending_rebase().snapshot.unwrap().commit_basis.rebase;
                }
            }
            let _ = app.update(Message::RequestDeleteBranch);
            assert!(app.delete_branch_pending.is_none(), "{state}");
        }
    }
    #[test]
    fn runtime_check_is_read_only_and_does_not_lock_repository_navigation() {
        let mut app = delete_app();
        let before = app.snapshot.clone();
        let _ = app.update(Message::CheckGitRuntime);
        assert!(app.checking_git);
        assert!(!app.loading);
        assert_eq!(app.snapshot, before);
        assert_eq!(app.summary, "Summary");
        let _ = app.update(Message::PanelSelected(Panel::Git));
        assert_eq!(app.panel, Panel::Git);
        assert_eq!(app.snapshot, before);
        assert!(app.notice.is_empty());
    }

    #[test]
    fn runtime_failure_does_not_discard_repository_or_drafts_and_allows_retry() {
        let mut app = loaded();
        app.checking_git = true;
        let before = app.snapshot.clone();
        let _ = app.update(Message::GitRuntimeLoaded(Err("Install Git".into())));
        assert!(!app.checking_git);
        assert!(app.git_runtime.as_ref().unwrap().is_err());
        assert_eq!(app.snapshot, before);
        assert_eq!(app.summary, "Summary");
        let _ = app.update(Message::CheckGitRuntime);
        assert!(app.checking_git);
        assert!(app.git_runtime.is_none());
    }

    #[test]
    fn runtime_result_does_not_complete_or_redirect_an_active_repository_write() {
        let mut app = loaded();
        app.loading = true;
        app.checking_git = true;
        let _ = app.update(Message::GitRuntimeLoaded(Ok(GitRuntime {
            version: "2.56.1".into(),
        })));
        assert!(app.loading);
        assert!(!app.checking_git);
        assert_eq!(app.summary, "Summary");
        assert_eq!(app.git_runtime.unwrap().unwrap().version, "2.56.1");
    }
}

#[cfg(test)]
mod side_preview_tests {
    use super::*;

    fn app() -> Desktop {
        Desktop {
            summary: "keep summary".into(),
            description: "keep body".into(),
            snapshot: Some(RepositorySnapshot {
                repository: desktop_core::Repository {
                    path: "/fixture".into(),
                },
                status: String::new(),
                history: String::new(),
                commits: Vec::new(),
                changes: Vec::new(),
                branches: Vec::new(),
                remotes: Vec::new(),
                stashes: Vec::new(),
                merge_message: None,
                commit_basis: desktop_core::CommitBasis {
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

    pub(super) fn choice(app: &Desktop) -> ConflictSideResolutionReview {
        let snapshot = app.snapshot.as_ref().unwrap();
        ConflictSideResolutionReview {
            preview: ConflictSidePreview {
                repository: snapshot.repository.clone(),
                change: FileChange {
                    path: "file".into(),
                    original_path: None,
                    index: 'U',
                    worktree: 'U',
                },
                side: ConflictSide::Theirs,
                oid: Some("a".repeat(40)),
                executable: false,
                content: ConflictSideContent::Binary { size: 3 },
            },
            basis: snapshot.commit_basis.clone(),
            working: ConflictWorkingContent::File {
                bytes: b"a\0b".to_vec(),
                executable: false,
            },
            selected: ConflictWorkingContent::File {
                bytes: b"c\0d".to_vec(),
                executable: false,
            },
            attributes: Vec::new(),
            settings: Vec::new(),
        }
    }

    #[test]
    fn confirmed_choice_is_consumed_and_partial_failure_refreshes_without_losing_drafts() {
        let mut app = app();
        let review = choice(&app);
        let _ = app.update(Message::ApplyConflictSideResolution);
        assert!(!app.loading);
        let _ = app.update(Message::ConflictSideResolutionReviewed(Ok(review)));
        assert!(app.side_resolution_review.is_some());
        assert!(app.detail.contains("Current working version"));
        assert!(app.detail.contains("Binary data"));
        let _ = app.update(Message::ApplyConflictSideResolution);
        assert!(app.loading);
        assert!(app.side_resolution_review.is_none());
        let detail = app.detail.clone();
        let _ = app.update(Message::CancelConflictSideResolution);
        let _ = app.update(Message::ApplyConflictSideResolution);
        let _ = app.update(Message::PanelSelected(Panel::Branches));
        assert!(app.loading);
        assert_eq!(app.panel, Panel::Changes);
        assert_eq!(app.detail, detail);
        let _ = app.update(Message::Wrote(
            Operation::ResolveConflict,
            Err("staging failed; inspect working file".into()),
        ));
        assert!(app.loading); // Refresh must finish before another write is offered.
        assert!(app.side_resolution_review.is_none());
        assert!(app.notice.contains("staging failed"));
        let _ = app.update(Message::Refreshed(Err("refresh failed".into())));
        assert!(!app.loading);
        assert_eq!(app.summary, "keep summary");
        assert_eq!(app.description, "keep body");
    }

    #[test]
    fn cancel_navigation_selection_refresh_and_preview_revoke_side_choices() {
        for action in 0..6 {
            let mut app = app();
            let review = choice(&app);
            let change = review.preview.change.clone();
            let preview = review.preview.clone();
            let _ = app.update(Message::ConflictSideResolutionReviewed(Ok(review)));
            let message = match action {
                0 => Message::CancelConflictSideResolution,
                1 => Message::PanelSelected(Panel::Branches),
                2 => Message::Select(change),
                3 => Message::Refreshed(Err("refresh failed".into())),
                4 => Message::ConflictSidePreviewed(Ok(preview)),
                _ => Message::PathChanged("/elsewhere".into()),
            };
            let _ = app.update(message);
            assert!(app.side_resolution_review.is_none(), "action {action}");
            // Changing repositories intentionally clears repository-specific drafts.
            assert_eq!(app.summary, if action == 5 { "" } else { "keep summary" });
            assert_eq!(app.description, if action == 5 { "" } else { "keep body" });
        }
    }

    #[test]
    fn side_preview_never_authorizes_resolution_and_preserves_drafts() {
        let mut app = app();
        let change = FileChange {
            path: "file".into(),
            original_path: None,
            index: 'U',
            worktree: 'U',
        };
        let _ = app.update(Message::ReviewConflictSide(
            change.clone(),
            ConflictSide::Ours,
        ));
        assert!(app.loading);
        assert!(app.conflict_review.is_none());
        let preview = ConflictSidePreview {
            repository: app.snapshot.as_ref().unwrap().repository.clone(),
            change,
            side: ConflictSide::Ours,
            oid: None,
            executable: false,
            content: ConflictSideContent::Deleted,
        };
        let _ = app.update(Message::ConflictSidePreviewed(Ok(preview)));
        assert!(!app.loading);
        assert!(app.detail.contains("index stage 2"));
        assert!(app.detail.contains("Read-only preview"));
        let _ = app.update(Message::ResolveConflict);
        assert!(!app.loading);
        assert_eq!(app.summary, "keep summary");
        assert_eq!(app.description, "keep body");
    }

    #[test]
    fn busy_side_preview_requests_do_not_redirect_the_active_operation() {
        let mut app = app();
        app.loading = true;
        app.detail = "active operation".into();
        let change = FileChange {
            path: "file".into(),
            original_path: None,
            index: 'U',
            worktree: 'U',
        };
        let _ = app.update(Message::ReviewConflictSide(change, ConflictSide::Theirs));
        assert!(app.loading);
        assert_eq!(app.detail, "active operation");
        assert_eq!(app.summary, "keep summary");
    }
}
