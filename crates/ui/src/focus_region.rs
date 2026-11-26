//! Transparent bounds metadata for keyboard focus inside scrollable widgets.
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Size, Theme, Vector};

const MARKER: &str = "desktop-rust.focus-region";

pub(super) fn is_region(id: Option<&widget::Id>) -> bool {
    id == Some(&widget::Id::new(MARKER))
}

pub(super) fn region<'a, M: 'a, R: renderer::Renderer + 'a>(
    content: impl Into<Element<'a, M, Theme, R>>,
) -> Element<'a, M, Theme, R> {
    Element::new(Region(content.into()))
}

struct Region<'a, M, R>(Element<'a, M, Theme, R>);

impl<M, R: renderer::Renderer> Widget<M, Theme, R> for Region<'_, M, R> {
    fn size(&self) -> Size<Length> {
        self.0.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.0.as_widget().size_hint()
    }
    fn tag(&self) -> widget::tree::Tag {
        self.0.as_widget().tag()
    }
    fn state(&self) -> widget::tree::State {
        self.0.as_widget().state()
    }
    fn children(&self) -> Vec<widget::Tree> {
        self.0.as_widget().children()
    }
    fn diff(&self, tree: &mut widget::Tree) {
        self.0.as_widget().diff(tree);
    }
    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &R,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.0.as_widget().layout(tree, renderer, limits)
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
        self.0
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(
            Some(&widget::Id::new(MARKER)),
            layout.bounds(),
            &mut |operation| {
                self.0
                    .as_widget()
                    .operate(tree, layout, renderer, operation)
            },
        );
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
        self.0.as_widget_mut().on_event(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.0
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        translation: Vector,
    ) -> Option<overlay::Element<'a, M, Theme, R>> {
        self.0
            .as_widget_mut()
            .overlay(tree, layout, renderer, translation)
    }
}
