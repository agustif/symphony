use std::sync::Arc;
use std::time::Duration;
use symphony_domain::{Command, IssueId};
use symphony_runtime::{Runtime, RetryPolicy, WorkerExit, RetryKind, RetrySchedule};
use symphony_tracker::{TrackerClient, TrackerIssue, TrackerState};
use symphony_config::RuntimeConfig;
use symphony_testkit::{FakeTracker, tracker_issue};

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

    let tracker = Arc::new(FakeTracker::new(vec![
        issue_1.clone(),
        issue_2.clone(),
        issue_3.clone(),
    ]));
    let runtime = Runtime::new(tracker);

    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 3;

    let tick = runtime.run_tick(&config).await.expect("tick should succeed");

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
    let tracker = Arc::new(FakeTracker::new(vec![
        TrackerIssue::new(issue_id.clone(), "SYM-1", TrackerState::new("Todo"))
    ]));
    let runtime = Runtime::new(tracker);
    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(100),
    };

    // First failure: 10s
    let res1 = runtime.handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy).await.unwrap();
    assert_eq!(res1.retry_schedule.unwrap().delay, Duration::from_secs(10));

    // Second failure: 20s
    let res2 = runtime.handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy).await.unwrap();
    assert_eq!(res2.retry_schedule.unwrap().delay, Duration::from_secs(20));

    // Capped: 100s
    for _ in 0..5 {
        let _ = runtime.handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy).await.unwrap();
    }
    let res_capped = runtime.handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy).await.unwrap();
    assert_eq!(res_capped.retry_schedule.unwrap().delay, Duration::from_secs(100));
}
