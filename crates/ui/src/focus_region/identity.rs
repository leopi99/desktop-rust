//! Prevent a reused widget position from transferring input state to another field.
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Theme, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldId {
    RepositoryPath,
    CommitSummary,
    CommitDescription,
    HistoryBranchName,
    BranchCreate,
    BranchRename,
    RemoteName,
    RemoteNewUrl,
    RemoteFetchUrl,
    RemotePushUrl,
    StashDescription,
    InitDestination,
    InitBranch,
    CloneSource,
    CloneDestination,
}

pub(crate) fn field<'a, M: 'a, R: renderer::Renderer + 'a>(
    id: FieldId,
    content: impl Into<Element<'a, M, Theme, R>>,
) -> Element<'a, M, Theme, R> {
    super::region(Element::new(Field {
        id,
        inner: content.into(),
    }))
}

struct Field<'a, M, R> {
    id: FieldId,
    inner: Element<'a, M, Theme, R>,
}

#[derive(Debug)]
struct State(FieldId);

impl<M, R: renderer::Renderer> Widget<M, Theme, R> for Field<'_, M, R> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State(self.id))
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(self.inner.as_widget())]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        if tree.state.downcast_ref::<State>().0 != self.id {
            tree.state = widget::tree::State::new(State(self.id));
            tree.children = self.children();
        } else {
            tree.diff_children(std::slice::from_ref(&self.inner));
        }
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
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn widget::Operation,
    ) {
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
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) -> event::Status {
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
    ) -> Option<overlay::Element<'a, M, Theme, R>> {
        self.inner
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::{focus, FocusDirection};
    use crate::Message;
    use iced::advanced::{clipboard, widget::operation};
    use iced::keyboard::{key, Key, Modifiers};
    use iced::widget::{column, scrollable, text_input, Space};

    type View = Element<'static, Message, Theme, ()>;

    fn input(id: FieldId, value: &'static str, edit: fn(String) -> Message) -> View {
        field(id, text_input("Field", value).on_input(edit))
    }

    fn layout(view: &View, tree: &mut widget::Tree) -> layout::Node {
        view.as_widget().layout(
            tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(500.0, 300.0)),
        )
    }

    fn run<T: Send + 'static>(
        view: &View,
        tree: &mut widget::Tree,
        op: impl widget::Operation<T> + 'static,
    ) -> Option<T> {
        let node = layout(view, tree);
        let mut op: Box<dyn widget::Operation<T>> = Box::new(op);
        for _ in 0..5 {
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
        panic!("Field focus operation did not finish");
    }

    fn type_character(view: &mut View, tree: &mut widget::Tree) -> Vec<Message> {
        let node = layout(view, tree);
        let mut messages = Vec::new();
        let key = Key::Character("è".into());
        let event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: key::Physical::Code(key::Code::KeyE),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::empty(),
            text: Some("è".into()),
        });
        let _ = view.as_widget_mut().on_event(
            tree,
            event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &(),
            &mut clipboard::Null,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(Size::new(500.0, 300.0)),
        );
        messages
    }

    fn focused(view: &View, tree: &mut widget::Tree) -> Option<usize> {
        run(view, tree, operation::focusable::count())
            .unwrap()
            .focused
    }

    #[test]
    fn changing_a_field_purpose_drops_focus_before_typing_can_edit_the_new_target() {
        let mut view = input(
            FieldId::CommitSummary,
            "summary draft",
            Message::SummaryChanged,
        );
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        assert_eq!(focused(&view, &mut tree), Some(0));
        view = input(
            FieldId::InitDestination,
            "project draft",
            Message::InitDestinationChanged,
        );
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), None);
        assert!(type_character(&mut view, &mut tree).is_empty());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let messages = type_character(&mut view, &mut tree);
        assert_eq!(messages.len(), 1);
        assert!(
            matches!(&messages[0],Message::InitDestinationChanged(value) if value == "project draftè")
        );
    }

    #[test]
    fn same_field_redraw_keeps_focus_and_selection_and_edits_only_its_draft() {
        let mut view = input(
            FieldId::CommitSummary,
            "keep draft",
            Message::SummaryChanged,
        );
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        tree.children[0]
            .state
            .downcast_mut::<text_input::State<()>>()
            .select_all();
        view = field(
            FieldId::CommitSummary,
            text_input("New label", "keep draft").on_input(Message::SummaryChanged),
        );
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), Some(0));
        let selection = tree.children[0]
            .state
            .downcast_ref::<text_input::State<()>>()
            .cursor()
            .selection(&text_input::Value::new("keep draft"));
        assert_eq!(selection, Some((0, 10)));
        let messages = type_character(&mut view, &mut tree);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0],Message::SummaryChanged(value) if value == "è"));
    }

    #[test]
    fn reordered_fields_do_not_migrate_cursor_or_focus_to_another_purpose() {
        let mut view: View = column![
            input(FieldId::CommitSummary, "summary", Message::SummaryChanged),
            input(
                FieldId::CommitDescription,
                "description",
                Message::DescriptionChanged
            )
        ]
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        assert_eq!(focused(&view, &mut tree), Some(1));
        view = column![
            input(
                FieldId::CommitDescription,
                "description",
                Message::DescriptionChanged
            ),
            input(FieldId::CommitSummary, "summary", Message::SummaryChanged)
        ]
        .into();
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), None);
        assert!(type_character(&mut view, &mut tree).is_empty());
        let _ = run(&view, &mut tree, focus(FocusDirection::Previous));
        let messages = type_character(&mut view, &mut tree);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0],Message::SummaryChanged(value) if value == "summaryè"));
    }

    #[test]
    fn shared_branch_draft_contexts_reset_but_the_common_repository_field_keeps_focus() {
        let mut view = input(
            FieldId::HistoryBranchName,
            "shared draft",
            Message::BranchNameChanged,
        );
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        view = input(
            FieldId::BranchCreate,
            "shared draft",
            Message::BranchNameChanged,
        );
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), None);
        assert!(type_character(&mut view, &mut tree).is_empty());
        let mut view: View = column![
            input(FieldId::RepositoryPath, "repo", Message::PathChanged),
            input(FieldId::CommitSummary, "summary", Message::SummaryChanged)
        ]
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        view = column![
            input(FieldId::RepositoryPath, "repo", Message::PathChanged),
            input(FieldId::CloneSource, "url", Message::CloneSourceChanged)
        ]
        .into();
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), Some(0));
        let messages = type_character(&mut view, &mut tree);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0],Message::PathChanged(value) if value == "repoè"));
    }

    #[derive(Debug, Default)]
    struct ScrollPosition(iced::Vector);
    impl widget::Operation<iced::Vector> for ScrollPosition {
        fn container(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            children: &mut dyn FnMut(&mut dyn widget::Operation<iced::Vector>),
        ) {
            children(self);
        }
        fn scrollable(
            &mut self,
            _state: &mut dyn operation::Scrollable,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _content: Rectangle,
            translation: iced::Vector,
        ) {
            self.0 = translation;
        }
        fn finish(&self) -> operation::Outcome<iced::Vector> {
            operation::Outcome::Some(self.0)
        }
    }

    #[test]
    fn changing_identity_keeps_scroll_offsets_and_explicit_refocus_still_reveals_the_field() {
        let form = |id, edit| -> View {
            scrollable(column![
                Space::with_height(300),
                input(id, "destination", edit),
                Space::with_height(40)
            ])
            .height(90)
            .width(200)
            .into()
        };
        let mut view = form(
            FieldId::InitDestination,
            Message::InitDestinationChanged as fn(String) -> Message,
        );
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let before = run(&view, &mut tree, ScrollPosition::default()).unwrap();
        assert!(before.y > 0.0);
        view = form(FieldId::CloneDestination, Message::CloneDestinationChanged);
        tree.diff(view.as_widget());
        assert_eq!(focused(&view, &mut tree), None);
        assert_eq!(
            run(&view, &mut tree, ScrollPosition::default()).unwrap(),
            before
        );
        assert!(type_character(&mut view, &mut tree).is_empty());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        assert_eq!(
            run(&view, &mut tree, ScrollPosition::default()).unwrap(),
            before
        );
        let messages = type_character(&mut view, &mut tree);
        assert!(
            matches!(&messages[0],Message::CloneDestinationChanged(value) if value == "destinationè")
        );
    }
}
