use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use symphony_config::RuntimeConfig;
use symphony_domain::{Command, Event, IssueId, OrchestratorState, RetryEntry, RunningEntry};
use symphony_runtime::{
    RetryPolicy, Runtime, WorkerExit, non_active_reconciliation_ids, terminal_reconciliation_ids,
};
use symphony_testkit::FakeTracker;
use symphony_tracker::{TrackerIssue, TrackerState};

#[tokio::test]
async fn dispatch_sorting_priority_and_recency() {
    let issue_1 = TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo"))
        .with_priority(3)
        .with_created_at(1000);
    let issue_2 = TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Todo"))
        .with_priority(1) // Higher priority
        .with_created_at(2000);
    let issue_3 = TrackerIssue::new(IssueId("3".into()), "SYM-3", TrackerState::new("Todo"))
        .with_priority(3)
        .with_created_at(500); // Older than issue 1

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        issue_1.clone(),
        issue_2.clone(),
        issue_3.clone(),
    ]));
    let runtime = Runtime::new(tracker);

    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 3;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");

    // Expected order: SYM-2 (prio 1), SYM-3 (prio 3, older), SYM-1 (prio 3, newer)
    assert_eq!(
        tick.commands,
        vec![
            Command::Dispatch(issue_2.id),
            Command::Dispatch(issue_3.id),
            Command::Dispatch(issue_1.id),
        ]
    );
}

#[tokio::test]
async fn exponential_backoff_conformance() {
    let issue_id = IssueId("SYM-1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);
    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(100),
    };

    // Set up issue as claimed and running before handling worker exit
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    // First failure: 10s
    let res1 = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(res1.retry_schedule.unwrap().delay, Duration::from_secs(10));

    // Set up for second failure
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    // Second failure: 20s
    let res2 = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(res2.retry_schedule.unwrap().delay, Duration::from_secs(20));

    // Capped: 100s
    for _ in 0..5 {
        runtime
            .setup_running(issue_id.clone(), RunningEntry::default())
            .await;
        let _ = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
            .await
            .unwrap();
    }
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;
    let res_capped = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(
        res_capped.retry_schedule.unwrap().delay,
        Duration::from_secs(100)
    );
}

#[tokio::test]
async fn candidate_selection_skips_claimed_running_and_retrying_issues() {
    let issue_claimed = TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo"));
    let issue_running = TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Todo"));
    let issue_retrying = TrackerIssue::new(IssueId("3".into()), "SYM-3", TrackerState::new("Todo"));
    let issue_available =
        TrackerIssue::new(IssueId("4".into()), "SYM-4", TrackerState::new("Todo"));

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        issue_claimed.clone(),
        issue_running.clone(),
        issue_retrying.clone(),
        issue_available.clone(),
    ]));
    let runtime = Runtime::new(tracker);

    runtime
        .update_agent(Event::Claim(issue_claimed.id.clone()))
        .await
        .expect("claim should apply");
    runtime
        .setup_running(issue_running.id.clone(), RunningEntry::default())
        .await;
    runtime
        .setup_running(issue_retrying.id.clone(), RunningEntry::default())
        .await;
    runtime
        .handle_worker_exit(
            issue_retrying.id.clone(),
            WorkerExit::Continuation,
            &RetryPolicy::default(),
        )
        .await
        .expect("retry should apply");

    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 10;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");
    assert_eq!(tick.commands, vec![Command::Dispatch(issue_available.id)]);
}

#[test]
fn terminal_reconciliation_releases_claimed_running_and_retrying() {
    let issue_terminal = IssueId("SYM-10".into());
    let issue_other = IssueId("SYM-20".into());

    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_terminal.clone());
    state.claimed.insert(issue_other.clone());
    state
        .running
        .insert(issue_terminal.clone(), RunningEntry::default());
    state
        .retry_attempts
        .insert(issue_terminal.clone(), RetryEntry { attempt: 2 });

    let terminal_ids = HashSet::from([issue_terminal.clone()]);
    let releases = terminal_reconciliation_ids(&state, &terminal_ids);

    assert_eq!(releases, vec![issue_terminal]);
}

#[test]
fn non_active_reconciliation_preserves_running_retrying_and_active_candidates() {
    let issue_stale = IssueId("SYM-11".into());
    let issue_running = IssueId("SYM-12".into());
    let issue_retrying = IssueId("SYM-13".into());
    let issue_active_candidate = IssueId("SYM-14".into());

    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_stale.clone());
    state.claimed.insert(issue_running.clone());
    state.claimed.insert(issue_retrying.clone());
    state.claimed.insert(issue_active_candidate.clone());
    state.running.insert(issue_running, RunningEntry::default());
    state
        .retry_attempts
        .insert(issue_retrying, RetryEntry { attempt: 1 });

    let active_ids = HashSet::from([issue_active_candidate]);
    let stale = non_active_reconciliation_ids(&state, &active_ids);

    assert_eq!(stale, vec![issue_stale]);
}
