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
    ToggleHunk(usize, bool),
    ToggleHunkLine(crate::HunkLine, bool),
    ApplySelectedHunks(crate::HunkSide),
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

#[derive(Debug)]
enum PatchSelection {
    Hunks(Vec<usize>),
    Lines(Vec<crate::HunkLine>),
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
                    self.selected_hunks.clear();
                    self.selected_lines.clear();
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
                    self.selected_hunks.clear();
                    self.selected_lines.clear();
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
                self.selected_hunks.clear();
                self.selected_lines.clear();
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
                        return self.dispatch_hunk_selection(review, vec![index], side);
                    }
                }
            }
            Event::ToggleHunk(index, selected)
                if !self.loading && self.panel == crate::Panel::Changes =>
            {
                if self
                    .hunk_review
                    .as_ref()
                    .is_some_and(|review| index < review.hunks.len())
                {
                    self.selected_lines.retain(|line| line.hunk != index);
                    if selected {
                        self.selected_hunks.insert(index);
                    } else {
                        self.selected_hunks.remove(&index);
                    }
                }
            }
            Event::ToggleHunkLine(line, selected)
                if !self.loading && self.panel == crate::Panel::Changes =>
            {
                if let Some(review) = &self.hunk_review {
                    if review.line_selection_available() {
                        if let Some(hunk) = review.hunks.get(line.hunk) {
                            let changed =
                                hunk.split_inclusive('\n')
                                    .nth(line.line)
                                    .is_some_and(|row| {
                                        matches!(row.as_bytes().first(), Some(b'+' | b'-'))
                                    });
                            if changed {
                                let full = self.selected_hunks.contains(&line.hunk);
                                if full && !selected {
                                    self.selected_hunks.remove(&line.hunk);
                                    for (index, row) in hunk.split_inclusive('\n').enumerate() {
                                        if matches!(row.as_bytes().first(), Some(b'+' | b'-')) {
                                            self.selected_lines.insert(crate::HunkLine {
                                                hunk: line.hunk,
                                                line: index,
                                            });
                                        }
                                    }
                                }
                                if !full || !selected {
                                    if selected {
                                        self.selected_lines.insert(line);
                                    } else {
                                        self.selected_lines.remove(&line);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Event::ApplySelectedHunks(side)
                if !self.loading && self.panel == crate::Panel::Changes =>
            {
                if self.hunk_review.as_ref().is_some_and(|review| {
                    review.side == side
                        && (!self.selected_hunks.is_empty() || !self.selected_lines.is_empty())
                        && self
                            .selected_hunks
                            .iter()
                            .all(|index| *index < review.hunks.len())
                }) {
                    if let Some(review) = self.hunk_review.take() {
                        let selection = if self.selected_lines.is_empty() {
                            PatchSelection::Hunks(self.selected_hunks.iter().copied().collect())
                        } else {
                            PatchSelection::Lines(self.combined_hunk_lines(&review))
                        };
                        return self.dispatch_patch_selection(review, selection, side);
                    }
                }
            }
            Event::CancelHunks if !self.loading => {
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
            }
            Event::DiffLoaded(result) => {
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
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
                    self.selected_hunks.clear();
                    self.selected_lines.clear();
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
                    self.selected_hunks.clear();
                    self.selected_lines.clear();
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
                    self.selected_hunks.clear();
                    self.selected_lines.clear();
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
                self.selected_hunks.clear();
                self.selected_lines.clear();
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
                self.selected_hunks.clear();
                self.selected_lines.clear();
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
                self.selected_hunks.clear();
                self.selected_lines.clear();
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
                        self.selected_hunks.clear();
                        self.selected_lines.clear();
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
            Event::ToggleHunkLine(..)
            | Event::ToggleHunk(..)
            | Event::ApplySelectedHunks(..)
            | Event::ReviewHunks(..)
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
    pub(crate) fn combined_hunk_lines(&self, review: &crate::HunkReview) -> Vec<crate::HunkLine> {
        let mut rows = self.selected_lines.clone();
        for hunk in &self.selected_hunks {
            if let Some(body) = review.hunks.get(*hunk) {
                for (line, row) in body.split_inclusive('\n').enumerate() {
                    if matches!(row.as_bytes().first(), Some(b'+' | b'-')) {
                        rows.insert(crate::HunkLine { hunk: *hunk, line });
                    }
                }
            }
        }
        rows.into_iter().collect()
    }

    fn dispatch_hunk_selection(
        &mut self,
        review: std::sync::Arc<crate::HunkReview>,
        indices: Vec<usize>,
        side: crate::HunkSide,
    ) -> Task<Message> {
        self.dispatch_patch_selection(review, PatchSelection::Hunks(indices), side)
    }

    fn dispatch_patch_selection(
        &mut self,
        review: std::sync::Arc<crate::HunkReview>,
        selection: PatchSelection,
        side: crate::HunkSide,
    ) -> Task<Message> {
        self.selected_hunks.clear();
        self.selected_lines.clear();
        self.loading = true;
        self.notice = format!("Updating reviewed {side:?} selection…");
        worker(
            move || match (side, selection) {
                (crate::HunkSide::Unstaged, PatchSelection::Hunks(indices)) => {
                    desktop_git::stage_hunks(&review, &indices)
                }
                (crate::HunkSide::Staged, PatchSelection::Hunks(indices)) => {
                    desktop_git::unstage_hunks(&review, &indices)
                }
                (crate::HunkSide::Unstaged, PatchSelection::Lines(lines)) => {
                    desktop_git::stage_lines(&review, &lines)
                }
                (crate::HunkSide::Staged, PatchSelection::Lines(lines)) => {
                    desktop_git::unstage_lines(&review, &lines)
                }
            },
            |result| Message::Wrote(Operation::Index, result),
        )
    }

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
