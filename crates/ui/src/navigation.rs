//! Focusable panel/file navigation and read-only review controls.
//! Git writes retain their existing separate handlers.
use crate::{Message, Panel};
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::keyboard::{key, Key};
use iced::{event, Border, Color, Element, Event, Length, Rectangle, Size, Theme, Vector};

/// Read-only actions offered for an observed changed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileAction {
    Diff,
    Hunks(crate::HunkSide),
    Conflict,
    ConflictSide(crate::ConflictSide),
}

/// Dismissal is tied to the exact immutable review displayed by this control.
#[derive(Debug, Clone)]
pub(super) enum ReviewTarget {
    Hunks(std::sync::Arc<crate::HunkReview>),
    ConflictChoice(std::sync::Arc<crate::ConflictSideResolutionReview>),
}

impl PartialEq for ReviewTarget {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Hunks(left), Self::Hunks(right)) => std::sync::Arc::ptr_eq(left, right),
            (Self::ConflictChoice(left), Self::ConflictChoice(right)) => {
                std::sync::Arc::ptr_eq(left, right)
            }
            _ => false,
        }
    }
}
impl Eq for ReviewTarget {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Panel(Panel),
    Review(ReviewTarget),
    File(std::sync::Arc<FileTarget>),
}

#[derive(Debug, PartialEq, Eq)]
struct FileTarget {
    repository: std::path::PathBuf,
    change: crate::FileChange,
    action: FileAction,
}

impl Target {
    fn message(&self) -> Message {
        match self {
            Self::Panel(panel) => Message::PanelSelected(*panel),
            Self::Review(ReviewTarget::Hunks(_)) => Message::CancelHunks,
            Self::Review(ReviewTarget::ConflictChoice(_)) => Message::CancelConflictSideResolution,
            Self::File(target) => match target.action {
                FileAction::Diff => Message::Select(target.change.clone()),
                FileAction::Hunks(crate::HunkSide::Unstaged) => {
                    Message::ReviewHunks(target.change.clone())
                }
                FileAction::Hunks(crate::HunkSide::Staged) => {
                    Message::ReviewStagedHunks(target.change.clone())
                }
                FileAction::Conflict => Message::ReviewConflict(target.change.clone()),
                FileAction::ConflictSide(side) => {
                    Message::ReviewConflictSide(target.change.clone(), side)
                }
            },
        }
    }
}

pub(super) fn file_button<'a, R>(
    label: impl Into<String>,
    repository: &std::path::Path,
    change: &crate::FileChange,
    action: FileAction,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    let target = Target::File(std::sync::Arc::new(FileTarget {
        repository: repository.to_path_buf(),
        change: change.clone(),
        action,
    }));
    read_button(label, target, enabled)
}

pub(super) fn file_row<'a, R>(
    label: String,
    status: String,
    repository: &std::path::Path,
    change: &crate::FileChange,
    active: bool,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    let target = Target::File(std::sync::Arc::new(FileTarget {
        repository: repository.to_path_buf(),
        change: change.clone(),
        action: FileAction::Diff,
    }));
    let contents = iced::widget::row![
        iced::widget::text(status).width(30),
        iced::widget::text(label)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(4);
    let mut inner = iced::widget::button(contents)
        .width(Length::Fill)
        .padding([9, 12])
        .style(move |theme, status| crate::appearance::file(theme, status, active));
    if enabled {
        inner = inner.on_press(target.message());
    }
    crate::focus_region::region(Element::new(Navigation {
        inner: inner.into(),
        target,
        enabled,
        focus_color: if active {
            Color::WHITE
        } else {
            crate::appearance::BLUE
        },
    }))
}

pub(super) fn toolbar<'a, R>(
    caption: &'a str,
    value: String,
    panel: Panel,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    let content = iced::widget::column![
        iced::widget::text(caption)
            .size(11)
            .color(Color::from_rgb8(175, 184, 193)),
        iced::widget::text(value)
            .size(14)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(3)
    .width(Length::Fill);
    let mut inner = iced::widget::button(content)
        .width(Length::Fill)
        .height(56)
        .padding([8, 16])
        .style(crate::appearance::toolbar_button);
    if enabled {
        inner = inner.on_press(Message::PanelSelected(panel));
    }
    crate::focus_region::region(Element::new(Navigation {
        inner: inner.into(),
        target: Target::Panel(panel),
        enabled,
        focus_color: Color::WHITE,
    }))
}

pub(super) fn review_button<'a, R>(
    label: impl Into<String>,
    review: ReviewTarget,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    read_button(label, Target::Review(review), enabled)
}

fn read_button<'a, R>(
    label: impl Into<String>,
    target: Target,
    enabled: bool,
) -> Element<'a, Message, Theme, R>
where
    R: iced::advanced::text::Renderer + 'a,
{
    let mut inner = iced::widget::button(iced::widget::text(label.into()))
        .style(iced::widget::button::secondary);
    if enabled {
        inner = inner.on_press(target.message());
    }
    crate::focus_region::region(Element::new(Navigation {
        inner: inner.into(),
        target,
        enabled,
        focus_color: crate::appearance::BLUE,
    }))
}

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
        .padding([10, 12])
        .style(move |theme, status| crate::appearance::tab(theme, status, active));
    if enabled {
        inner = inner.on_press(Message::PanelSelected(panel));
    }
    crate::focus_region::region(Element::new(Navigation {
        inner: inner.into(),
        target: Target::Panel(panel),
        enabled,
        focus_color: crate::appearance::BLUE,
    }))
}

struct Navigation<'a, R> {
    inner: Element<'a, Message, Theme, R>,
    target: Target,
    enabled: bool,
    focus_color: Color,
}

#[derive(Debug)]
struct State {
    target: Target,
    focused: bool,
    armed: Option<key::Named>,
}

impl State {
    fn new(target: Target) -> Self {
        Self {
            target,
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
        widget::tree::State::new(State::new(self.target.clone()))
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(self.inner.as_widget())]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.target != self.target || !self.enabled {
            *state = State::new(self.target.clone());
            // The inner button owns pointer/touch press state. A release must not
            // activate a replacement target that merely reused this tree slot.
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
        if state.target != self.target || !self.enabled {
            *state = State::new(self.target.clone());
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
                    shell.publish(self.target.message());
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
                        color: self.focus_color,
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
mod tests;
