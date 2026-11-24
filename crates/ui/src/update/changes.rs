//! Guarded changes state transitions.
use crate::{
    worker, ConflictContent, ConflictReview, ConflictSide, ConflictSideContent,
    ConflictSidePreview, Desktop, FileChange, FileDiff, Message, Operation,
};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    Select(FileChange),
    DiffLoaded(Result<FileDiff, String>),
    ReviewConflict(FileChange),
    ReviewConflictSide(FileChange, ConflictSide),
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
                    self.detail = format!("Loading {}…", change.path.display());
                    return worker(
                        move || desktop_git::diff(&path, &change),
                        Message::DiffLoaded,
                    );
                }
            }
            Event::DiffLoaded(result) => {
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
                    self.loading = true;
                    self.detail = String::from("Loading read-only index side…");
                    return worker(
                        move || desktop_git::inspect_conflict_side(&path, &change, side),
                        Message::ConflictSidePreviewed,
                    );
                }
            }
            Event::ConflictSidePreviewed(result) => {
                self.loading = false;
                self.conflict_review = None;
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
            Event::Select(..)
            | Event::ReviewConflict(..)
            | Event::ReviewConflictSide(..)
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
