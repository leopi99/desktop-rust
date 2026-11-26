//! Guarded rebase state transitions.
use crate::{worker, Desktop, Message, Operation, RebaseKind, RebasePreview};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    PreviewRebase,
    RebasePreviewed(Result<Box<RebasePreview>, String>),
    RebaseBranch,
    ContinueRebase,
    RequestAbortRebase,
    CancelAbortRebase,
    AbortRebase,
}

impl Desktop {
    pub(super) fn update_rebase(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::PreviewRebase if !self.loading => {
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
            Event::RebasePreviewed(result) => {
                self.loading = false;
                match result {
                    Ok(preview) => {
                        let preview = *preview;
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
            Event::RebaseBranch if !self.loading => {
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
            Event::ContinueRebase if !self.loading => {
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
            Event::RequestAbortRebase
                if !self.loading
                    && self.snapshot.as_ref().is_some_and(|s| {
                        s.commit_basis.rebase.as_ref().is_some_and(|r| r.supported)
                    }) =>
            {
                self.abort_rebase_pending = true
            }
            Event::CancelAbortRebase => self.abort_rebase_pending = false,
            Event::AbortRebase if !self.loading && self.abort_rebase_pending => {
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
            Event::PreviewRebase
            | Event::RebaseBranch
            | Event::ContinueRebase
            | Event::RequestAbortRebase
            | Event::AbortRebase => {}
        }
        Task::none()
    }
}
