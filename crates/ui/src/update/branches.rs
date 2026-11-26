//! Guarded branches state transitions.
use crate::{worker, Desktop, LocalBranch, Message, Operation, Panel};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    BranchNameChanged(String),
    RenameBranchNameChanged(String),
    BranchSelected(LocalBranch),
    CreateBranch,
    RenameCurrentBranch,
    RenameSelectedBranch,
    RequestDeleteBranch,
    CancelDeleteBranch,
    DeleteBranch,
    SwitchBranch,
}

impl Desktop {
    pub(super) fn update_branches(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::BranchNameChanged(value) if !self.loading => self.new_branch = value,
            Event::RenameBranchNameChanged(value) if !self.loading => self.rename_branch = value,
            Event::BranchSelected(branch) if !self.loading => {
                self.delete_branch_pending = None;
                self.target_branch = Some(branch);
                self.merge_preview = None;
                self.rebase_preview = None;
            }
            Event::CreateBranch if !self.loading && !self.new_branch.is_empty() => {
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
            Event::RenameCurrentBranch if !self.loading && !self.rename_branch.is_empty() => {
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
            Event::RenameSelectedBranch
                if !self.loading
                    && self.panel == Panel::Branches
                    && !self.rename_branch.is_empty() =>
            {
                if let (Some(snapshot), Some(branch)) = (&self.snapshot, &self.target_branch) {
                    if snapshot.commit_basis.branch.is_none()
                        || snapshot.commit_basis.head.is_none()
                        || snapshot.commit_basis.merge_heads.is_some()
                        || snapshot.commit_basis.rebase.is_some()
                        || branch.is_current
                        || !snapshot.branches.contains(branch)
                    {
                        self.notice = String::from("Select an observed non-current local branch with no merge or rebase in progress");
                        return Task::none();
                    }
                    let path = snapshot.repository.path.clone();
                    let expected = snapshot.commit_basis.clone();
                    let branch = branch.clone();
                    let name = self.rename_branch.clone();
                    self.delete_branch_pending = None;
                    self.loading = true;
                    self.notice = format!(
                        "Renaming selected branch: {}…",
                        branch.name.to_string_lossy()
                    );
                    return worker(
                        move || desktop_git::rename_local_branch(&path, &branch, &name, &expected),
                        |result| Message::Wrote(Operation::RenameBranch, result),
                    );
                }
            }
            Event::RequestDeleteBranch if !self.loading && self.panel == Panel::Branches => {
                self.delete_branch_pending = None;
                if let (Some(snapshot), Some(branch)) = (&self.snapshot, &self.target_branch) {
                    if snapshot.commit_basis.branch.is_some()
                        && snapshot.commit_basis.head.is_some()
                        && snapshot.commit_basis.merge_heads.is_none()
                        && snapshot.commit_basis.rebase.is_none()
                        && !branch.is_current
                        && snapshot.branches.contains(branch)
                    {
                        self.delete_branch_pending = Some(branch.clone());
                    }
                }
            }
            Event::CancelDeleteBranch => self.delete_branch_pending = None,
            Event::DeleteBranch if !self.loading && self.panel == Panel::Branches => {
                if let Some(branch) = self.delete_branch_pending.take() {
                    if let Some(snapshot) = &self.snapshot {
                        if self.target_branch.as_ref() != Some(&branch)
                            || !snapshot.branches.contains(&branch)
                        {
                            return Task::none();
                        }
                        let path = snapshot.repository.path.clone();
                        let expected = snapshot.commit_basis.clone();
                        self.loading = true;
                        self.notice = String::from("Deleting reviewed local branch…");
                        return worker(
                            move || desktop_git::delete_local_branch(&path, &branch, &expected),
                            |result| Message::Wrote(Operation::DeleteBranch, result),
                        );
                    }
                }
            }
            Event::SwitchBranch if !self.loading => {
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
            Event::BranchNameChanged(..)
            | Event::RenameBranchNameChanged(..)
            | Event::BranchSelected(..)
            | Event::CreateBranch
            | Event::RenameSelectedBranch
            | Event::RenameCurrentBranch
            | Event::RequestDeleteBranch
            | Event::DeleteBranch
            | Event::SwitchBranch => {}
        }
        Task::none()
    }
}
