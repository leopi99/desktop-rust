//! Guarded stashes state transitions.
use crate::{worker, Desktop, Message, Operation, StashEntry};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    ToggleStashes,
    StashSelected(StashEntry),
    StashMessageChanged(String),
    SaveStash,
    ApplyStash,
}

impl Desktop {
    pub(super) fn update_stashes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::ToggleStashes => self.show_stashes = !self.show_stashes,
            Event::StashSelected(stash) if !self.loading => self.selected_stash = Some(stash),
            Event::StashMessageChanged(value) if !self.loading => self.stash_message = value,
            Event::SaveStash if !self.loading => {
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
            Event::ApplyStash if !self.loading => {
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
            Event::StashSelected(..)
            | Event::StashMessageChanged(..)
            | Event::SaveStash
            | Event::ApplyStash => {}
        }
        Task::none()
    }
}
