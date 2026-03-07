use std::sync::Arc;

use serde_json::json;
use symphony_domain::{IssueId, RunningEntry, Usage};
use symphony_runtime::{RetryPolicy, Runtime, WorkerExit};
use symphony_testkit::FakeTracker;
use symphony_tracker::{TrackerIssue, TrackerState};

#[tokio::test]
async fn token_aggregation_remains_correct_across_repeated_updates() {
    let issue_id = IssueId("OBS-1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("OBS-1".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    // First cumulative update: 100 input, 50 output, 150 total
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-obs".to_owned()),
            Some("thread-obs".to_owned()),
            Some("turn-1".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_000),
            None,
            Some(Usage {
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
            }),
            None,
        )
        .await
        .expect("first protocol update should succeed");

    let snap1 = runtime.snapshot().await;
    assert_eq!(snap1.input_tokens, 100);
    assert_eq!(snap1.output_tokens, 50);
    assert_eq!(snap1.total_tokens, 150);

    // Second cumulative update: 250 input, 120 output, 370 total
    // Delta should be +150 input, +70 output, +220 total
    // Global totals should be 250, 120, 370
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-obs".to_owned()),
            Some("thread-obs".to_owned()),
            Some("turn-2".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_010),
            None,
            Some(Usage {
                input_tokens: 250,
                output_tokens: 120,
                total_tokens: 370,
            }),
            None,
        )
        .await
        .expect("second protocol update should succeed");

    let snap2 = runtime.snapshot().await;
    assert_eq!(snap2.input_tokens, 250);
    assert_eq!(snap2.output_tokens, 120);
    assert_eq!(snap2.total_tokens, 370);

    // Third cumulative update: 400 input, 200 output, 600 total
    // Delta should be +150 input, +80 output, +230 total
    // Global totals should be 400, 200, 600
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-obs".to_owned()),
            Some("thread-obs".to_owned()),
            Some("turn-3".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_020),
            None,
            Some(Usage {
                input_tokens: 400,
                output_tokens: 200,
                total_tokens: 600,
            }),
            None,
        )
        .await
        .expect("third protocol update should succeed");

    let snap3 = runtime.snapshot().await;
    assert_eq!(snap3.input_tokens, 400);
    assert_eq!(snap3.output_tokens, 200);
    assert_eq!(snap3.total_tokens, 600);
}

#[tokio::test]
async fn rate_limit_propagation_preserves_latest_payload() {
    let issue_id = IssueId("OBS-2".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-2",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("OBS-2".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    // First update with rate limits
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-obs-2".to_owned()),
            Some("thread-obs-2".to_owned()),
            Some("turn-1".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_000),
            None,
            None,
            Some(json!({
                "primary": { "remaining": 50 }
            })),
        )
        .await
        .expect("first rate limit update should succeed");

    let snap1 = runtime.snapshot().await;
    assert_eq!(
        snap1.rate_limits,
        Some(json!({ "primary": { "remaining": 50 } }))
    );

    // Second update replaces rate limits with new payload
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-obs-2".to_owned()),
            Some("thread-obs-2".to_owned()),
            Some("turn-2".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_010),
            None,
            None,
            Some(json!({
                "primary": { "remaining": 12 },
                "secondary": { "remaining": 99 }
            })),
        )
        .await
        .expect("second rate limit update should succeed");

    let snap2 = runtime.snapshot().await;
    assert_eq!(
        snap2.rate_limits,
        Some(json!({
            "primary": { "remaining": 12 },
            "secondary": { "remaining": 99 }
        }))
    );
}

