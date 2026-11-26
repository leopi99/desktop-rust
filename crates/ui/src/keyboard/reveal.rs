//! Minimal top-left scroll adjustments for the focused region's ancestor path.
use crate::Message;
use iced::advanced::widget::{operation, Id, Operation};
use iced::{Rectangle, Vector};

#[derive(Debug, Clone, Copy)]
pub(super) struct Scroll {
    pub index: usize,
    pub bounds: Rectangle,
    pub content: Rectangle,
    pub translation: Vector,
}

#[derive(Debug, Clone)]
pub(super) struct Target {
    pub bounds: Rectangle,
    pub ancestors: Vec<Scroll>,
}

impl Target {
    pub(super) fn reveal(&self) -> operation::Outcome<Message> {
        let mut bounds = self.bounds;
        let mut adjustments = Vec::new();
        if !valid(bounds) {
            return operation::Outcome::None;
        }
        for scroll in self.ancestors.iter().rev() {
            if !valid(scroll.bounds)
                || !valid(scroll.content)
                || !scroll.translation.x.is_finite()
                || !scroll.translation.y.is_finite()
            {
                return operation::Outcome::None;
            }
            let offset = Vector::new(
                axis(
                    bounds.x,
                    bounds.width,
                    scroll.bounds.x,
                    scroll.bounds.width,
                    scroll.content.width,
                    scroll.translation.x,
                ),
                axis(
                    bounds.y,
                    bounds.height,
                    scroll.bounds.y,
                    scroll.bounds.height,
                    scroll.content.height,
                    scroll.translation.y,
                ),
            );
            if offset != scroll.translation {
                adjustments.push((scroll.index, offset));
            }
            bounds = bounds - offset;
        }
        if adjustments.is_empty() {
            operation::Outcome::None
        } else {
            operation::Outcome::Chain(Box::new(Reveal {
                adjustments,
                current: 0,
            }))
        }
    }
}

fn valid(bounds: Rectangle) -> bool {
    [bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|n| n.is_finite())
        && bounds.width >= 0.0
        && bounds.height >= 0.0
}

fn axis(start: f32, size: f32, viewport: f32, extent: f32, content: f32, old: f32) -> f32 {
    let visible = start - old;
    let offset = if visible < viewport || size > extent {
        start - viewport
    } else if visible + size > viewport + extent {
        start + size - viewport - extent
    } else {
        old
    };
    offset.clamp(0.0, (content - extent).max(0.0))
}

#[derive(Debug)]
struct Reveal {
    adjustments: Vec<(usize, Vector)>,
    current: usize,
}

