//! Focusable review selection; keyboard activation never applies a Git patch.
use crate::Message;
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::keyboard::{key, Key};
use iced::{event, Border, Color, Element, Event, Length, Rectangle, Size, Theme, Vector};

/// A selectable review coordinate; this never represents a Git write action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Hunk(usize),
    Line(crate::HunkLine),
}

impl Target {
    fn message(self, selected: bool) -> Message {
        match self {
            Self::Hunk(index) => Message::ToggleHunk(index, selected),
            Self::Line(line) => Message::ToggleHunkLine(line, selected),
        }
    }
}

pub(super) fn checkbox<'a, R>(
    label: impl Into<String>,
    review: &std::sync::Arc<crate::HunkReview>,
    target: Target,
    checked: bool,
    enabled: bool,
    monospace: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    let mut inner = iced::widget::checkbox(label, checked)
        .size(14)
        .width(Length::Fill);
    if monospace {
        inner = inner.font(iced::Font::MONOSPACE);
    }
    if enabled {
        inner = inner.on_toggle(move |selected| target.message(selected));
    }
    crate::focus_region::region(Element::new(Selection {
        inner: inner.into(),
        review: std::sync::Arc::clone(review),
        target,
        checked,
        enabled,
    }))
}

struct Selection<'a, R> {
    inner: Element<'a, Message, Theme, R>,
    review: std::sync::Arc<crate::HunkReview>,
    target: Target,
    checked: bool,
    enabled: bool,
}

#[derive(Debug)]
struct State {
    review: std::sync::Arc<crate::HunkReview>,
    target: Target,
    focused: bool,
    armed: Option<key::Named>,
}

