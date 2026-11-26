//! Application chrome shared by the workspace panels.
use crate::{appearance, clip::clip, Desktop, Message, Panel};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

impl Desktop {
    pub(super) fn shell<'a>(
        &'a self,
        title: &'static str,
        body: Element<'a, Message>,
        transfer_controls: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let repository_name = self
            .snapshot
            .as_ref()
            .map(|s| {
                s.repository
                    .path
                    .file_name()
                    .unwrap_or(s.repository.path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| "Open a repository".into());
        let branch_name = self
            .snapshot
            .as_ref()
            .and_then(|s| s.commit_basis.branch.as_ref())
            .map(|b| {
                b.to_string_lossy()
                    .trim_start_matches("refs/heads/")
                    .to_owned()
            })
            .unwrap_or_else(|| "No branch selected".into());
        let toolbar = row![
            container(clip(crate::navigation::toolbar(
                "Current repository",
                repository_name,
                Panel::Repository,
                !self.loading
            )))
            .width(appearance::SIDEBAR),
            container(clip(crate::navigation::toolbar(
                "Current branch",
                branch_name,
                Panel::Branches,
                !self.loading
            )))
            .width(Length::Fill),
            container(clip(crate::navigation::toolbar(
                "Remote connections",
                "Fetch, pull and push".into(),
                Panel::Remotes,
                !self.loading
            )))
            .width(240),
        ]
        .height(56);
        let mut workspace = column![].height(Length::Fill).width(Length::Fill);
        if self.panel != Panel::Changes {
            workspace = workspace.push(
                container(row![
                    container(row![
                        crate::navigation::button("Changes", Panel::Changes, false, !self.loading),
                        crate::navigation::button(
                            "History",
                            Panel::History,
                            self.panel == Panel::History,
                            !self.loading
                        ),
                    ])
                    .width(appearance::SIDEBAR),
                    container(text(title).size(16)).padding([10, 16]),
                ])
                .width(Length::Fill)
                .style(appearance::sidebar),
            );
        }
        if self.snapshot.as_ref().is_some_and(|s| {
            s.commit_basis.merge_heads.is_some() || s.commit_basis.rebase.is_some()
        }) {
            let mut manage = button("Manage operation").style(button::secondary);
            if !self.loading {
                manage = manage.on_press(Message::PanelSelected(Panel::Branches));
            }
            workspace = workspace.push(
                container(
                    row![
                        text("Merge or rebase in progress"),
                        Space::with_width(Length::Fill),
                        manage
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([8, 12])
                .width(Length::Fill)
                .style(appearance::sidebar),
            );
        }
        workspace = workspace.push(
            if matches!(self.panel, Panel::Changes | Panel::History) && self.snapshot.is_some() {
                container(body).height(Length::Fill)
            } else {
                container(body).padding(20).height(Length::Fill)
            },
        );
        if !self.notice.is_empty() {
            workspace = workspace.push(
                container(scrollable(text(&self.notice)).height(52))
                    .padding(8)
                    .width(Length::Fill)
                    .style(appearance::sidebar),
            );
        }
        workspace = workspace.push(transfer_controls);
        let footer = container(
            row![
                text(if self.loading {
                    "Working…"
                } else {
                    "Desktop Rust · Development preview"
                })
                .size(11)
                .color(appearance::MUTED),
                Space::with_width(Length::Fill),
                container(crate::navigation::button(
                    "Saved changes",
                    Panel::Stashes,
                    self.panel == Panel::Stashes,
                    !self.loading
                ))
                .width(120),
                container(crate::navigation::button(
                    "Create / Clone",
                    Panel::Repository,
                    self.panel == Panel::Repository,
                    !self.loading
                ))
                .width(120),
                container(crate::navigation::button(
                    "Git",
                    Panel::Git,
                    self.panel == Panel::Git,
                    !self.loading
                ))
                .width(60),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([0, 12])
        .width(Length::Fill)
        .style(appearance::sidebar);
        column![toolbar, workspace, footer]
            .height(Length::Fill)
            .into()
    }
}
