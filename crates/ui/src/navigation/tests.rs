use super::*;
use crate::keyboard::{focus, FocusDirection};
use iced::advanced::{clipboard, widget::operation};
use iced::keyboard::Modifiers;
use iced::widget::{column, text_input};
use iced::Point;

type View = Element<'static, Message, Theme, ()>;

fn layout(view: &View, tree: &mut widget::Tree) -> layout::Node {
    view.as_widget().layout(
        tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(960.0, 640.0)),
    )
}

fn run<T: Send + 'static>(
    view: &View,
    tree: &mut widget::Tree,
    op: impl widget::Operation<T> + 'static,
) -> Option<T> {
    let node = layout(view, tree);
    let mut op: Box<dyn widget::Operation<T>> = Box::new(op);
    for _ in 0..4 {
        view.as_widget().operate(
            tree,
            Layout::new(&node),
            &(),
            &mut operation::black_box(&mut *op),
        );
        match op.finish() {
            operation::Outcome::None => return None,
            operation::Outcome::Some(value) => return Some(value),
            operation::Outcome::Chain(next) => op = next,
        }
    }
    panic!("Widget operation did not finish");
}

fn send(
    view: &mut View,
    tree: &mut widget::Tree,
    event: Event,
    cursor: mouse::Cursor,
) -> (event::Status, Vec<Message>) {
    let node = layout(view, tree);
    let mut messages = Vec::new();
    let status = view.as_widget_mut().on_event(
        tree,
        event,
        Layout::new(&node),
        cursor,
        &(),
        &mut clipboard::Null,
        &mut Shell::new(&mut messages),
        &Rectangle::with_size(Size::new(960.0, 640.0)),
    );
    (status, messages)
}

fn key(name: key::Named, modifiers: Modifiers, down: bool) -> Event {
    let key = Key::Named(name);
    Event::Keyboard(if down {
        iced::keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: key::Physical::Code(key::Code::Enter),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
        }
    } else {
        iced::keyboard::Event::KeyReleased {
            key,
            location: iced::keyboard::Location::Standard,
            modifiers,
        }
    })
}

fn focused_button() -> (View, widget::Tree) {
    let view = button("Branches", Panel::Branches, false, true);
    let mut tree = widget::Tree::new(view.as_widget());
    assert!(run(&view, &mut tree, focus(FocusDirection::Next)).is_none());
    (view, tree)
}

#[test]
fn mixed_form_traversal_includes_enabled_navigation_and_skips_disabled_actions() {
    let view: View = column![
        text_input("Path", "draft").on_input(Message::PathChanged),
        button("Changes", Panel::Changes, true, true),
        button("Disabled", Panel::Branches, false, false),
        button("Git", Panel::Git, false, true),
        text_input("Summary", "keep").on_input(Message::SummaryChanged),
        iced::widget::button("Commit").on_press(Message::Commit),
    ]
    .into();
    let mut tree = widget::Tree::new(view.as_widget());
    for (direction, expected) in [
        (FocusDirection::Next, 0),
        (FocusDirection::Next, 1),
        (FocusDirection::Next, 2),
        (FocusDirection::Next, 3),
        (FocusDirection::Next, 0),
        (FocusDirection::Previous, 3),
        (FocusDirection::Previous, 2),
        (FocusDirection::Previous, 1),
    ] {
        assert!(run(&view, &mut tree, focus(direction)).is_none());
        let count = run(&view, &mut tree, operation::focusable::count()).unwrap();
        assert_eq!(count.total, 4);
        assert_eq!(count.focused, Some(expected));
        assert!(!tree.children[2].state.downcast_ref::<State>().focused);
    }
}

#[test]
fn enter_and_space_activate_once_on_release_despite_repeated_keypresses() {
    for name in [key::Named::Enter, key::Named::Space] {
        let (mut view, mut tree) = focused_button();
        assert!(send(
            &mut view,
            &mut tree,
            key(name, Modifiers::empty(), false),
            mouse::Cursor::Unavailable
        )
        .1
        .is_empty());
        for _ in 0..5 {
            let (status, messages) = send(
                &mut view,
                &mut tree,
                key(name, Modifiers::empty(), true),
                mouse::Cursor::Unavailable,
            );
            assert_eq!(status, event::Status::Captured);
            assert!(messages.is_empty());
        }
        let (_, messages) = send(
            &mut view,
            &mut tree,
            key(name, Modifiers::empty(), false),
            mouse::Cursor::Unavailable,
        );
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            messages[0],
            Message::PanelSelected(Panel::Branches)
        ));
        assert!(send(
            &mut view,
            &mut tree,
            key(name, Modifiers::empty(), false),
            mouse::Cursor::Unavailable
        )
        .1
        .is_empty());
    }
}