impl State {
    fn new(review: &std::sync::Arc<crate::HunkReview>, target: Target) -> Self {
        Self {
            review: std::sync::Arc::clone(review),
            target,
            focused: false,
            armed: None,
        }
    }
    fn matches(&self, review: &std::sync::Arc<crate::HunkReview>, target: Target) -> bool {
        self.target == target && std::sync::Arc::ptr_eq(&self.review, review)
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

impl<R: iced::advanced::text::Renderer> Widget<Message, Theme, R> for Selection<'_, R> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(&self.review, self.target))
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(self.inner.as_widget())]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<State>();
        if !state.matches(&self.review, self.target) || !self.enabled {
            *state = State::new(&self.review, self.target);
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
        if !state.matches(&self.review, self.target) || !self.enabled {
            *state = State::new(&self.review, self.target);
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
                if let Key::Named(name @ key::Named::Space) = key {
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
                key: Key::Named(name @ key::Named::Space),
                modifiers,
                ..
            }) if state.focused && self.enabled => {
                if state.armed.take() == Some(*name) && modifiers.is_empty() {
                    shell.publish(self.target.message(!self.checked));
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
    use std::sync::Arc;

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

    fn review() -> Arc<crate::HunkReview> {
        crate::tests::hunk_app().hunk_review.unwrap()
    }

    fn control(
        review: &Arc<crate::HunkReview>,
        target: Target,
        checked: bool,
        enabled: bool,
    ) -> View {
        checkbox("Include", review, target, checked, enabled, false)
    }

    fn focused(review: &Arc<crate::HunkReview>) -> (View, widget::Tree) {
        let view = control(review, Target::Hunk(0), false, true);
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        (view, tree)
    }

    fn space(view: &mut View, tree: &mut widget::Tree, down: bool) -> Vec<Message> {
        send(
            view,
            tree,
            key(key::Named::Space, Modifiers::empty(), down),
            mouse::Cursor::Unavailable,
        )
        .1
    }

    #[test]
    fn traversal_includes_review_rows_and_skips_disabled_and_git_write_buttons() {
        let review = review();
        let line = crate::HunkLine { hunk: 0, line: 1 };
        let view: View = column![
            text_input("Path", "draft").on_input(Message::PathChanged),
            control(&review, Target::Hunk(0), false, true),
            control(&review, Target::Line(line), true, false),
            control(&review, Target::Line(line), false, true),
            iced::widget::button("Apply")
                .on_press(Message::ApplySelectedHunks(crate::HunkSide::Unstaged)),
        ]
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        for (direction, expected) in [
            (FocusDirection::Next, 0),
            (FocusDirection::Next, 1),
            (FocusDirection::Next, 2),
            (FocusDirection::Next, 0),
            (FocusDirection::Previous, 2),
            (FocusDirection::Previous, 1),
        ] {
            let _ = run(&view, &mut tree, focus(direction));
            let count = run(&view, &mut tree, operation::focusable::count()).unwrap();
            assert_eq!(count.total, 3);
            assert_eq!(count.focused, Some(expected));
        }
    }

    #[test]
    fn repeated_space_selects_once_and_only_updates_selection_and_preserves_drafts() {
        for target in [
            Target::Hunk(0),
            Target::Line(crate::HunkLine { hunk: 0, line: 1 }),
        ] {
            let mut app = crate::tests::hunk_app();
            app.summary = "keep summary".into();
            app.description = "keep description".into();
            let review = Arc::clone(app.hunk_review.as_ref().unwrap());
            let mut view = control(&review, target, false, true);
            let mut tree = widget::Tree::new(view.as_widget());
            let _ = run(&view, &mut tree, focus(FocusDirection::Next));
            assert!(space(&mut view, &mut tree, false).is_empty());
            for _ in 0..5 {
                assert!(space(&mut view, &mut tree, true).is_empty());
            }
            // Ordinary redraw keeps the exact review/coordinate identity.
            view = control(&review, target, false, true);
            tree.diff(view.as_widget());
            let messages = space(&mut view, &mut tree, false);
            assert_eq!(messages.len(), 1);
            for message in messages {
                assert!(matches!(
                    message,
                    Message::ToggleHunk(..) | Message::ToggleHunkLine(..)
                ));
                let _ = app.update(message);
            }
            match target {
                Target::Hunk(index) => assert!(app.selected_hunks.contains(&index)),
                Target::Line(line) => assert!(app.selected_lines.contains(&line)),
            }
            assert!(!app.loading);
            assert!(Arc::ptr_eq(app.hunk_review.as_ref().unwrap(), &review));
            assert_eq!(app.summary, "keep summary");
            assert_eq!(app.description, "keep description");
            assert!(space(&mut view, &mut tree, false).is_empty());
            view = control(&review, target, true, true);
            tree.diff(view.as_widget());
            assert!(space(&mut view, &mut tree, true).is_empty());
            for message in space(&mut view, &mut tree, false) {
                let _ = app.update(message);
            }
            assert!(app.selected_hunks.is_empty());
            assert!(app.selected_lines.is_empty());
            assert!(!app.loading);
        }
    }

    #[test]
    fn interrupted_press_cannot_toggle_another_review_coordinate_or_disabled_control() {
        for interruption in [
            "review",
            "coordinate",
            "disabled",
            "window",
            "modifier",
            "tab",
            "outside",
            "focus",
        ] {
            let review = review();
            let (mut view, mut tree) = focused(&review);
            assert!(space(&mut view, &mut tree, true).is_empty());
            match interruption {
                "review" => {
                    view = control(&Arc::new((*review).clone()), Target::Hunk(0), false, true)
                }
                "coordinate" => {
                    view = control(
                        &review,
                        Target::Line(crate::HunkLine { hunk: 0, line: 1 }),
                        false,
                        true,
                    )
                }
                "disabled" => view = control(&review, Target::Hunk(0), false, false),
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
                        operation::focusable::focus::<Message>(widget::Id::new("elsewhere")),
                    );
                }
                _ => unreachable!(),
            }
            tree.diff(view.as_widget());
            assert!(
                space(&mut view, &mut tree, false).is_empty(),
                "{interruption}"
            );
            if matches!(interruption, "review" | "coordinate" | "disabled") {
                assert!(!tree.state.downcast_ref::<State>().focused);
            }
        }
    }

    #[test]
    fn enter_modifiers_and_unfocused_space_never_toggle() {
        let review = review();
        for name in [key::Named::Enter, key::Named::Space] {
            for modifiers in [
                Modifiers::SHIFT,
                Modifiers::CTRL,
                Modifiers::ALT,
                Modifiers::LOGO,
            ] {
                let (mut view, mut tree) = focused(&review);
                for down in [true, false] {
                    assert!(send(
                        &mut view,
                        &mut tree,
                        key(name, modifiers, down),
                        mouse::Cursor::Unavailable
                    )
                    .1
                    .is_empty());
                }
            }
        }
        let (mut view, mut tree) = focused(&review);
        for down in [true, false] {
            assert!(send(
                &mut view,
                &mut tree,
                key(key::Named::Enter, Modifiers::empty(), down),
                mouse::Cursor::Unavailable
            )
            .1
            .is_empty());
        }
        let mut view = control(&review, Target::Hunk(0), false, true);
        let mut tree = widget::Tree::new(view.as_widget());
        assert!(space(&mut view, &mut tree, true).is_empty());
        assert!(space(&mut view, &mut tree, false).is_empty());
    }

    #[test]
    fn pointer_toggle_remains_single_and_late_busy_delivery_is_ignored() {
        let review = review();
        for touch in [false, true] {
            let mut view = control(&review, Target::Hunk(0), false, true);
            let mut tree = widget::Tree::new(view.as_widget());
            let node = layout(&view, &mut tree);
            let point = Layout::new(&node).bounds().center();
            let cursor = mouse::Cursor::Available(point);
            let event = if touch {
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: iced::touch::Finger(0),
                    position: point,
                })
            } else {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            };
            let (_, messages) = send(&mut view, &mut tree, event, cursor);
            assert_eq!(messages.len(), 1);
            assert!(tree.state.downcast_ref::<State>().focused);
            let mut app = crate::tests::hunk_app();
            app.loading = true;
            for message in messages {
                let _ = app.update(message);
            }
            assert!(app.selected_hunks.is_empty());
            assert!(app.hunk_review.is_some());
        }
    }
}
