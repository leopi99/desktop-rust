use super::{clip::clip, Desktop, Message, Panel};
use desktop_core::{FileChange, MergeKind, RebaseKind, RemoteAction};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

impl Desktop {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let mut files = column![text("Changed files").size(20)].spacing(6);
        if let Some(snapshot) = &self.snapshot {
            if snapshot.changes.is_empty() {
                files = files.push(text("Working tree clean"));
            }
            for change in &snapshot.changes {
                let label = format!(
                    "{}{}  {}",
                    change.index,
                    change.worktree,
                    change.path.display()
                );
                let mut control = button(text(label));
                if !self.loading {
                    control = control.on_press(Message::Select(change.clone()));
                }
                let mut stage = button("Stage");
                let mut unstage = button("Unstage");
                if !self.loading && change.can_stage() {
                    stage = stage.on_press(Message::Stage(change.clone()));
                }
                if !self.loading && change.can_unstage() {
                    unstage = unstage.on_press(Message::Unstage(change.clone()));
                }
                files = files.push(control);
                if change.is_conflicted() {
                    let mut review = button("Review conflict");
                    if !self.loading {
                        review = review.on_press(Message::ReviewConflict(change.clone()));
                    }
                    files = files.push(review);
                } else {
                    files = files.push(row![stage, unstage].spacing(6));
                }
            }
        }
        if let Some(review) = &self.conflict_review {
            let mut resolve = button("Mark reviewed file resolved");
            if !self.loading && review.marker_count == 0 {
                resolve = resolve.on_press(Message::ResolveConflict);
            }
            files = files
                .push(text(format!("Reviewed: {}", review.change.path.display())))
                .push(resolve);
        }
        let mut open = button(if self.loading {
            "Loading…"
        } else {
            "Open / Refresh"
        });
        if !self.loading {
            open = open.on_press(Message::Open);
        }
        let mut commit = button(
            if self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.commit_basis.merge_heads.is_some())
            {
                "Complete merge commit"
            } else {
                "Commit all staged changes"
            },
        );
        if !self.loading
            && !self.summary.trim().is_empty()
            && self.snapshot.as_ref().is_some_and(|s| {
                s.commit_basis.rebase.is_none()
                    && (s.changes.iter().any(FileChange::can_unstage)
                        || s.commit_basis.merge_heads.is_some())
                    && !s.changes.iter().any(FileChange::is_conflicted)
            })
        {
            commit = commit.on_press(Message::Commit);
        }
        let mut branch_controls = column![text("Open a repository to manage branches")].spacing(8);
        if let Some(snapshot) = &self.snapshot {
            let merging = snapshot.commit_basis.merge_heads.is_some();
            let rebasing = snapshot.commit_basis.rebase.is_some();
            let current = if let Some(state) = &snapshot.commit_basis.rebase {
                let branch = state.branch.to_string_lossy();
                format!(
                    "Rebasing branch: {}",
                    branch.strip_prefix("refs/heads/").unwrap_or(&branch)
                )
            } else if let Some(branch) = &snapshot.commit_basis.branch {
                let branch = branch.to_string_lossy();
                format!(
                    "Current branch: {}",
                    branch.strip_prefix("refs/heads/").unwrap_or(&branch)
                )
            } else {
                String::from("Detached HEAD — create or switch to a branch to continue")
            };
            let mut switch = button("Switch branch");
            if !self.loading
                && !merging
                && !rebasing
                && self
                    .target_branch
                    .as_ref()
                    .is_some_and(|branch| !branch.is_current)
            {
                switch = switch.on_press(Message::SwitchBranch);
            }
            let mut preview = button("Preview merge");
            let mut merge = button("Merge reviewed branch");
            if !self.loading && !merging && !rebasing {
                if self
                    .target_branch
                    .as_ref()
                    .is_some_and(|branch| !branch.is_current)
                {
                    preview = preview.on_press(Message::PreviewMerge);
                }
                if self
                    .merge_preview
                    .as_ref()
                    .is_some_and(|p| p.kind != MergeKind::AlreadyUpToDate)
                {
                    merge = merge.on_press(Message::MergeBranch);
                }
            }
            let mut preview_rebase = button("Preview rebase");
            let mut rebase = button("Rebase reviewed commits");
            if !self.loading && !merging && !rebasing {
                if self
                    .target_branch
                    .as_ref()
                    .is_some_and(|branch| !branch.is_current)
                {
                    preview_rebase = preview_rebase.on_press(Message::PreviewRebase);
                }
                if self
                    .rebase_preview
                    .as_ref()
                    .is_some_and(|p| p.kind != RebaseKind::AlreadyUpToDate)
                {
                    rebase = rebase.on_press(Message::RebaseBranch);
                }
            }
            let mut create = button("Create and switch");
            if !self.loading && !merging && !rebasing && !self.new_branch.is_empty() {
                create = create.on_press(Message::CreateBranch);
            }
            let mut rename = button("Rename current branch");
            if !self.loading
                && !merging
                && !rebasing
                && !self.rename_branch.is_empty()
                && snapshot.commit_basis.branch.is_some()
                && snapshot.commit_basis.head.is_some()
            {
                rename = rename.on_press(Message::RenameCurrentBranch);
            }
            branch_controls = column![
                text(current),
                row![
                    pick_list(
                        snapshot.branches.as_slice(),
                        self.target_branch.as_ref(),
                        Message::BranchSelected
                    )
                    .placeholder("Select a local branch"),
                    switch,
                ]
                .spacing(8),
                row![preview, merge].spacing(8),
                row![preview_rebase, rebase].spacing(8),
                row![
                    text_input("New branch name", &self.new_branch)
                        .on_input(Message::BranchNameChanged),
                    create
                ]
                .spacing(8),
                row![
                    text_input("New name for the current branch", &self.rename_branch)
                        .on_input(Message::RenameBranchNameChanged),
                    rename
                ]
                .spacing(8),
                text("Renaming is local. The remote branch keeps its existing name.").size(14),
            ]
            .spacing(8);
            if merging {
                let mut abort = button("Abort merge…");
                if !self.loading {
                    abort = abort.on_press(Message::RequestAbortMerge);
                }
                branch_controls = branch_controls
                    .push(text(
                        "Merge in progress: resolve conflicts and complete the merge commit.",
                    ))
                    .push(abort);
                if self.abort_merge_pending {
                    let mut confirm = button("Discard merge result and abort");
                    if !self.loading {
                        confirm = confirm.on_press(Message::AbortMerge);
                    }
                    branch_controls = branch_controls.push(text("Abort discards merge results and resolution edits. Saved stash entries are kept."))
                        .push(row![confirm, button("Cancel").on_press(Message::CancelAbortMerge)].spacing(8));
                }
            }
        }
        if let Some(snapshot) = &self.snapshot {
            if let Some(state) = &snapshot.commit_basis.rebase {
                branch_controls = branch_controls.push(text(format!(
                    "Rebase in progress: {} — step {}/{}",
                    state.branch.to_string_lossy(),
                    state.step,
                    state.total
                )));
                if state.supported {
                    let mut next = button("Continue rebase");
                    let mut abort = button("Abort rebase…");
                    if !self.loading {
                        if !snapshot.changes.iter().any(FileChange::is_conflicted) {
                            next = next.on_press(Message::ContinueRebase);
                        }
                        abort = abort.on_press(Message::RequestAbortRebase);
                    }
                    branch_controls = branch_controls.push(row![next, abort].spacing(8));
                    if self.abort_rebase_pending {
                        let mut confirm = button("Discard rebase result and abort");
                        if !self.loading {
                            confirm = confirm.on_press(Message::AbortRebase);
                        }
                        branch_controls = branch_controls.push(text("Abort returns to the original branch and discards resolution edits."))
                            .push(row![confirm, button("Cancel").on_press(Message::CancelAbortRebase)].spacing(8));
                    }
                } else {
                    branch_controls = branch_controls.push(text(
                        "Use Git to continue or abort this unsupported rebase.",
                    ));
                }
            }
        }
        let mut remote_controls = column![].spacing(8);
        if let Some(snapshot) = &self.snapshot {
            let mut fetch = button("Fetch");
            let mut pull = button("Pull (fast-forward)");
            let mut push = button("Push branch");
            if !self.loading {
                if let Some(remote) = &self.selected_remote {
                    fetch = fetch.on_press(Message::RemoteAction(RemoteAction::Fetch));
                    if remote.target_branch.is_some() {
                        pull = pull.on_press(Message::RemoteAction(RemoteAction::Pull));
                        push = push.on_press(Message::RemoteAction(RemoteAction::Push));
                    }
                }
            }
            let target = self.selected_remote.as_ref().and_then(|r| r.target_branch.as_ref())
                .map(|target| format!("Remote branch: {}", target.to_string_lossy()))
                .unwrap_or_else(|| String::from("Select a remote; pull/push require a committed local branch on its tracked remote"));
            remote_controls = column![
                row![pick_list(
                    snapshot.remotes.as_slice(),
                    self.selected_remote.as_ref(),
                    Message::RemoteSelected
                )
                .placeholder("Select a remote"),],
                row![fetch, pull, push].spacing(8),
                text(target),
                self.remote_settings_view(),
            ]
            .spacing(6);
        }
        let mut clone = button("Clone");
        if !self.loading
            && !self.clone_source.trim().is_empty()
            && !self.clone_destination.trim().is_empty()
        {
            clone = clone.on_press(Message::Clone);
        }
        let mut transfer_controls = column![].spacing(6);
        if self.transfer.is_some() || !self.transfer_progress.is_empty() {
            transfer_controls = transfer_controls.push(
                scrollable(text(&self.transfer_progress).font(iced::Font::MONOSPACE))
                    .height(Length::Fixed(90.0)),
            );
            if self.transfer.is_some() {
                transfer_controls = transfer_controls
                    .push(button("Cancel transfer").on_press(Message::CancelTransfer));
            }
        }
        let content: Element<'_, Message> = if self.show_history {
            let mut history = column![text("Commits").size(20)].spacing(6);
            if self.history_commits.is_empty() {
                history = history.push(text("No commits yet"));
            }
            for commit in &self.history_commits {
                let mut control = button(text(format!("{} {}", &commit.oid[..7], commit.subject)));
                if !self.loading {
                    control = control.on_press(Message::SelectCommit(commit.clone()));
                }
                history = history.push(control);
            }
            if self.history_has_more {
                let mut older = button("Load older commits");
                if !self.loading {
                    older = older.on_press(Message::LoadOlderCommits);
                }
                history = history.push(older);
            }
            let mut committed_files = column![text("Committed files").size(20)].spacing(6);
            if let Some(details) = &self.commit_details {
                if details.files.is_empty() {
                    committed_files =
                        committed_files.push(text("No changes relative to the first parent"));
                }
                for file in &details.files {
                    let mut control =
                        button(text(format!("{} {}", file.status, file.path.display())));
                    if !self.loading {
                        control = control.on_press(Message::SelectCommitFile(file.clone()));
                    }
                    committed_files = committed_files.push(control);
                }
            }
            row![
                scrollable(history).width(Length::FillPortion(2)),
                scrollable(committed_files).width(Length::FillPortion(2)),
                scrollable(text(&self.detail).font(iced::Font::MONOSPACE))
                    .width(Length::FillPortion(5))
            ]
            .spacing(12)
            .height(Length::Fill)
            .into()
        } else {
            row![
                scrollable(files).width(Length::FillPortion(1)),
                scrollable(text(&self.detail).font(iced::Font::MONOSPACE))
                    .width(Length::FillPortion(3))
            ]
            .spacing(16)
            .height(Length::Fill)
            .into()
        };
        let commit_controls: Element<'_, Message> = if self.show_history || self.snapshot.is_none()
        {
            column![].into()
        } else {
            column![
                text_input("Commit summary", &self.summary).on_input(Message::SummaryChanged),
                text_input("Description (optional)", &self.description)
                    .on_input(Message::DescriptionChanged),
                commit
            ]
            .spacing(12)
            .into()
        };
        let mut initialize = button("Create repository");
        if !self.loading && !self.init_destination.trim().is_empty() {
            initialize = initialize.on_press(Message::Initialize);
        }
        let mut sidebar = column![
            text("Desktop Rust").size(24),
            text("Development preview").size(13),
            text_input("Repository path", &self.path).on_input(Message::PathChanged),
            open,
        ]
        .spacing(12)
        .width(220);
        if let Some(snapshot) = &self.snapshot {
            sidebar = sidebar.push(
                text(
                    snapshot
                        .repository
                        .path
                        .file_name()
                        .unwrap_or(snapshot.repository.path.as_os_str())
                        .to_string_lossy()
                        .into_owned(),
                )
                .size(18),
            );
            sidebar = sidebar.push(
                text(
                    snapshot
                        .commit_basis
                        .branch
                        .as_ref()
                        .map(|b| {
                            b.to_string_lossy()
                                .trim_start_matches("refs/heads/")
                                .to_owned()
                        })
                        .unwrap_or_else(|| "Detached HEAD".into()),
                )
                .size(14),
            );
        }
        for (label, panel) in [
            ("Changes", Panel::Changes),
            ("History", Panel::History),
            ("Branches", Panel::Branches),
            ("Remotes", Panel::Remotes),
            ("Saved changes", Panel::Stashes),
            ("Create / Clone", Panel::Repository),
        ] {
            let active = self.panel == panel;
            let mut nav = button(label).width(Length::Fill).style(if active {
                button::primary
            } else {
                button::secondary
            });
            if !self.loading {
                nav = nav.on_press(Message::PanelSelected(panel));
            }
            sidebar = sidebar.push(nav);
        }
        let title = match self.panel {
            Panel::Changes => "Changes",
            Panel::History => "History",
            Panel::Branches => "Branches",
            Panel::Remotes => "Remotes",
            Panel::Stashes => "Saved changes",
            Panel::Repository => "Create or clone a repository",
        };
        let body: Element<'_, Message> = match self.panel {
            Panel::Changes | Panel::History => {
                if self.snapshot.is_none() {
                    column![
                        text("Open a local repository using its path in the sidebar."),
                        button("Create or clone a repository")
                            .on_press(Message::PanelSelected(Panel::Repository)),
                        text(&self.detail)
                    ]
                    .spacing(16)
                    .into()
                } else {
                    column![content, commit_controls]
                        .spacing(12)
                        .height(Length::Fill)
                        .into()
                }
            }
            Panel::Branches => scrollable(
                column![
                    branch_controls,
                    text(&self.detail).font(iced::Font::MONOSPACE)
                ]
                .spacing(20),
            )
            .height(Length::Fill)
            .into(),
            Panel::Remotes => scrollable(
                column![
                    text("Fetch updates, transfer commits, or configure repository remotes."),
                    remote_controls
                ]
                .spacing(16),
            )
            .height(Length::Fill)
            .into(),
            Panel::Stashes => scrollable(self.stash_view()).height(Length::Fill).into(),
            Panel::Repository => scrollable(
                column![
                    text("Create a repository").size(20),
                    text_input("New or existing project folder", &self.init_destination)
                        .on_input(Message::InitDestinationChanged),
                    text_input("Initial branch (default: main)", &self.init_branch)
                        .on_input(Message::InitBranchChanged),
                    initialize,
                    text("Clone a repository").size(20),
                    text_input("Clone URL or local source", &self.clone_source)
                        .on_input(Message::CloneSourceChanged),
                    text_input("New destination folder", &self.clone_destination)
                        .on_input(Message::CloneDestinationChanged),
                    clone,
                ]
                .spacing(16),
            )
            .height(Length::Fill)
            .into(),
        };
        let mut workspace = column![text(title).size(24)]
            .spacing(12)
            .width(Length::Fill);
        if self.snapshot.as_ref().is_some_and(|s| {
            s.commit_basis.merge_heads.is_some() || s.commit_basis.rebase.is_some()
        }) {
            let mut manage = button("Manage operation");
            if !self.loading {
                manage = manage.on_press(Message::PanelSelected(Panel::Branches));
            }
            workspace =
                workspace.push(row![text("Merge or rebase in progress"), manage].spacing(12));
        }
        workspace = workspace.push(body);
        if !self.notice.is_empty() {
            workspace = workspace.push(scrollable(text(&self.notice)).height(48));
        }
        workspace = workspace.push(transfer_controls);
        row![clip(scrollable(sidebar).height(Length::Fill)), workspace]
            .spacing(24)
            .padding(20)
            .height(Length::Fill)
            .into()
    }
}
