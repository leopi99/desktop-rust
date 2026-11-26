//! Unhandled Tab navigation between text inputs and the existing capture clock.
use crate::{Desktop, Message};
use iced::advanced::widget::{operation, Id, Operation};
use iced::keyboard::{key, Key, Modifiers};
use iced::{Rectangle, Subscription};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusDirection {
    Next,
    Previous,
}

pub(super) fn subscription(app: &Desktop) -> Subscription<Message> {
    let capture =
        if app.capture.is_some() && !app.loading && !app.checking_git && app.capture_frames < 4 {
            iced::window::frames().map(|_| Message::CaptureFrame)
        } else {
            Subscription::none()
        };
    Subscription::batch([iced::keyboard::on_key_press(key_pressed), capture])
}

fn key_pressed(key: Key, modifiers: Modifiers) -> Option<Message> {
    if key != Key::Named(key::Named::Tab) {
        return None;
    }
    match modifiers {
        Modifiers::SHIFT => Some(Message::MoveFocus(FocusDirection::Previous)),
        modifiers if modifiers.is_empty() => Some(Message::MoveFocus(FocusDirection::Next)),
        _ => None,
    }
}

// Count the current rendered tree on every request: no cached field indices or
// stale widget IDs survive navigation. Wrap without an unfocused intermediate step.
pub(super) fn focus(direction: FocusDirection) -> impl Operation<Message> {
    let next: fn(operation::focusable::Count) -> FocusAt = match direction {
        FocusDirection::Next => |count| FocusAt {
            target: count
                .focused
                .and_then(|n| n.checked_add(1))
                .filter(|n| *n < count.total)
                .unwrap_or(0),
            current: 0,
        },
        FocusDirection::Previous => |count| FocusAt {
            target: count
                .focused
                .and_then(|n| n.checked_sub(1))
                .unwrap_or_else(|| count.total.saturating_sub(1)),
            current: 0,
        },
    };
    operation::then(operation::focusable::count(), next)
}

#[derive(Debug)]
struct FocusAt {
    target: usize,
    current: usize,
}

impl Operation<Message> for FocusAt {
    fn container(
        &mut self,
        _id: Option<&Id>,
        _bounds: Rectangle,
        children: &mut dyn FnMut(&mut dyn Operation<Message>),
    ) {
        children(self);
    }