#[test]
fn interrupted_presses_do_not_activate_after_focus_loss_disable_or_retarget() {
    for interruption in [
        "window", "modifier", "tab", "outside", "focus", "disabled", "retarget",
    ] {
        let (mut view, mut tree) = focused_button();
        let _ = send(
            &mut view,
            &mut tree,
            key(key::Named::Enter, Modifiers::empty(), true),
            mouse::Cursor::Unavailable,
        );
        match interruption {
            "window" => {
                let _ = send(
                    &mut view,
                    &mut tree,
                    Event::Window(iced::window::Event::Unfocused),
                    mouse::Cursor::Unavailable,
                );
            }
            "modifier" => {
                let _ = send(
                    &mut view,
                    &mut tree,
                    Event::Keyboard(iced::keyboard::Event::ModifiersChanged(Modifiers::CTRL)),
                    mouse::Cursor::Unavailable,
                );
            }
            "tab" => {
                let _ = send(
                    &mut view,
                    &mut tree,
                    key(key::Named::Tab, Modifiers::empty(), true),
                    mouse::Cursor::Unavailable,
                );
            }
            "outside" => {
                let _ = send(
                    &mut view,
                    &mut tree,
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    mouse::Cursor::Available(Point::new(-1.0, -1.0)),
                );
            }
            "focus" => {
                let _ = run(
                    &view,
                    &mut tree,
                    operation::focusable::focus::<Message>(widget::Id::new("another-field")),
                );
            }
            "disabled" => {
                view = button("Branches", Panel::Branches, false, false);
                tree.diff(view.as_widget());
            }
            "retarget" => {
                view = button("Git", Panel::Git, false, true);
                tree.diff(view.as_widget());
            }
            _ => unreachable!(),
        }
        assert!(
            send(
                &mut view,
                &mut tree,
                key(key::Named::Enter, Modifiers::empty(), false),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty(),
            "{interruption}"
        );
        if matches!(interruption, "disabled" | "retarget") {
            assert!(!tree.state.downcast_ref::<State>().focused);
        }
    }
}

#[test]
fn modifier_combinations_and_unfocused_keys_do_not_navigate() {
    for name in [
        key::Named::Enter,
        key::Named::Space,
        key::Named::Escape,
        key::Named::Tab,
    ] {
        for modifiers in [
            Modifiers::SHIFT,
            Modifiers::CTRL,
            Modifiers::ALT,
            Modifiers::LOGO,
            Modifiers::SHIFT | Modifiers::CTRL,
        ] {
            let (mut view, mut tree) = focused_button();
            assert!(send(
                &mut view,
                &mut tree,
                key(name, modifiers, true),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty());
            assert!(send(
                &mut view,
                &mut tree,
                key(name, modifiers, false),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty());
        }
        let mut view = button("Branches", Panel::Branches, false, true);
        let mut tree = widget::Tree::new(view.as_widget());
        assert!(send(
            &mut view,
            &mut tree,
            key(name, Modifiers::empty(), true),
            mouse::Cursor::Unavailable
        )
        .1
        .is_empty());
        assert!(send(
            &mut view,
            &mut tree,
            key(name, Modifiers::empty(), false),
            mouse::Cursor::Unavailable
        )
        .1
        .is_empty());
    }
}

#[test]
fn pointer_and_touch_clicks_keep_the_existing_single_navigation_message() {
    for touch in [false, true] {
        let mut view = button("Branches", Panel::Branches, false, true);
        let mut tree = widget::Tree::new(view.as_widget());
        let node = layout(&view, &mut tree);
        let point = Layout::new(&node).bounds().center();
        let cursor = mouse::Cursor::Available(point);
        let (press, release) = if touch {
            (
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: iced::touch::Finger(0),
                    position: point,
                }),
                Event::Touch(iced::touch::Event::FingerLifted {
                    id: iced::touch::Finger(0),
                    position: point,
                }),
            )
        } else {
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            )
        };
        assert!(send(&mut view, &mut tree, press, cursor).1.is_empty());
        let (_, messages) = send(&mut view, &mut tree, release, cursor);
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            messages[0],
            Message::PanelSelected(Panel::Branches)
        ));
        assert!(tree.state.downcast_ref::<State>().focused);
    }
}

