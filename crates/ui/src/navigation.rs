//! Focusable sidebar navigation; Git action buttons keep their existing handlers.
use crate::{Message, Panel};
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::keyboard::{key, Key};
use iced::{event, Border, Color, Element, Event, Length, Rectangle, Size, Theme, Vector};

pub(super) fn button<'a, R>(
    label: &'a str,
    panel: Panel,
    active: bool,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    let mut inner: iced::widget::Button<'a, Message, Theme, R> = iced::widget::button(label)
        .width(Length::Fill)
        .style(if active {
            iced::widget::button::primary
        } else {
            iced::widget::button::secondary
        });
    if enabled {
        inner = inner.on_press(Message::PanelSelected(panel));
    }
    crate::focus_region::region(Element::new(Navigation {
        inner: inner.into(),
        panel,
        enabled,
    }))
}

struct Navigation<'a, R> {
    inner: Element<'a, Message, Theme, R>,
    panel: Panel,
    enabled: bool,
}

#[derive(Debug)]
struct State {
    panel: Panel,
    focused: bool,
    armed: Option<key::Named>,
}

impl State {
    fn new(panel: Panel) -> Self {
        Self {
            panel,
            focused: false,
            armed: None,
        }
    }
}

impl widget::operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
        self.armed = None;
    }
    fn unfocus(&mut self) {
        self.focused = false;
        self.armed = None;
    }
}

impl<R: iced::advanced::text::Renderer> Widget<Message, Theme, R> for Navigation<'_, R> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(self.panel))
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(self.inner.as_widget())]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.panel != self.panel || !self.enabled {
            *state = State::new(self.panel);
        }
        tree.diff_children(std::slice::from_ref(&self.inner));
    }
    fn size(&self) -> Size<Length> {
        self.inner.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.inner.as_widget().size_hint()
    }
    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &R,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        if self.enabled {
            operation.focusable(state, None);
        } else {
            widget::operation::Focusable::unfocus(state);
        }
        self.inner
            .as_widget()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        use widget::operation::Focusable as _;
        let state = tree.state.downcast_mut::<State>();
        if state.panel != self.panel || !self.enabled {
            *state = State::new(self.panel);
        }
        match &event {
            Event::Window(iced::window::Event::Unfocused) => state.unfocus(),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if self.enabled && cursor.is_over(layout.bounds()) {
                    state.focus();
                } else {
                    state.unfocus();
                }
            }
            Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => {
                if self.enabled && layout.bounds().contains(*position) {
                    state.focus();
                } else {
                    state.unfocus();
                }
            }
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers))
                if !modifiers.is_empty() =>
            {
                state.armed = None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if state.focused && self.enabled =>
            {
                if let Key::Named(name @ (key::Named::Enter | key::Named::Space)) = key {
                    if modifiers.is_empty() {
                        if state.armed.is_none() {
                            state.armed = Some(*name);
                        }
                        return event::Status::Captured;
                    }
                }
                state.armed = None;
            }
            Event::Keyboard(iced::keyboard::Event::KeyReleased {
                key: Key::Named(name @ (key::Named::Enter | key::Named::Space)),
                modifiers,
                ..
            }) if state.focused && self.enabled => {
                if state.armed.take() == Some(*name) && modifiers.is_empty() {
                    shell.publish(Message::PanelSelected(self.panel));
                }
                return event::Status::Captured;
            }
            _ => {}
        }
        self.inner.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut R,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if self.enabled && tree.state.downcast_ref::<State>().focused {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        color: theme.palette().text,
                        width: 2.0,
                        radius: 3.0.into(),
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }
    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> mouse::Interaction {
        self.inner.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, R>> {
        self.inner
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}

#[cfg(test)]
mod tests {
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
}
