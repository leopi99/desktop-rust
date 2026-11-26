//! Behavioral coverage for the presentation-only selected-file state.
use super::*;

#[test]
fn read_requests_track_the_file_but_busy_requests_cannot_redirect_presentation() {
    let change = FileChange {
        path: "folder/file.txt".into(),
        original_path: Some("old.txt".into()),
        index: 'R',
        worktree: 'M',
    };
    for message in [
        Message::Select(change.clone()),
        Message::ReviewHunks(change.clone()),
        Message::ReviewStagedHunks(change.clone()),
        Message::ReviewConflict(change.clone()),
        Message::ReviewConflictSide(change.clone(), ConflictSide::Ours),
        Message::RequestConflictSideResolution(change.clone(), ConflictSide::Theirs),
    ] {
        let mut app = tests::hunk_app();
        app.summary = "keep summary".into();
        app.description = "keep description".into();
        let original = app.selected_change.clone();
        app.loading = true;
        let _ = app.update(message.clone());
        assert_eq!(app.selected_change, original);
        app.loading = false;
        let _ = app.update(message);
        assert_eq!(app.selected_change.as_ref(), Some(&change));
        assert!(app.loading);
        assert_eq!(app.summary, "keep summary");
        assert_eq!(app.description, "keep description");
    }
}

#[test]
fn repository_and_navigation_transitions_revoke_the_previous_file_presentation() {
    for action in ["path", "open", "loaded", "history", "branches", "show"] {
        let mut app = tests::hunk_app();
        assert!(app.selected_change.is_some());
        let message = match action {
            "path" => Message::PathChanged("other-repository".into()),
            "open" => Message::Open,
            "loaded" => Message::Loaded(Ok(app.snapshot.clone().unwrap())),
            "history" => Message::PanelSelected(Panel::History),
            "branches" => Message::PanelSelected(Panel::Branches),
            "show" => Message::ShowHistory(false),
            _ => unreachable!(),
        };
        let _ = app.update(message);
        assert!(app.selected_change.is_none(), "{action}");
    }
}

#[test]
fn review_results_select_the_observed_path_and_dismissal_revokes_that_presentation() {
    let mut app = tests::hunk_app();
    app.summary = "keep summary".into();
    app.description = "keep description".into();
    let review = app.hunk_review.as_ref().unwrap().as_ref().clone();
    let snapshot = app.snapshot.clone();
    let _ = app.update(Message::CancelHunks);
    assert!(app.selected_change.is_none());
    let _ = app.update(Message::HunksReviewed(Ok(review.clone())));
    assert_eq!(app.selected_change.as_ref(), Some(&review.change));
    let _ = app.update(Message::DismissReview);
    assert!(app.selected_change.is_none());
    let choice = side_preview_tests::choice(&app);
    let expected = choice.preview.change.clone();
    let _ = app.update(Message::ConflictSideResolutionReviewed(Ok(choice)));
    assert_eq!(app.selected_change.as_ref(), Some(&expected));
    let _ = app.update(Message::CancelConflictSideResolution);
    assert!(app.selected_change.is_none());
    assert!(!app.loading);
    assert_eq!(app.snapshot, snapshot);
    assert_eq!(app.summary, "keep summary");
    assert_eq!(app.description, "keep description");
}