#[test]
fn same_target_redraw_preserves_focus_but_busy_delivery_still_hits_the_app_guard() {
    let (mut view, mut tree) = focused_button();
    let _ = send(
        &mut view,
        &mut tree,
        key(key::Named::Space, Modifiers::empty(), true),
        mouse::Cursor::Unavailable,
    );
    view = button("Branches", Panel::Branches, true, true);
    tree.diff(view.as_widget());
    let (_, messages) = send(
        &mut view,
        &mut tree,
        key(key::Named::Space, Modifiers::empty(), false),
        mouse::Cursor::Unavailable,
    );
    assert_eq!(messages.len(), 1);
    let mut app = crate::Desktop {
        loading: true,
        summary: "keep summary".into(),
        description: "keep description".into(),
        ..crate::Desktop::default()
    };
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(app.panel, Panel::Changes);
    assert!(app.loading);
    assert_eq!(app.summary, "keep summary");
    assert_eq!(app.description, "keep description");
}

fn changed_file() -> crate::FileChange {
    crate::tests::hunk_app().hunk_review.unwrap().change.clone()
}

fn file_control(
    repository: &std::path::Path,
    change: &crate::FileChange,
    action: FileAction,
    enabled: bool,
) -> View {
    file_button("Read file", repository, change, action, enabled)
}

#[test]
fn file_and_review_controls_join_traversal_without_git_write_buttons() {
    let change = changed_file();
    let root = std::path::Path::new("repository");
    let view: View = column![
        text_input("Path", "draft").on_input(Message::PathChanged),
        button("Changes", Panel::Changes, true, true),
        file_control(root, &change, FileAction::Diff, true),
        file_control(
            root,
            &change,
            FileAction::Hunks(crate::HunkSide::Staged),
            false
        ),
        file_control(
            root,
            &change,
            FileAction::Hunks(crate::HunkSide::Unstaged),
            true
        ),
        iced::widget::button("Stage").on_press(Message::Stage(change.clone())),
    ]
    .into();
    let mut tree = widget::Tree::new(view.as_widget());
    for (direction, expected) in [
        (FocusDirection::Next, 0),
        (FocusDirection::Next, 1),
        (FocusDirection::Next, 2),
        (FocusDirection::Next, 3),
        (FocusDirection::Next, 0),
        (FocusDirection::Previous, 3),
        (FocusDirection::Previous, 2),
    ] {
        let _ = run(&view, &mut tree, focus(direction));
        let count = run(&view, &mut tree, operation::focusable::count()).unwrap();
        assert_eq!(count.total, 4);
        assert_eq!(count.focused, Some(expected));
    }
}

fn assert_file_message(message: Message, expected: FileAction, change: &crate::FileChange) {
    let (actual, received) = match message {
        Message::Select(change) => (FileAction::Diff, change),
        Message::ReviewHunks(change) => (FileAction::Hunks(crate::HunkSide::Unstaged), change),
        Message::ReviewStagedHunks(change) => (FileAction::Hunks(crate::HunkSide::Staged), change),
        Message::ReviewConflict(change) => (FileAction::Conflict, change),
        Message::ReviewConflictSide(change, side) => (FileAction::ConflictSide(side), change),
        other => panic!("Read-only control emitted {other:?}"),
    };
    assert_eq!(actual, expected);
    assert_eq!(&received, change);
}

