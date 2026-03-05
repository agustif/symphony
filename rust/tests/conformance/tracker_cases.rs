#![forbid(unsafe_code)]

use symphony_domain::IssueId;
use symphony_testkit::FakeTracker;
use symphony_tracker::{TrackerClient, TrackerIssue, TrackerState};

#[tokio::test]
async fn tracker_fetch_contract_filters_candidates_by_state() {
    let tracker = FakeTracker::with_default_issues(vec![
        TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo")),
        TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Done")),
    ]);

    let filtered = tracker
        .fetch_candidates_by_states(&[TrackerState::new("todo")])
        .await
        .expect("state-filtered candidate fetch should succeed");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].identifier, "SYM-1");
}

#[tokio::test]
async fn tracker_error_mapping_surfaces_transport_error_from_candidates() {
    let tracker = FakeTracker::scripted(vec![symphony_testkit::FakeTrackerResponse::error(
        "upstream unavailable",
    )]);

    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("scripted error should be surfaced");
    assert_eq!(
        error.to_string(),
        "tracker transport error: upstream unavailable"
    );
}

#[tokio::test]
async fn tracker_state_fetch_contract_returns_requested_subset() {
    let tracker = FakeTracker::with_default_issues(vec![
        TrackerIssue::new(IssueId("lin_1".into()), "SYM-1", TrackerState::new("Todo")),
        TrackerIssue::new(
            IssueId("lin_2".into()),
            "SYM-2",
            TrackerState::new("In Progress"),
        ),
    ]);

    let states = tracker
        .fetch_states_by_ids(&[IssueId("lin_2".into())])
        .await
        .expect("state subset fetch should succeed");

    assert_eq!(states.len(), 1);
    assert_eq!(
        states.get(&IssueId("lin_2".into())),
        Some(&TrackerState::new("In Progress"))
    );
}