    fn focusable(&mut self, state: &mut dyn operation::Focusable, _id: Option<&Id>) {
        if self.current == self.target {
            if !state.is_focused() {
                state.focus();
            }
        } else {
            state.unfocus();
        }
        self.current = self.current.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{layout, subscription, widget::Tree, Layout};
    use iced::event::Status;
    use iced::futures::{executor::block_on, stream, StreamExt};
    use iced::widget::{button, column, text_input};
    use iced::{Element, Event, Size, Theme};

    fn press(key: Key, modifiers: Modifiers, status: Status) -> subscription::Event {
        subscription::Event::Interaction {
            window: iced::window::Id::unique(),
            event: Event::Keyboard(iced::keyboard::Event::KeyPressed {
                modified_key: key.clone(),
                key,
                physical_key: key::Physical::Code(key::Code::Tab),
                location: iced::keyboard::Location::Standard,
                modifiers,
                text: None,
            }),
            status,
        }
    }

    fn messages(app: &Desktop, events: Vec<subscription::Event>) -> Vec<Message> {
        subscription::into_recipes(super::subscription(app))
            .into_iter()
            .flat_map(|recipe| {
                block_on(
                    recipe
                        .stream(Box::pin(stream::iter(events.clone())))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    fn form(names: &[&'static str]) -> Element<'static, Message, Theme, ()> {
        let mut fields = column![];
        for name in names {
            fields = fields.push(text_input(name, name).on_input(Message::SummaryChanged));
        }
        fields
            .push(button("Commit").on_press(Message::Commit))
            .into()
    }

    fn apply(view: &Element<'_, Message, Theme, ()>, tree: &mut Tree, direction: FocusDirection) {
        let node = view.as_widget().layout(
            tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(960.0, 640.0)),
        );
        let mut operation: Box<dyn Operation<Message>> = Box::new(focus(direction));
        for _ in 0..4 {
            view.as_widget().operate(
                tree,
                Layout::new(&node),
                &(),
                &mut operation::black_box(&mut *operation),
            );
            match operation.finish() {
                operation::Outcome::None => return,
                operation::Outcome::Chain(next) => operation = next,
                operation::Outcome::Some(_) => {
                    panic!("Focus must not emit a write or edit message")
                }
            }
        }
        panic!("Focus traversal did not finish");
    }

    fn focused(tree: &Tree, count: usize) -> Vec<usize> {
        (0..count)
            .filter(|n| {
                tree.children[*n]
                    .state
                    .downcast_ref::<text_input::State<()>>()
                    .is_focused()
            })
            .collect()
    }

    #[test]
    fn tab_and_shift_tab_cycle_through_real_text_inputs_without_activating_buttons() {
        let view = form(&["path", "summary", "description"]);
        let mut tree = Tree::new(view.as_widget());
        for (modifiers, expected) in [
            (Modifiers::SHIFT, 2),
            (Modifiers::empty(), 0),
            (Modifiers::empty(), 1),
            (Modifiers::empty(), 2),
            (Modifiers::empty(), 0),
            (Modifiers::SHIFT, 2),
            (Modifiers::SHIFT, 1),
        ] {
            let events = messages(
                &Desktop::default(),
                vec![press(
                    Key::Named(key::Named::Tab),
                    modifiers,
                    Status::Ignored,
                )],
            );
            assert_eq!(events.len(), 1);
            let Message::MoveFocus(direction) = events[0] else {
                panic!("Unexpected action for Tab");
            };
            apply(&view, &mut tree, direction);
            assert_eq!(focused(&tree, 3), vec![expected]);
        }
    }

    #[test]
    fn bindings_ignore_captured_keys_releases_modifiers_and_text_editing() {
        let mut events = vec![press(
            Key::Named(key::Named::Tab),
            Modifiers::empty(),
            Status::Captured,
        )];
        for bits in 0..16 {
            let mut modifiers = Modifiers::empty();
            for (bit, flag) in [
                Modifiers::SHIFT,
                Modifiers::CTRL,
                Modifiers::ALT,
                Modifiers::LOGO,
            ]
            .into_iter()
            .enumerate()
            {
                if bits & (1 << bit) != 0 {
                    modifiers |= flag;
                }
            }
            events.push(press(
                Key::Named(key::Named::Tab),
                modifiers,
                Status::Ignored,
            ));
            for key in [
                Key::Named(key::Named::Enter),
                Key::Named(key::Named::Escape),
                Key::Character("è".into()),
                Key::Character("v".into()),
            ] {
                events.push(press(key, modifiers, Status::Ignored));
            }
        }
        events.push(subscription::Event::Interaction {
            window: iced::window::Id::unique(),
            event: Event::Keyboard(iced::keyboard::Event::KeyReleased {
                key: Key::Named(key::Named::Tab),
                location: iced::keyboard::Location::Standard,
                modifiers: Modifiers::empty(),
            }),
            status: Status::Ignored,
        });
        let result = messages(&Desktop::default(), events);
        assert_eq!(result.len(), 2);
        assert!(matches!(
            result[0],
            Message::MoveFocus(FocusDirection::Next)
        ));
        assert!(matches!(
            result[1],
            Message::MoveFocus(FocusDirection::Previous)
        ));
    }

    #[test]
    fn tab_subscription_survives_busy_runtime_checks_and_capture_clock_states() {
        for (loading, checking_git, frames) in [
            (false, false, 0),
            (true, false, 0),
            (false, true, 0),
            (false, false, 4),
        ] {
            let app = Desktop {
                loading,
                checking_git,
                capture: Some("unused.png".into()),
                capture_frames: frames,
                ..Desktop::default()
            };
            let result = messages(
                &app,
                vec![press(
                    Key::Named(key::Named::Tab),
                    Modifiers::empty(),
                    Status::Ignored,
                )],
            );
            assert_eq!(result.len(), 1);
            assert!(matches!(
                result[0],
                Message::MoveFocus(FocusDirection::Next)
            ));
        }
    }

    #[test]
    fn empty_and_single_input_trees_keep_focus_and_cursor_stable() {
        let empty = form(&[]);
        let mut tree = Tree::new(empty.as_widget());
        apply(&empty, &mut tree, FocusDirection::Next);
        apply(&empty, &mut tree, FocusDirection::Previous);
        let view = form(&["path"]);
        let mut tree = Tree::new(view.as_widget());
        apply(&view, &mut tree, FocusDirection::Next);
        tree.children[0]
            .state
            .downcast_mut::<text_input::State<()>>()
            .move_cursor_to_front();
        for direction in [FocusDirection::Next, FocusDirection::Previous] {
            apply(&view, &mut tree, direction);
            assert_eq!(focused(&tree, 1), vec![0]);
            let cursor = tree.children[0]
                .state
                .downcast_ref::<text_input::State<()>>()
                .cursor();
            assert!(matches!(
                cursor.state(&text_input::Value::new("path")),
                text_input::cursor::State::Index(0)
            ));
        }
    }
}
