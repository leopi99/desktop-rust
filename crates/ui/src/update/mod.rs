//! Exhaustive routing from application messages to typed domain events.
mod application;
mod branches;
mod changes;
mod completion;
mod history;
mod merge;
mod rebase;
mod remotes;
mod repository;
mod stashes;

use crate::{Desktop, Message};
use iced::Task;

impl Desktop {
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PanelSelected(panel) => {
                self.update_application(application::Event::PanelSelected(panel))
            }
            Message::CheckGitRuntime => {
                self.update_application(application::Event::CheckGitRuntime)
            }
            Message::GitRuntimeLoaded(result) => {
                self.update_application(application::Event::GitRuntimeLoaded(result))
            }
            Message::CaptureFrame => self.update_application(application::Event::CaptureFrame),
            Message::Captured(screenshot) => {
                self.update_application(application::Event::Captured(screenshot))
            }
            Message::PathChanged(path) => {
                self.update_repository(repository::Event::PathChanged(path))
            }
            Message::Open => self.update_repository(repository::Event::Open),
            Message::Loaded(result) => {
                self.update_repository(repository::Event::Loaded(result.map(Box::new)))
            }
            Message::ShowHistory(show) => self.update_history(history::Event::ShowHistory(show)),
            Message::SelectCommit(commit) => {
                self.update_history(history::Event::SelectCommit(commit))
            }
            Message::CommitLoaded(result) => {
                self.update_history(history::Event::CommitLoaded(result))
            }
            Message::SelectCommitFile(file) => {
                self.update_history(history::Event::SelectCommitFile(file))
            }
            Message::CommitDiffLoaded(result) => {
                self.update_history(history::Event::CommitDiffLoaded(result))
            }
            Message::CreateBranchAtCommit => {
                self.update_history(history::Event::CreateBranchAtCommit)
            }
            Message::LoadOlderCommits => self.update_history(history::Event::LoadOlderCommits),
            Message::OlderCommitsLoaded(result) => {
                self.update_history(history::Event::OlderCommitsLoaded(result))
            }
            Message::Select(change) => self.update_changes(changes::Event::Select(change)),
            Message::DiffLoaded(result) => self.update_changes(changes::Event::DiffLoaded(result)),
            Message::ReviewConflict(change) => {
                self.update_changes(changes::Event::ReviewConflict(change))
            }
            Message::ReviewConflictSide(change, side) => {
                self.update_changes(changes::Event::ReviewConflictSide(change, side))
            }
            Message::RequestConflictSideResolution(change, side) => {
                self.update_changes(changes::Event::RequestConflictSideResolution(change, side))
            }
            Message::ConflictSideResolutionReviewed(result) => self.update_changes(
                changes::Event::ConflictSideResolutionReviewed(result.map(Box::new)),
            ),
            Message::ApplyConflictSideResolution => {
                self.update_changes(changes::Event::ApplyConflictSideResolution)
            }
            Message::CancelConflictSideResolution => {
                self.update_changes(changes::Event::CancelConflictSideResolution)
            }
            Message::ConflictSidePreviewed(result) => {
                self.update_changes(changes::Event::ConflictSidePreviewed(result))
            }
            Message::ConflictReviewed(result) => {
                self.update_changes(changes::Event::ConflictReviewed(result.map(Box::new)))
            }
            Message::ResolveConflict => self.update_changes(changes::Event::ResolveConflict),
            Message::Stage(change) => self.update_changes(changes::Event::Stage(change)),
            Message::Unstage(change) => self.update_changes(changes::Event::Unstage(change)),
            Message::SummaryChanged(value) => {
                self.update_changes(changes::Event::SummaryChanged(value))
            }
            Message::DescriptionChanged(value) => {
                self.update_changes(changes::Event::DescriptionChanged(value))
            }
            Message::Commit => self.update_changes(changes::Event::Commit),
            Message::BranchNameChanged(value) => {
                self.update_branches(branches::Event::BranchNameChanged(value))
            }
            Message::RenameBranchNameChanged(value) => {
                self.update_branches(branches::Event::RenameBranchNameChanged(value))
            }
            Message::RenameSelectedBranch => {
                self.update_branches(branches::Event::RenameSelectedBranch)
            }
            Message::RenameCurrentBranch => {
                self.update_branches(branches::Event::RenameCurrentBranch)
            }
            Message::RequestDeleteBranch => {
                self.update_branches(branches::Event::RequestDeleteBranch)
            }
            Message::CancelDeleteBranch => {
                self.update_branches(branches::Event::CancelDeleteBranch)
            }
            Message::DeleteBranch => self.update_branches(branches::Event::DeleteBranch),
            Message::BranchSelected(branch) => {
                self.update_branches(branches::Event::BranchSelected(branch))
            }
            Message::CreateBranch => self.update_branches(branches::Event::CreateBranch),
            Message::SwitchBranch => self.update_branches(branches::Event::SwitchBranch),
            Message::PreviewMerge => self.update_merge(merge::Event::PreviewMerge),
            Message::MergePreviewed(result) => {
                self.update_merge(merge::Event::MergePreviewed(result.map(Box::new)))
            }
            Message::MergeBranch => self.update_merge(merge::Event::MergeBranch),
            Message::RequestAbortMerge => self.update_merge(merge::Event::RequestAbortMerge),
            Message::CancelAbortMerge => self.update_merge(merge::Event::CancelAbortMerge),
            Message::AbortMerge => self.update_merge(merge::Event::AbortMerge),
            Message::PreviewRebase => self.update_rebase(rebase::Event::PreviewRebase),
            Message::RebasePreviewed(result) => {
                self.update_rebase(rebase::Event::RebasePreviewed(result.map(Box::new)))
            }
            Message::RebaseBranch => self.update_rebase(rebase::Event::RebaseBranch),
            Message::ContinueRebase => self.update_rebase(rebase::Event::ContinueRebase),
            Message::RequestAbortRebase => self.update_rebase(rebase::Event::RequestAbortRebase),
            Message::CancelAbortRebase => self.update_rebase(rebase::Event::CancelAbortRebase),
            Message::AbortRebase => self.update_rebase(rebase::Event::AbortRebase),
            Message::RemoteSelected(remote) => {
                self.update_remotes(remotes::Event::RemoteSelected(remote))
            }
            Message::RemoteAction(action) => {
                self.update_remotes(remotes::Event::RemoteAction(action))
            }
            Message::RemoteNameChanged(value) => {
                self.update_remotes(remotes::Event::RemoteNameChanged(value))
            }
            Message::RemoteUrlChanged(value) => {
                self.update_remotes(remotes::Event::RemoteUrlChanged(value))
            }
            Message::FetchUrlChanged(value) => {
                self.update_remotes(remotes::Event::FetchUrlChanged(value))
            }
            Message::PushUrlChanged(value) => {
                self.update_remotes(remotes::Event::PushUrlChanged(value))
            }
            Message::ToggleRemoteSettings => {
                self.update_remotes(remotes::Event::ToggleRemoteSettings)
            }
            Message::RequestRemoveRemote => {
                self.update_remotes(remotes::Event::RequestRemoveRemote)
            }
            Message::CancelRemoveRemote => self.update_remotes(remotes::Event::CancelRemoveRemote),
            Message::ConfigureRemote(action) => {
                self.update_remotes(remotes::Event::ConfigureRemote(action))
            }
            Message::ToggleStashes => self.update_stashes(stashes::Event::ToggleStashes),
            Message::StashSelected(stash) => {
                self.update_stashes(stashes::Event::StashSelected(stash))
            }
            Message::StashMessageChanged(value) => {
                self.update_stashes(stashes::Event::StashMessageChanged(value))
            }
            Message::SaveStash => self.update_stashes(stashes::Event::SaveStash),
            Message::ApplyStash => self.update_stashes(stashes::Event::ApplyStash),
            Message::CloneSourceChanged(value) => {
                self.update_repository(repository::Event::CloneSourceChanged(value))
            }
            Message::CloneDestinationChanged(value) => {
                self.update_repository(repository::Event::CloneDestinationChanged(value))
            }
            Message::Clone => self.update_repository(repository::Event::Clone),
            Message::Cloned(result) => self.update_repository(repository::Event::Cloned(result)),
            Message::InitDestinationChanged(value) => {
                self.update_repository(repository::Event::InitDestinationChanged(value))
            }
            Message::InitBranchChanged(value) => {
                self.update_repository(repository::Event::InitBranchChanged(value))
            }
            Message::Initialize => self.update_repository(repository::Event::Initialize),
            Message::Initialized(result) => {
                self.update_repository(repository::Event::Initialized(result))
            }
            Message::CancelTransfer => self.update_remotes(remotes::Event::CancelTransfer),
            Message::TransferProgress(serial, output) => {
                self.update_remotes(remotes::Event::TransferProgress(serial, output))
            }
            Message::Wrote(operation, result) => {
                self.update_completion(completion::Event::Wrote(operation, result))
            }
            Message::Refreshed(result) => {
                self.update_completion(completion::Event::Refreshed(result.map(Box::new)))
            }
        }
    }
}