#[tokio::test]
async fn snapshot_running_and_retrying_counts_match_state() {
    let running_1 = IssueId("OBS-R1".into());
    let running_2 = IssueId("OBS-R2".into());
    let retrying_1 = IssueId("OBS-RET1".into());
    let retrying_2 = IssueId("OBS-RET2".into());

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        TrackerIssue::new(running_1.clone(), "OBS-R1", TrackerState::new("Todo")),
        TrackerIssue::new(running_2.clone(), "OBS-R2", TrackerState::new("Todo")),
        TrackerIssue::new(retrying_1.clone(), "OBS-RET1", TrackerState::new("Todo")),
        TrackerIssue::new(retrying_2.clone(), "OBS-RET2", TrackerState::new("Todo")),
    ]));
    let runtime = Runtime::new(tracker);

    // Set up two running issues
    runtime
        .setup_running(running_1.clone(), RunningEntry::default())
        .await;
    runtime
        .setup_running(running_2.clone(), RunningEntry::default())
        .await;

    // Set up two retrying issues via handle_worker_exit with Continuation
    runtime
        .setup_running(retrying_1.clone(), RunningEntry::default())
        .await;
    runtime
        .handle_worker_exit(
            retrying_1.clone(),
            WorkerExit::Continuation,
            &RetryPolicy::default(),
        )
        .await
        .expect("retry for retrying_1 should apply");

    runtime
        .setup_running(retrying_2.clone(), RunningEntry::default())
        .await;
    runtime
        .handle_worker_exit(
            retrying_2.clone(),
            WorkerExit::Continuation,
            &RetryPolicy::default(),
        )
        .await
        .expect("retry for retrying_2 should apply");

    let snapshot = runtime.snapshot().await;
    assert_eq!(snapshot.running, 2, "expected 2 running issues");
    assert_eq!(snapshot.retrying, 2, "expected 2 retrying issues");
}

// =============================================================================
// C5.1.1: Structured log and state-surface validation for required observability
// =============================================================================

#[tokio::test]
async fn snapshot_exposes_required_runtime_activity_fields() {
    let issue_id = IssueId("OBS-ACT1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-ACT1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("OBS-ACT1".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    let snapshot = runtime.state_snapshot().await;

    // Verify the activity struct exists and has the expected fields
    // (poll timing fields are only populated when poll loop runs)
    assert!(snapshot.runtime.activity.throughput.window_seconds >= 0.0);
    // Verify the snapshot includes activity tracking
    assert!(!snapshot.runtime.activity.poll_in_progress);
}

#[tokio::test]
async fn snapshot_exposes_required_token_accounting_fields() {
    let issue_id = IssueId("OBS-TOK1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-TOK1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("OBS-TOK1".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("session-tok1".to_owned()),
            Some("thread-tok1".to_owned()),
            Some("turn-1".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_000),
            None,
            Some(Usage {
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
            }),
            None,
        )
        .await
        .expect("protocol update should succeed");

    let snapshot = runtime.state_snapshot().await;

    // Verify token fields are exposed
    assert_eq!(snapshot.runtime.total_tokens, 150);
    assert_eq!(snapshot.runtime.input_tokens, 100);
    assert_eq!(snapshot.runtime.output_tokens, 50);
}

#[tokio::test]
async fn issue_snapshot_exposes_required_session_fields() {
    let issue_id = IssueId("OBS-SESS1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-SESS1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("OBS-SESS1".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("thread-sess1-turn-2".to_owned()),
            Some("thread-sess1".to_owned()),
            Some("turn-2".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(1_700_000_100),
            Some("processing turn 2".to_owned()),
            Some(Usage {
                input_tokens: 50,
                output_tokens: 30,
                total_tokens: 80,
            }),
            None,
        )
        .await
        .expect("protocol update should succeed");

    let snapshot = runtime.state_snapshot().await;

    let issue = snapshot
        .issues
        .iter()
        .find(|i| i.id == issue_id)
        .expect("issue should appear in snapshot");

    // Verify required session fields
    // Note: session_id combines thread/turn info; IssueSnapshot does not have separate thread_id/turn_id
    assert_eq!(issue.session_id.as_deref(), Some("thread-sess1-turn-2"));
    assert_eq!(issue.turn_count, 1);
    assert!(issue.last_event_at.is_some());
}

#[tokio::test]
async fn retrying_issue_snapshot_exposes_required_retry_fields() {
    let issue_id = IssueId("OBS-RET3".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "OBS-RET3",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    // Set up running with an error message that will be captured in retry metadata
    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                last_codex_message: Some("agent error: timeout exceeded".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    runtime
        .handle_worker_exit(
            issue_id.clone(),
            WorkerExit::RetryableFailure,
            &RetryPolicy::default(),
        )
        .await
        .expect("retryable failure should schedule retry");

    let snapshot = runtime.state_snapshot().await;

    let issue = snapshot
        .issues
        .iter()
        .find(|i| i.id == issue_id)
        .expect("retrying issue should appear in snapshot");

    // Verify required retry fields
    assert_eq!(issue.state, "Retrying");
    assert_eq!(issue.retry_attempts, 1);
    assert!(issue.retry_due_at.is_some());
    assert!(issue.retry_error.is_some());
}