#[test]
fn every_read_action_emits_once_on_release_and_redraw_preserves_same_target() {
    let change = changed_file();
    let root = std::path::Path::new("repository");
    for action in [
        FileAction::Diff,
        FileAction::Hunks(crate::HunkSide::Unstaged),
        FileAction::Hunks(crate::HunkSide::Staged),
        FileAction::Conflict,
        FileAction::ConflictSide(crate::ConflictSide::Ours),
        FileAction::ConflictSide(crate::ConflictSide::Theirs),
    ] {
        for name in [key::Named::Enter, key::Named::Space] {
            let mut view = file_control(root, &change, action, true);
            let mut tree = widget::Tree::new(view.as_widget());
            let _ = run(&view, &mut tree, focus(FocusDirection::Next));
            for _ in 0..5 {
                assert!(send(
                    &mut view,
                    &mut tree,
                    key(name, Modifiers::empty(), true),
                    mouse::Cursor::Unavailable
                )
                .1
                .is_empty());
            }
            view = file_control(root, &change, action, true);
            tree.diff(view.as_widget());
            let (_, messages) = send(
                &mut view,
                &mut tree,
                key(name, Modifiers::empty(), false),
                mouse::Cursor::Unavailable,
            );
            assert_eq!(messages.len(), 1);
            for message in messages {
                assert_file_message(message, action, &change);
            }
            assert!(send(
                &mut view,
                &mut tree,
                key(name, Modifiers::empty(), false),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty());
        }
    }
}

#[test]
fn file_identity_changes_cancel_armed_activation_without_redirecting_to_a_replacement() {
    for replacement in [
        "repository",
        "path",
        "origin",
        "index",
        "worktree",
        "action",
        "panel",
        "disabled",
    ] {
        let change = changed_file();
        let root = std::path::Path::new("repository");
        let mut view = file_control(root, &change, FileAction::Diff, true);
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let _ = send(
            &mut view,
            &mut tree,
            key(key::Named::Enter, Modifiers::empty(), true),
            mouse::Cursor::Unavailable,
        );
        let mut changed = change.clone();
        let mut repository = root.to_path_buf();
        let mut action = FileAction::Diff;
        match replacement {
            "repository" => repository = "other-repository".into(),
            "path" => changed.path = "another-file".into(),
            "origin" => changed.original_path = Some("old-file".into()),
            "index" => changed.index = 'M',
            "worktree" => changed.worktree = 'D',
            "action" => action = FileAction::Hunks(crate::HunkSide::Unstaged),
            "panel" | "disabled" => {}
            _ => unreachable!(),
        }
        view = if replacement == "panel" {
            button("Changes", Panel::Changes, true, true)
        } else {
            file_control(&repository, &changed, action, replacement != "disabled")
        };
        tree.diff(view.as_widget());
        assert!(!tree.state.downcast_ref::<State>().focused, "{replacement}");
        assert!(
            send(
                &mut view,
                &mut tree,
                key(key::Named::Enter, Modifiers::empty(), false),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty(),
            "{replacement}"
        );
    }
}

#[test]
fn file_pointer_and_keyboard_keep_native_path_bytes_and_existing_busy_guard() {
    let change = changed_file();
    #[cfg(unix)]
    let change = {
        let mut change = change;
        use std::os::unix::ffi::OsStringExt as _;
        change.path = std::ffi::OsString::from_vec(b"literal-\xff\nfile".to_vec()).into();
        change.original_path = Some(std::ffi::OsString::from_vec(b"old-\xfe".to_vec()).into());
        change
    };
    let root = std::path::Path::new("repository");
    for touch in [false, true] {
        let mut view = file_control(root, &change, FileAction::Diff, true);
        let mut tree = widget::Tree::new(view.as_widget());
        let node = layout(&view, &mut tree);
        let point = Layout::new(&node).bounds().center();
        let cursor = mouse::Cursor::Available(point);
        let (press, release) = if touch {
            (
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: iced::touch::Finger(0),
                    position: point,
                }),
                Event::Touch(iced::touch::Event::FingerLifted {
                    id: iced::touch::Finger(0),
                    position: point,
                }),
            )
        } else {
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            )
        };
        assert!(send(&mut view, &mut tree, press, cursor).1.is_empty());
        let (_, messages) = send(&mut view, &mut tree, release, cursor);
        assert_eq!(messages.len(), 1);
        for message in messages {
            assert_file_message(message, FileAction::Diff, &change);
        }
        let _ = send(
            &mut view,
            &mut tree,
            key(key::Named::Enter, Modifiers::empty(), true),
            mouse::Cursor::Unavailable,
        );
        let (_, messages) = send(
            &mut view,
            &mut tree,
            key(key::Named::Enter, Modifiers::empty(), false),
            mouse::Cursor::Unavailable,
        );
        assert_eq!(messages.len(), 1);
        let mut app = crate::tests::hunk_app();
        app.loading = true;
        app.summary = "keep summary".into();
        app.description = "keep description".into();
        let reviewed = app.hunk_review.clone();
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(app.hunk_review, reviewed);
        assert_eq!(app.summary, "keep summary");
        assert_eq!(app.description, "keep description");
    }
}

#[test]
fn file_activation_uses_existing_review_invalidation_without_clearing_drafts() {
    let mut app = crate::tests::hunk_app();
    app.summary = "keep summary".into();
    app.description = "keep description".into();
    app.selected_hunks.insert(0);
    let change = app.hunk_review.as_ref().unwrap().change.clone();
    let mut view = file_control(
        std::path::Path::new("repository"),
        &change,
        FileAction::Diff,
        true,
    );
    let mut tree = widget::Tree::new(view.as_widget());
    let _ = run(&view, &mut tree, focus(FocusDirection::Next));
    let _ = send(
        &mut view,
        &mut tree,
        key(key::Named::Space, Modifiers::empty(), true),
        mouse::Cursor::Unavailable,
    );
    let (_, messages) = send(
        &mut view,
        &mut tree,
        key(key::Named::Space, Modifiers::empty(), false),
        mouse::Cursor::Unavailable,
    );
    assert_eq!(messages.len(), 1);
    for message in messages {
        let _ = app.update(message);
    }
    assert!(app.loading);
    assert!(app.hunk_review.is_none());
    assert!(app.selected_hunks.is_empty());
    assert_eq!(app.summary, "keep summary");
    assert_eq!(app.description, "keep description");
}

mod review;