impl Operation<Message> for Reveal {
    fn container(
        &mut self,
        _id: Option<&Id>,
        _bounds: Rectangle,
        children: &mut dyn FnMut(&mut dyn Operation<Message>),
    ) {
        children(self);
    }
    fn scrollable(
        &mut self,
        state: &mut dyn operation::Scrollable,
        _id: Option<&Id>,
        _bounds: Rectangle,
        _content: Rectangle,
        _translation: Vector,
    ) {
        if let Some((_, offset)) = self
            .adjustments
            .iter()
            .find(|(index, _)| *index == self.current)
        {
            state.scroll_to(operation::scrollable::AbsoluteOffset {
                x: offset.x,
                y: offset.y,
            });
        }
        self.current = self.current.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus_region::region;
    use crate::keyboard::{focus, FocusDirection};
    use iced::advanced::{layout, widget, Layout};
    use iced::widget::{column, row, scrollable, text_input, Space};
    use iced::{Element, Size, Theme};

    type View = Element<'static, Message, Theme, ()>;

    #[derive(Debug, Clone, Default)]
    struct Snapshot {
        scrolls: Vec<(Option<Id>, Scroll)>,
        focused: Option<Id>,
        bounds: Option<Rectangle>,
        region: Option<Rectangle>,
    }

    impl Operation<Snapshot> for Snapshot {
        fn container(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            children: &mut dyn FnMut(&mut dyn Operation<Snapshot>),
        ) {
            let old = self.region;
            if crate::focus_region::is_region(id) {
                self.region = Some(bounds);
            }
            children(self);
            self.region = old;
        }
        fn focusable(&mut self, state: &mut dyn operation::Focusable, id: Option<&Id>) {
            if state.is_focused() {
                self.focused = id.cloned();
                self.bounds = self.region;
            }
        }
        fn scrollable(
            &mut self,
            _state: &mut dyn operation::Scrollable,
            id: Option<&Id>,
            bounds: Rectangle,
            content: Rectangle,
            translation: Vector,
        ) {
            self.scrolls.push((
                id.cloned(),
                Scroll {
                    index: self.scrolls.len(),
                    bounds,
                    content,
                    translation,
                },
            ));
        }
        fn finish(&self) -> operation::Outcome<Snapshot> {
            operation::Outcome::Some(self.clone())
        }
    }

    fn run<T: Send + 'static>(
        view: &View,
        tree: &mut widget::Tree,
        op: impl Operation<T> + 'static,
    ) -> Option<T> {
        let node = view.as_widget().layout(
            tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(500.0, 300.0)),
        );
        let mut op: Box<dyn Operation<T>> = Box::new(op);
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
        panic!("Focus/reveal operation did not finish");
    }
    fn snapshot(view: &View, tree: &mut widget::Tree) -> Snapshot {
        run(view, tree, Snapshot::default()).unwrap()
    }
    fn input(name: &'static str) -> View {
        region(
            text_input(name, name)
                .id(name)
                .on_input(Message::SummaryChanged),
        )
    }
    fn visible(bounds: Rectangle, scroll: Scroll) -> Rectangle {
        let shifted = bounds - scroll.translation;
        assert!(
            shifted.x >= scroll.bounds.x - 0.01,
            "{shifted:?} {scroll:?}"
        );
        assert!(
            shifted.y >= scroll.bounds.y - 0.01,
            "{shifted:?} {scroll:?}"
        );
        assert!(
            shifted.x + shifted.width <= scroll.bounds.x + scroll.bounds.width + 0.01,
            "{shifted:?} {scroll:?}"
        );
        assert!(
            shifted.y + shifted.height <= scroll.bounds.y + scroll.bounds.height + 0.01,
            "{shifted:?} {scroll:?}"
        );
        shifted
    }

    #[test]
    fn forward_backward_and_wrap_keep_real_scrollable_inputs_visible() {
        let view: View =
            scrollable(column![input("first"), Space::with_height(400), input("last")].spacing(8))
                .height(100)
                .width(200)
                .into();
        let mut tree = widget::Tree::new(view.as_widget());
        for (direction, name) in [
            (FocusDirection::Next, "first"),
            (FocusDirection::Next, "last"),
            (FocusDirection::Next, "first"),
            (FocusDirection::Previous, "last"),
            (FocusDirection::Previous, "first"),
        ] {
            assert!(run(&view, &mut tree, focus(direction)).is_none());
            let result = snapshot(&view, &mut tree);
            assert_eq!(result.focused, Some(Id::new(name)));
            let _ = visible(result.bounds.unwrap(), result.scrolls[0].1);
        }
    }

    #[test]
    fn nested_ancestors_reveal_the_target_and_leave_unrelated_scrolls_unchanged() {
        let inner = scrollable(column![
            Space::with_height(260),
            input("nested"),
            Space::with_height(80)
        ])
        .height(70)
        .width(160)
        .id(scrollable::Id::new("inner"));
        let outer = scrollable(column![
            Space::with_height(260),
            inner,
            Space::with_height(80)
        ])
        .height(120)
        .width(180)
        .id(scrollable::Id::new("outer"));
        let other = scrollable(column![Space::with_height(500)])
            .height(100)
            .width(100)
            .id(scrollable::Id::new("other"));
        let view: View = row![outer, other].into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(
            &view,
            &mut tree,
            operation::scrollable::scroll_to::<Message>(
                Id::new("other"),
                operation::scrollable::AbsoluteOffset { x: 0.0, y: 85.0 },
            ),
        );
        let before = snapshot(&view, &mut tree);
        assert!(run(&view, &mut tree, focus(FocusDirection::Next)).is_none());
        let after = snapshot(&view, &mut tree);
        assert_eq!(after.focused, Some(Id::new("nested")));
        assert!(after.scrolls[0].1.translation.y > 0.0);
        assert!(after.scrolls[1].1.translation.y > 0.0);
        assert_eq!(
            after.scrolls[2].1.translation,
            before.scrolls[2].1.translation
        );
        let translated = visible(after.bounds.unwrap(), after.scrolls[1].1);
        let _ = visible(translated, after.scrolls[0].1);
    }

    #[test]
    fn visible_fields_do_not_reset_scroll_and_empty_trees_do_not_move() {
        let view: View = scrollable(column![
            Space::with_height(50),
            input("visible"),
            Space::with_height(200)
        ])
        .height(100)
        .width(200)
        .id(scrollable::Id::new("form"))
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(
            &view,
            &mut tree,
            operation::scrollable::scroll_to::<Message>(
                Id::new("form"),
                operation::scrollable::AbsoluteOffset { x: 0.0, y: 30.0 },
            ),
        );
        let before = snapshot(&view, &mut tree).scrolls[0].1.translation;
        for direction in [FocusDirection::Next, FocusDirection::Previous] {
            let _ = run(&view, &mut tree, focus(direction));
            assert_eq!(snapshot(&view, &mut tree).scrolls[0].1.translation, before);
        }
        let view: View = scrollable(Space::with_height(400))
            .height(90)
            .width(200)
            .id(scrollable::Id::new("empty"))
            .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(
            &view,
            &mut tree,
            operation::scrollable::scroll_to::<Message>(
                Id::new("empty"),
                operation::scrollable::AbsoluteOffset { x: 0.0, y: 50.0 },
            ),
        );
        let before = snapshot(&view, &mut tree).scrolls[0].1.translation;
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        assert_eq!(snapshot(&view, &mut tree).scrolls[0].1.translation, before);
    }

    #[test]
    fn both_top_left_axes_can_reveal_a_field() {
        let view: View = scrollable(
            column![
                Space::with_height(180),
                row![Space::with_width(240), input("corner")]
            ]
            .width(500),
        )
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::new(),
            horizontal: scrollable::Scrollbar::new(),
        })
        .height(100)
        .width(160)
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let result = snapshot(&view, &mut tree);
        assert!(result.scrolls[0].1.translation.x > 0.0);
        assert!(result.scrolls[0].1.translation.y > 0.0);
        let bounds = result.bounds.unwrap();
        // A field wider than the viewport aligns its leading edge.
        let adjusted = bounds - result.scrolls[0].1.translation;
        assert!((adjusted.x - result.scrolls[0].1.bounds.x).abs() < 0.01);
        assert!(adjusted.y >= result.scrolls[0].1.bounds.y - 0.01);
    }

    #[test]
    fn oversized_targets_align_the_leading_edge_without_oscillation() {
        let field = region(
            text_input("Large", "large")
                .size(80)
                .id("large")
                .on_input(Message::SummaryChanged),
        );
        let view: View = scrollable(column![
            Space::with_height(150),
            field,
            Space::with_height(150)
        ])
        .height(40)
        .width(200)
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Next));
        let first = snapshot(&view, &mut tree);
        let scroll = first.scrolls[0].1;
        assert!((first.bounds.unwrap().y - scroll.translation.y - scroll.bounds.y).abs() < 0.01);
        let _ = run(&view, &mut tree, focus(FocusDirection::Previous));
        assert_eq!(
            snapshot(&view, &mut tree).scrolls[0].1.translation,
            scroll.translation
        );
    }

    #[test]
    fn unannotated_controls_keep_focus_without_scrolling_an_unrelated_region() {
        let view: View = scrollable(column![
            input("annotated"),
            Space::with_height(300),
            text_input("Legacy", "legacy")
                .id("legacy")
                .on_input(Message::SummaryChanged)
        ])
        .height(90)
        .width(200)
        .into();
        let mut tree = widget::Tree::new(view.as_widget());
        let _ = run(&view, &mut tree, focus(FocusDirection::Previous));
        let result = snapshot(&view, &mut tree);
        assert_eq!(result.focused, Some(Id::new("legacy")));
        assert!(result.bounds.is_none());
        assert_eq!(result.scrolls[0].1.translation, Vector::ZERO);
    }
}
