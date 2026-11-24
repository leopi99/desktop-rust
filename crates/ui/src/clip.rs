//! Enforce a renderer layer for panel bounds, including software-rendered text.
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::{event, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

pub(super) fn clip<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(Clip(content.into()))
}

struct Clip<'a, Message>(Element<'a, Message>);

// This wrapper is transparent to layout, state, focus operations, and overlays.
impl<Message> Widget<Message, Theme, Renderer> for Clip<'_, Message> {
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
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.0.as_widget().layout(tree, renderer, limits)
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        if let Some(bounds) = layout.bounds().intersection(viewport) {
            renderer.with_layer(bounds, |renderer| {
                self.0
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &bounds);
            });
        }
    }
    fn operate(
        &self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.0
            .as_widget()
            .operate(tree, layout, renderer, operation);
    }
    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.0
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.0
            .as_widget_mut()
            .overlay(tree, layout, renderer, translation)
    }
}
