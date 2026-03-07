#![forbid(unsafe_code)]

use std::fs;

use symphony_config::from_front_matter_with_env;
use symphony_domain::IssueId;
use symphony_testkit::FakeTracker;
use symphony_tracker::{TrackerClient, TrackerIssue, TrackerState};
use symphony_workflow::parse as parse_workflow;

use super::{
    ConformanceEnv, REAL_LINEAR_PROJECT_SLUG_ENV, REAL_LINEAR_WORKFLOW_FILE,
    live_linear_conformance_config, skip_real_integration,
};

#[tokio::test]
async fn tracker_fetch_contract_empty_state_filter_delegates_to_full_candidate_fetch() {
    let tracker = FakeTracker::with_default_issues(vec![
        TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo")),
        TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Done")),
    ]);

    let issues = tracker
        .fetch_candidates_by_states(&[])
        .await
        .expect("empty state-filtered fetch should reuse candidate fetch");

    assert_eq!(issues.len(), 2);
    assert_eq!(tracker.fetch_count(), 1);
}

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
async fn tracker_fetch_contract_blank_state_filter_short_circuits_without_fetch() {
    let tracker = FakeTracker::with_default_issues(vec![TrackerIssue::new(
        IssueId("1".into()),
        "SYM-1",
        TrackerState::new("Todo"),
    )]);

    let filtered = tracker
        .fetch_candidates_by_states(&[TrackerState::new("   "), TrackerState::new("\n\t")])
        .await
        .expect("blank state-filtered fetch should short-circuit");

    assert!(filtered.is_empty());
    assert_eq!(tracker.fetch_count(), 0);
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

#[tokio::test]
async fn tracker_state_fetch_contract_ignores_missing_and_duplicate_ids() {
    let tracker = FakeTracker::with_default_issues(vec![
        TrackerIssue::new(IssueId("lin_1".into()), "SYM-1", TrackerState::new("Todo")),
        TrackerIssue::new(
            IssueId("lin_2".into()),
            "SYM-2",
            TrackerState::new("In Progress"),
        ),
    ]);

    let states = tracker
        .fetch_states_by_ids(&[
            IssueId("lin_2".into()),
            IssueId("lin_2".into()),
            IssueId("lin_missing".into()),
        ])
        .await
        .expect("duplicate/missing state subset fetch should succeed");

    assert_eq!(states.len(), 1);
    assert_eq!(
        states.get(&IssueId("lin_2".into())),
        Some(&TrackerState::new("In Progress"))
    );
    assert_eq!(tracker.fetch_count(), 1);
}

#[tokio::test]
async fn tracker_terminal_state_fetch_contract_filters_missing_and_non_terminal_ids() {
    let tracker = FakeTracker::with_default_issues(vec![
        TrackerIssue::new(IssueId("lin_1".into()), "SYM-1", TrackerState::new("Todo")),
        TrackerIssue::new(IssueId("lin_2".into()), "SYM-2", TrackerState::new("Done")),
    ]);

    let terminal_states = tracker
        .fetch_terminal_states_by_ids(
            &[
                IssueId("lin_1".into()),
                IssueId("lin_2".into()),
                IssueId("lin_missing".into()),
            ],
            &[TrackerState::new(" done ")],
        )
        .await
        .expect("terminal state fetch should filter partial data");

    assert_eq!(terminal_states.len(), 1);
    assert_eq!(
        terminal_states.get(&IssueId("lin_2".into())),
        Some(&TrackerState::new("Done"))
    );
}

#[test]
fn real_integration_profile_preflight_uses_explicit_skip_behavior() {
    let live = match live_linear_conformance_config() {
        Ok(config) => config,
        Err(reason) => {
            skip_real_integration(
                "real_integration_profile_preflight_uses_explicit_skip_behavior",
                reason,
            );
            return;
        }
    };

    let workflow_contents = fs::read_to_string(REAL_LINEAR_WORKFLOW_FILE)
        .expect("real workflow file should exist when live conformance is opted in");
    let workflow = parse_workflow(&workflow_contents)
        .expect("real workflow should parse when live conformance is opted in");
    let env = ConformanceEnv::from_entries([
        ("LINEAR_API_KEY", live.api_key.as_str()),
        (REAL_LINEAR_PROJECT_SLUG_ENV, live.project_slug.as_str()),
    ]);
    let config = from_front_matter_with_env(&workflow.front_matter, &env)
        .expect("live workflow front matter should resolve with live conformance inputs");

    assert_eq!(
        config.tracker.api_key.as_deref(),
        Some(live.api_key.as_str())
    );
    assert_eq!(
        config.tracker.project_slug.as_deref(),
        Some(live.project_slug.as_str())
    );
    assert!(!config.tracker.endpoint.trim().is_empty());
}
