//! Guarded application state transitions.
use crate::{capture, worker, Desktop, GitRuntime, Message, Panel};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    MoveFocus(crate::keyboard::FocusDirection),
    PanelSelected(Panel),
    CheckGitRuntime,
    GitRuntimeLoaded(Result<GitRuntime, String>),
    CaptureFrame,
    Captured(iced::window::Screenshot),
}

impl Desktop {
    pub(super) fn update_application(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::MoveFocus(direction) => {
                return iced::advanced::widget::operate(crate::keyboard::focus(direction))
            }
            Event::PanelSelected(panel) if !self.loading => {
                self.delete_branch_pending = None;
                self.panel = panel;
                self.show_history = panel == Panel::History;
                self.show_stashes = panel == Panel::Stashes;
                self.show_remote_settings = panel == Panel::Remotes;
                self.abort_merge_pending = false;
                self.abort_rebase_pending = false;
                self.remote_remove_pending = false;
                if matches!(panel, Panel::Changes | Panel::History) {
                    return self.update(Message::ShowHistory(panel == Panel::History));
                }
                self.conflict_review = None;
                self.side_resolution_review = None;
                self.hunk_review = None;
                self.selected_hunks.clear();
                self.selected_lines.clear();
                self.merge_preview = None;
                self.rebase_preview = None;
            }
            Event::CheckGitRuntime if !self.checking_git => {
                self.checking_git = true;
                self.git_runtime = None;
                return worker(desktop_git::git_runtime, Message::GitRuntimeLoaded);
            }
            Event::GitRuntimeLoaded(result) => {
                self.checking_git = false;
                self.git_runtime = Some(result);
            }
            Event::CaptureFrame => {
                self.capture_frames += 1;
                if self.capture_navigation_focus && self.capture_frames <= 2 {
                    return self.update(Message::MoveFocus(crate::keyboard::FocusDirection::Next));
                }
                if self.capture_frames == 4 {
                    return iced::window::get_latest()
                        .and_then(iced::window::screenshot)
                        .map(Message::Captured);
                }
            }
            Event::Captured(screenshot) => {
                if let Some(path) = self.capture.take() {
                    match capture::save(&path, &screenshot) {
                        Ok(()) => self
                            .capture_success
                            .store(true, std::sync::atomic::Ordering::SeqCst),
                        Err(error) => {
                            #[expect(
                                clippy::print_stderr,
                                reason = "A developer capture failure must be reported before the app exits"
                            )]
                            {
                                eprintln!("Capture failed: {error}");
                            }
                        }
                    }
                }
                return iced::exit();
            }
            Event::PanelSelected(..) | Event::CheckGitRuntime => {}
        }
        Task::none()
    }
}
