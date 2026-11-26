//! Guarded merge state transitions.
use crate::{worker, Desktop, MergeKind, MergePreview, Message, Operation};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    PreviewMerge,
    MergePreviewed(Result<Box<MergePreview>, String>),
    MergeBranch,
    RequestAbortMerge,
    CancelAbortMerge,
    AbortMerge,
}

impl Desktop {
    pub(super) fn update_merge(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::PreviewMerge if !self.loading => {
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
            Event::MergePreviewed(result) => {
                self.loading = false;
                match result {
                    Ok(preview) => {
                        let preview = *preview;
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
            Event::MergeBranch if !self.loading => {
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
            Event::RequestAbortMerge
                if !self.loading
                    && self
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.commit_basis.merge_heads.is_some()) =>
            {
                self.abort_merge_pending = true
            }
            Event::CancelAbortMerge => self.abort_merge_pending = false,
            Event::AbortMerge if !self.loading && self.abort_merge_pending => {
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
            Event::PreviewMerge
            | Event::MergeBranch
            | Event::RequestAbortMerge
            | Event::AbortMerge => {}
        }
        Task::none()
    }
}
