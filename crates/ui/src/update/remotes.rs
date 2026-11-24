//! Guarded remotes state transitions.
use crate::{
    transfer_worker, worker, Desktop, Message, Operation, Remote, RemoteAction, RemoteEdit,
    RemoteUrlKind, TransferControl,
};
use iced::Task;

#[derive(Debug)]
pub(super) enum Event {
    RemoteSelected(Remote),
    RemoteAction(RemoteAction),
    RemoteNameChanged(String),
    RemoteUrlChanged(String),
    FetchUrlChanged(String),
    PushUrlChanged(String),
    ToggleRemoteSettings,
    RequestRemoveRemote,
    CancelRemoveRemote,
    ConfigureRemote(RemoteEdit),
    CancelTransfer,
    TransferProgress(u64, String),
}

impl Desktop {
    pub(super) fn update_remotes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::RemoteSelected(remote) if !self.loading => self.select_remote(Some(remote)),
            Event::RemoteAction(action) if !self.loading => {
                if let (Some(snapshot), Some(remote)) = (&self.snapshot, &self.selected_remote) {
                    let path = snapshot.repository.path.clone();
                    let remote = remote.clone();
                    let expected = snapshot.commit_basis.clone();
                    self.loading = true;
                    self.notice = format!("{action:?} in progress…");
                    let control = self.begin_transfer();
                    return transfer_worker(
                        control,
                        self.transfer_serial,
                        move |control| {
                            desktop_git::remote_operation_with_control(
                                &path, &remote, action, &expected, control,
                            )
                        },
                        |result| Message::Wrote(Operation::Remote, result),
                    );
                }
            }
            Event::RemoteNameChanged(value) if !self.loading => self.new_remote_name = value,
            Event::RemoteUrlChanged(value) if !self.loading => self.new_remote_url = value,
            Event::FetchUrlChanged(value) if !self.loading => {
                self.fetch_url = value;
                self.remote_remove_pending = false;
            }
            Event::PushUrlChanged(value) if !self.loading => {
                self.push_url = value;
                self.remote_remove_pending = false;
            }
            Event::ToggleRemoteSettings => self.show_remote_settings = !self.show_remote_settings,
            Event::RequestRemoveRemote if !self.loading && self.selected_remote.is_some() => {
                self.remote_remove_pending = true
            }
            Event::CancelRemoveRemote => self.remote_remove_pending = false,
            Event::ConfigureRemote(action) if !self.loading => {
                return self.configure_remote(action)
            }
            Event::CancelTransfer => {
                if let Some(control) = &self.transfer {
                    control.cancel();
                    self.notice = String::from("Cancellation requested… waiting for Git to stop");
                }
            }
            Event::TransferProgress(serial, output)
                if self.transfer.is_some() && serial == self.transfer_serial =>
            {
                self.transfer_progress = output.replace('\r', "\n");
            }
            Event::RemoteSelected(..)
            | Event::RemoteAction(..)
            | Event::RemoteNameChanged(..)
            | Event::RemoteUrlChanged(..)
            | Event::FetchUrlChanged(..)
            | Event::PushUrlChanged(..)
            | Event::RequestRemoveRemote
            | Event::ConfigureRemote(..)
            | Event::TransferProgress(..) => {}
        }
        Task::none()
    }
}

impl Desktop {
    pub(crate) fn remote_name(&self) -> &str {
        if self.new_remote_name.is_empty() {
            "origin"
        } else {
            &self.new_remote_name
        }
    }
}

impl Desktop {
    pub(crate) fn select_remote(&mut self, remote: Option<Remote>) {
        self.fetch_url = remote
            .as_ref()
            .and_then(|r| r.fetch_urls.first())
            .and_then(|u| u.to_str())
            .unwrap_or_default()
            .to_owned();
        self.push_url = remote
            .as_ref()
            .and_then(|r| r.push_urls.first())
            .and_then(|u| u.to_str())
            .unwrap_or_default()
            .to_owned();
        self.selected_remote = remote;
        self.remote_remove_pending = false;
    }
}

impl Desktop {
    pub(crate) fn configure_remote(&mut self, action: RemoteEdit) -> Task<Message> {
        let Some(snapshot) = &self.snapshot else {
            return Task::none();
        };
        if action == RemoteEdit::Remove && !self.remote_remove_pending {
            return Task::none();
        }
        let remote = self.selected_remote.clone();
        if action != RemoteEdit::Add && remote.is_none() {
            return Task::none();
        }
        let name = self.remote_name().to_owned();
        let url = match action {
            RemoteEdit::Add => self.new_remote_url.clone(),
            RemoteEdit::FetchUrl => self.fetch_url.clone(),
            RemoteEdit::PushUrl => self.push_url.clone(),
            RemoteEdit::Remove => String::new(),
        };
        if action != RemoteEdit::Remove && url.trim().is_empty() {
            return Task::none();
        }
        let path = snapshot.repository.path.clone();
        self.remote_remove_pending = false;
        self.loading = true;
        self.notice = String::from("Updating remote configuration…");
        worker(
            move || {
                if action == RemoteEdit::Add {
                    return desktop_git::add_remote(&path, &name, &url);
                }
                let remote =
                    remote.ok_or_else(|| std::io::Error::other("Select a remote first"))?;
                match action {
                    RemoteEdit::FetchUrl => {
                        desktop_git::set_remote_url(&path, &remote, RemoteUrlKind::Fetch, &url)
                    }
                    RemoteEdit::PushUrl => {
                        desktop_git::set_remote_url(&path, &remote, RemoteUrlKind::Push, &url)
                    }
                    RemoteEdit::Remove => desktop_git::remove_remote(&path, &remote),
                    RemoteEdit::Add => unreachable!(),
                }
            },
            move |result| Message::Wrote(Operation::RemoteConfig(action), result),
        )
    }
}

impl Desktop {
    pub(crate) fn begin_transfer(&mut self) -> TransferControl {
        let control = TransferControl::default();
        self.transfer = Some(control.clone());
        self.transfer_serial = self.transfer_serial.wrapping_add(1);
        self.transfer_progress.clear();
        control
    }
}

impl Desktop {
    pub(crate) fn finish_transfer(&mut self) {
        if let Some(control) = self.transfer.take() {
            self.transfer_progress = control.progress().replace('\r', "\n");
        }
    }
}
