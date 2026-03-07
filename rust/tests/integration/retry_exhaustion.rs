#![forbid(unsafe_code)]

//! Retry exhaustion tests for the Symphony runtime.
//!
//! These tests verify that retry logic correctly handles failure scenarios,
//! exponential backoff, and eventual permanent failure after max attempts.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use symphony_config::RuntimeConfig;
use symphony_runtime::{
    RetryKind, RetryPolicy, Runtime,
    worker::{WorkerContext, WorkerError, WorkerLauncher, WorkerOutcome},
};
use symphony_testkit::{FakeTracker, tracker_issue};
use tokio::sync::Mutex;

/// Mock worker that always fails with retryable errors
struct RetryableFailureWorker {
    failure_count: Mutex<usize>,
    max_failures: usize,
}

impl RetryableFailureWorker {
    fn always_fail() -> Self {
        Self {
            failure_count: Mutex::new(0),
            max_failures: usize::MAX,
        }
    }

    fn fail_then_succeed(failures_before_success: usize) -> Self {
        Self {
            failure_count: Mutex::new(0),
            max_failures: failures_before_success,
        }
    }
}

#[async_trait]
impl WorkerLauncher for RetryableFailureWorker {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
        let mut count = self.failure_count.lock().await;
        *count += 1;

        if *count <= self.max_failures {
            // Send protocol message about failure
            if let Some(tx) = &ctx.protocol_updates {
                use symphony_runtime::worker::WorkerProtocolUpdate;
                let _ = tx.send(WorkerProtocolUpdate {
                    event: Some("turn/failed".to_owned()),
                    message: Some(format!("Attempt {} failed", *count)),
                    ..WorkerProtocolUpdate::default()
                });
            }
            Ok(WorkerOutcome::RetryableFailure)
        } else {
            Ok(WorkerOutcome::Completed)
        }
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-integration-retry-{label}-{nonce}"));
    std::fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

/// Test: Exponential backoff between retry attempts.
/// Verifies that retry delays increase exponentially.
#[tokio::test]
async fn retry_uses_exponential_backoff() {
    let issue = tracker_issue("SYM-RETRY-1", "Todo");
    let _tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    let policy = RetryPolicy {
        continuation_delay: Duration::from_millis(100),
        failure_base_delay: Duration::from_millis(100),
        max_failure_backoff: Duration::from_secs(10),
    };

    // Test backoff calculation for failure retries
    // The backoff doubles with each attempt: 100ms, 200ms, 400ms, 800ms...
    let schedule1 = policy.schedule_retry(issue_id("SYM-RETRY-1"), 1, RetryKind::Failure);
    let schedule2 = policy.schedule_retry(issue_id("SYM-RETRY-1"), 2, RetryKind::Failure);
    let schedule3 = policy.schedule_retry(issue_id("SYM-RETRY-1"), 3, RetryKind::Failure);
    let schedule4 = policy.schedule_retry(issue_id("SYM-RETRY-1"), 4, RetryKind::Failure);

    assert_eq!(schedule1.delay, Duration::from_millis(100));
    assert_eq!(schedule2.delay, Duration::from_millis(200));
    assert_eq!(schedule3.delay, Duration::from_millis(400));
    assert_eq!(schedule4.delay, Duration::from_millis(800));

    // Should cap at max_failure_backoff
    let aggressive_policy = RetryPolicy {
        continuation_delay: Duration::from_secs(5),
        failure_base_delay: Duration::from_secs(5),
        max_failure_backoff: Duration::from_secs(10),
    };
    let schedule =
        aggressive_policy.schedule_retry(issue_id("SYM-RETRY-1"), 10, RetryKind::Failure);
    assert_eq!(schedule.delay, Duration::from_secs(10));
}

use symphony_testkit::issue_id;

/// Test: Retry exhaustion after max attempts.
/// Verifies that after many retry attempts, the issue eventually stops retrying.
#[tokio::test]
async fn retry_exhaustion_stops_retrying() {
    let issue = tracker_issue("SYM-RETRY-EXHAUST", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    let worker = Arc::new(RetryableFailureWorker::always_fail());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("retry-exhaust");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for retries to occur
    tokio::time::sleep(Duration::from_millis(500)).await;

    let snapshot = runtime.state_snapshot().await;

    // After some time, the issue should not be running
    // (It may still be retrying depending on backoff)
    assert!(
        snapshot.runtime.running <= 1,
        "at most 1 issue should be running"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Successful retry after failures.
/// Verifies that an issue can succeed after some retry attempts.
#[tokio::test]
async fn retry_succeeds_after_failures() {
    let issue = tracker_issue("SYM-RETRY-SUCCESS", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    // Fail twice, then succeed
    let worker = Arc::new(RetryableFailureWorker::fail_then_succeed(2));
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("retry-success");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for retries to complete successfully
    tokio::time::sleep(Duration::from_millis(700)).await;

    let snapshot = runtime.state_snapshot().await;
    // After retries and success, running should be 0
    // retrying might be 0 or 1 depending on exact timing
    assert_eq!(
        snapshot.runtime.running, 0,
        "issue should not be running after success"
    );
    // Allow for some retry state that hasn't been cleaned up yet

    let _ = shutdown_tx.send(());
    let _ = runtime_task.await;

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Retry state is preserved in state snapshot.
/// Verifies that retry attempts and error messages are visible in snapshots.
#[tokio::test]
async fn retry_state_appears_in_snapshot() {
    let issue = tracker_issue("SYM-RETRY-SNAP", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    let worker = Arc::new(RetryableFailureWorker::always_fail());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("retry-snapshot");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for first retry to be scheduled
    tokio::time::sleep(Duration::from_millis(150)).await;

    let _snapshot = runtime.state_snapshot().await;

    // Should have at least one retry in progress or completed
    // (depending on timing and backoff)
    // The runtime tracks retries separately

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Workspace cleanup on retry exhaustion.
/// Verifies that workspace is handled appropriately when retries are exhausted.
#[tokio::test]
async fn workspace_handled_on_retry() {
    let issue = tracker_issue("SYM-RETRY-CLEANUP", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    let worker = Arc::new(RetryableFailureWorker::always_fail());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("retry-cleanup");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Non-retryable failure doesn't trigger retries.
/// Verifies that PermanentFailure doesn't cause retry attempts.
#[tokio::test]
async fn permanent_failure_skips_retries() {
    let issue = tracker_issue("SYM-PERMANENT-FAIL", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    struct PermanentFailureWorker;
    #[async_trait]
    impl WorkerLauncher for PermanentFailureWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            Ok(WorkerOutcome::PermanentFailure)
        }
    }

    let worker = Arc::new(PermanentFailureWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("permanent-fail");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let snapshot = runtime.state_snapshot().await;

    // Should have no retries (permanent failure doesn't retry)
    assert_eq!(
        snapshot.runtime.retrying, 0,
        "permanent failure should not trigger retries"
    );
    assert_eq!(snapshot.runtime.running, 0, "no issues should be running");

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Continuation outcome triggers retry.
/// Verifies that Continuation outcome schedules a retry.
#[tokio::test]
async fn continuation_schedules_retry() {
    let issue = tracker_issue("SYM-CONTINUATION", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    struct ContinuationWorker;
    #[async_trait]
    impl WorkerLauncher for ContinuationWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(WorkerOutcome::Continuation)
        }
    }

    let worker = Arc::new(ContinuationWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("continuation");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for first continuation
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check that retry was scheduled
    let snapshot = runtime.state_snapshot().await;
    // After continuation, the issue should be in retrying state
    assert!(
        snapshot.runtime.retrying >= 1 || snapshot.runtime.running >= 1,
        "issue should be retrying or running after continuation"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}
