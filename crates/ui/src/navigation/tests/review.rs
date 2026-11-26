//! Real-widget coverage for review dismissal controls.
use super::*;
use std::sync::Arc;

fn targets() -> [ReviewTarget; 2] {
    let app = crate::tests::hunk_app();
    [
        ReviewTarget::Hunks(Arc::clone(app.hunk_review.as_ref().unwrap())),
        ReviewTarget::ConflictChoice(Arc::new(crate::side_preview_tests::choice(&app))),
    ]
}

fn control(target: ReviewTarget, enabled: bool) -> View {
    review_button("Close review", target, enabled)
}

#[test]
fn dismissal_buttons_join_traversal_and_skip_disabled_and_write_controls() {
    let [hunks, side] = targets();
    let view: View = iced::widget::column![
        text_input("Summary", "keep").on_input(Message::SummaryChanged),
        control(hunks, true),
        control(side.clone(), false),
        control(side, true),
        iced::widget::button("Apply").on_press(Message::ApplyConflictSideResolution),
    ]
    .into();
    let mut tree = widget::Tree::new(view.as_widget());
    for (direction, expected) in [
        (FocusDirection::Next, 0),
        (FocusDirection::Next, 1),
        (FocusDirection::Next, 2),
        (FocusDirection::Next, 0),
        (FocusDirection::Previous, 2),
    ] {
        let _ = run(&view, &mut tree, focus(direction));
        let count = run(&view, &mut tree, operation::focusable::count()).unwrap();
        assert_eq!(count.total, 3);
        assert_eq!(count.focused, Some(expected));
    }
}

#[test]
fn held_keys_dismiss_once_and_same_review_redraw_preserves_the_gesture() {
    for target in targets() {
        for name in [key::Named::Enter, key::Named::Space] {
            let mut view = control(target.clone(), true);
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
            view = control(target.clone(), true);
            tree.diff(view.as_widget());
            let (_, messages) = send(
                &mut view,
                &mut tree,
                key(name, Modifiers::empty(), false),
                mouse::Cursor::Unavailable,
            );
            assert_eq!(messages.len(), 1);
            let mut app = crate::tests::hunk_app();
            app.summary = "keep summary".into();
            app.description = "keep body".into();
            match &target {
                ReviewTarget::Hunks(review) => {
                    app.hunk_review = Some(Arc::clone(review));
                    app.selected_hunks.insert(0);
                }
                ReviewTarget::ConflictChoice(review) => {
                    app.hunk_review = None;
                    app.side_resolution_review = Some(Arc::clone(review));
                }
            }
            for message in messages {
                assert!(matches!(
                    (&target, &message),
                    (ReviewTarget::Hunks(_), Message::CancelHunks)
                        | (
                            ReviewTarget::ConflictChoice(_),
                            Message::CancelConflictSideResolution
                        )
                ));
                let _ = app.update(message);
            }
            assert!(app.hunk_review.is_none());
            assert!(app.side_resolution_review.is_none());
            assert!(app.selected_hunks.is_empty());
            assert!(!app.loading);
            assert_eq!(app.summary, "keep summary");
            assert_eq!(app.description, "keep body");
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
fn replacement_review_even_with_equal_content_cannot_inherit_focus_or_armed_dismissal() {
    for target in targets() {
        for replacement in ["review", "kind", "disabled", "navigation"] {
            let mut view = control(target.clone(), true);
            let mut tree = widget::Tree::new(view.as_widget());
            let _ = run(&view, &mut tree, focus(FocusDirection::Next));
            let _ = send(
                &mut view,
                &mut tree,
                key(key::Named::Enter, Modifiers::empty(), true),
                mouse::Cursor::Unavailable,
            );
            view = match replacement {
                "review" => control(
                    match &target {
                        ReviewTarget::Hunks(review) => {
                            ReviewTarget::Hunks(Arc::new((**review).clone()))
                        }
                        ReviewTarget::ConflictChoice(review) => {
                            ReviewTarget::ConflictChoice(Arc::new((**review).clone()))
                        }
                    },
                    true,
                ),
                "kind" => control(
                    match &target {
                        ReviewTarget::Hunks(_) => targets()[1].clone(),
                        ReviewTarget::ConflictChoice(_) => targets()[0].clone(),
                    },
                    true,
                ),
                "disabled" => control(target.clone(), false),
                "navigation" => button("Changes", Panel::Changes, true, true),
                _ => unreachable!(),
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
}

#[test]
fn pointer_press_survives_same_review_redraw_but_never_dismisses_a_replacement() {
    for target in targets() {
        for touch in [false, true] {
            for replace in [false, true] {
                let mut view = control(target.clone(), true);
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
                let next = if replace {
                    match &target {
                        ReviewTarget::Hunks(review) => {
                            ReviewTarget::Hunks(Arc::new((**review).clone()))
                        }
                        ReviewTarget::ConflictChoice(review) => {
                            ReviewTarget::ConflictChoice(Arc::new((**review).clone()))
                        }
                    }
                } else {
                    target.clone()
                };
                view = control(next, true);
                tree.diff(view.as_widget());
                let (_, messages) = send(&mut view, &mut tree, release, cursor);
                assert_eq!(
                    messages.len(),
                    usize::from(!replace),
                    "touch={touch}, replace={replace}"
                );
                for message in messages {
                    assert!(matches!(
                        (&target, &message),
                        (ReviewTarget::Hunks(_), Message::CancelHunks)
                            | (
                                ReviewTarget::ConflictChoice(_),
                                Message::CancelConflictSideResolution
                            )
                    ));
                }
            }
        }
    }
}
