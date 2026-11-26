//! Guarded changes state transitions.
use crate::{
    worker, ConflictContent, ConflictReview, ConflictSide, ConflictSideContent,
    ConflictSidePreview, ConflictSideResolutionReview, ConflictWorkingContent, Desktop, FileChange,
    FileDiff, Message, Operation,
};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    Select(FileChange),
    ReviewHunks(FileChange, crate::HunkSide),
    HunksReviewed(Result<Box<crate::HunkReview>, String>),
    StageHunk(usize, crate::HunkSide),
    CancelHunks,
    DiffLoaded(Result<FileDiff, String>),
    ReviewConflict(FileChange),
    ReviewConflictSide(FileChange, ConflictSide),
    RequestConflictSideResolution(FileChange, ConflictSide),
    ConflictSideResolutionReviewed(Result<Box<ConflictSideResolutionReview>, String>),
    ApplyConflictSideResolution,
    CancelConflictSideResolution,
    ConflictSidePreviewed(Result<ConflictSidePreview, String>),
    ConflictReviewed(Result<Box<ConflictReview>, String>),
    ResolveConflict,
    Stage(FileChange),
    Unstage(FileChange),
    SummaryChanged(String),
    DescriptionChanged(String),
    Commit,
}

impl Desktop {
    pub(super) fn update_changes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Select(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.loading = true;
                    self.conflict_review = None;
                    self.side_resolution_review = None;
                    self.hunk_review = None;
                    self.detail = format!("Loading {}…", change.path.display());
                    return worker(
                        move || desktop_git::diff(&path, &change),
                        Message::DiffLoaded,
                    );
                }
            }
            Event::ReviewHunks(change, side)
                if !self.loading && self.panel == crate::Panel::Changes =>
            {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.conflict_review = None;
                    self.side_resolution_review = None;
                    self.hunk_review = None;
                    self.loading = true;
                    self.detail = format!("Reviewing complete {side:?} hunks…");
                    return worker(
                        move || match side {
                            crate::HunkSide::Unstaged => desktop_git::review_hunks(&path, &change),
                            crate::HunkSide::Staged => {
                                desktop_git::review_staged_hunks(&path, &change)
                            }
                        },
                        Message::HunksReviewed,
                    );
                }
            }
            Event::HunksReviewed(result) => {
                self.loading = false;
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                match result {
                    Ok(review) => self.hunk_review = Some(std::sync::Arc::new(*review)),
                    Err(error) => self.detail = format!("Unable to review hunks: {error}"),
                }
            }
            Event::StageHunk(index, side)
                if !self.loading && self.panel == crate::Panel::Changes =>
            {
                if self
                    .hunk_review
                    .as_ref()
                    .is_some_and(|review| index < review.hunks.len() && review.side == side)
                {
                    if let Some(review) = self.hunk_review.take() {
                        self.loading = true;
                        self.notice = format!("Updating reviewed {side:?} hunk…");
                        return worker(
                            move || match side {
                                crate::HunkSide::Unstaged => {
                                    desktop_git::stage_hunk(&review, index)
                                }
                                crate::HunkSide::Staged => {
                                    desktop_git::unstage_hunk(&review, index)
                                }
                            },
                            |result| Message::Wrote(Operation::Index, result),
                        );
                    }
                }
            }
            Event::CancelHunks if !self.loading => self.hunk_review = None,
            Event::DiffLoaded(result) => {
                self.hunk_review = None;
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
            Event::ReviewConflict(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.conflict_review = None;
                    self.side_resolution_review = None;
                    self.hunk_review = None;
                    self.loading = true;
                    self.detail = format!("Reviewing conflict: {}", change.path.display());
                    return worker(
                        move || desktop_git::inspect_conflict(&path, &change),
                        Message::ConflictReviewed,
                    );
                }
            }
            Event::ReviewConflictSide(change, side) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.conflict_review = None;
                    self.side_resolution_review = None;
                    self.hunk_review = None;
                    self.loading = true;
                    self.detail = String::from("Loading read-only index side…");
                    return worker(
                        move || desktop_git::inspect_conflict_side(&path, &change, side),
                        Message::ConflictSidePreviewed,
                    );
                }
            }
            Event::RequestConflictSideResolution(change, side) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    let path = snapshot.repository.path.clone();
                    self.conflict_review = None;
                    self.side_resolution_review = None;
                    self.hunk_review = None;
                    self.loading = true;
                    self.detail = String::from("Reviewing side replacement/deletion…");
                    return worker(
                        move || desktop_git::review_conflict_side_resolution(&path, &change, side),
                        Message::ConflictSideResolutionReviewed,
                    );
                }
            }
            Event::ConflictSideResolutionReviewed(result) => {
                self.loading = false;
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                match result {
                    Ok(review) => {
                        let current = working_display(&review.working);
                        let target = working_display(&review.selected);
                        self.detail = format!("Review {:?} side for {}\nBlob: {}\nConfirming will replace or delete the current working file and stage only this path.\nDuring rebase, ours is rebased-onto; theirs is the replayed commit.\nConfigured Git checkout hooks remain active.\n\nCurrent working version:\n{}\n\nSelected index version:\n{}", review.preview.side, review.preview.change.path.display(), review.preview.oid.as_deref().unwrap_or("(deleted)"), current, target);
                        self.side_resolution_review = Some(std::sync::Arc::new(*review));
                    }
                    Err(error) => self.detail = format!("Unable to review side choice: {error}"),
                }
            }
            Event::ApplyConflictSideResolution if !self.loading => {
                if let Some(review) = self.side_resolution_review.take() {
                    self.loading = true;
                    self.notice = String::from("Applying confirmed side choice…");
                    return worker(
                        move || desktop_git::apply_conflict_side_resolution(&review),
                        |result| Message::Wrote(Operation::ResolveConflict, result),
                    );
                }
            }
            Event::CancelConflictSideResolution if !self.loading => {
                self.side_resolution_review = None
            }
            Event::ConflictSidePreviewed(result) => {
                self.loading = false;
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.detail = match result {
                    Ok(preview) => {
                        let side = match preview.side {
                            ConflictSide::Ours => "Ours — index stage 2",
                            ConflictSide::Theirs => "Theirs — index stage 3",
                        };
                        let content = match preview.content {
                            ConflictSideContent::Deleted => {
                                String::from("This side deletes the file.")
                            }
                            ConflictSideContent::Binary { size } => {
                                format!("Binary blob: {size} bytes. Text preview unavailable.")
                            }
                            ConflictSideContent::Text { preview, truncated } => format!(
                                "{preview}{}",
                                if truncated {
                                    "\n[Index-side preview truncated at 64 KiB]"
                                } else {
                                    ""
                                }
                            ),
                        };
                        format!("{}\n{}\nBlob: {}\nExecutable: {}\nRead-only preview. No resolution applied.\nDuring rebase, ours is the rebased-onto side; theirs is the replayed commit.\n\n{}", preview.change.path.display(), side, preview.oid.as_deref().unwrap_or("(deleted)"), preview.executable, content)
                    }
                    Err(error) => format!("Unable to inspect conflict side: {error}"),
                };
            }
            Event::ConflictReviewed(result) => {
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.loading = false;
                match result {
                    Ok(review) => {
                        let review = *review;
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
                        self.side_resolution_review = None;
                        self.hunk_review = None;
                        self.detail = format!("Unable to review conflict: {error}");
                    }
                }
            }
            Event::ResolveConflict if !self.loading => {
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
            Event::Stage(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    return self.dispatch_write(snapshot.repository.path.clone(), change, true);
                }
            }
            Event::Unstage(change) if !self.loading => {
                if let Some(snapshot) = &self.snapshot {
                    return self.dispatch_write(snapshot.repository.path.clone(), change, false);
                }
            }
            Event::SummaryChanged(value) if !self.loading => self.summary = value,
            Event::DescriptionChanged(value) if !self.loading => self.description = value,
            Event::Commit if !self.loading && !self.summary.trim().is_empty() => {
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
            Event::ReviewHunks(..)
            | Event::StageHunk(..)
            | Event::CancelHunks
            | Event::Select(..)
            | Event::ReviewConflict(..)
            | Event::ReviewConflictSide(..)
            | Event::RequestConflictSideResolution(..)
            | Event::ApplyConflictSideResolution
            | Event::CancelConflictSideResolution
            | Event::ResolveConflict
            | Event::Stage(..)
            | Event::Unstage(..)
            | Event::SummaryChanged(..)
            | Event::DescriptionChanged(..)
            | Event::Commit => {}
        }
        Task::none()
    }
}

impl Desktop {
    pub(crate) fn dispatch_write(
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

fn working_display(content: &ConflictWorkingContent) -> String {
    match content {
        ConflictWorkingContent::Deleted => String::from("(file deleted)"),
        ConflictWorkingContent::File { bytes, executable } => {
            let preview = if bytes.contains(&0) {
                String::from("Binary data; contents not rendered")
            } else {
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]),
                    if bytes.len() > 64 * 1024 {
                        "\n[Preview truncated]"
                    } else {
                        ""
                    }
                )
            };
            format!(
                "{} bytes; executable: {}\n{}",
                bytes.len(),
                executable,
                preview
            )
        }
    }
}
