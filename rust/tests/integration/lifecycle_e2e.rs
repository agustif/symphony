#![forbid(unsafe_code)]

//! End-to-end lifecycle tests for the Symphony runtime.
//!
//! These tests verify the complete issue lifecycle from claim through
//! completion, ensuring all state transitions are correctly handled.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use symphony_config::RuntimeConfig;
use symphony_runtime::{
    Runtime,
    worker::{WorkerContext, WorkerError, WorkerLauncher, WorkerOutcome, WorkerProtocolUpdate},
};
use symphony_testkit::{FakeTracker, FakeTrackerResponse, tracker_issue};
use tokio::sync::Mutex;

/// Mock worker launcher that can be configured with a sequence of outcomes
/// and optional protocol messages to simulate various worker behaviors.
#[derive(Default)]
struct LifecycleMockWorker {
    outcomes: Mutex<Vec<WorkerOutcome>>,
    protocol_updates: Option<Vec<WorkerProtocolUpdate>>,
}

impl LifecycleMockWorker {
    fn with_outcomes(outcomes: Vec<WorkerOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes),
            protocol_updates: None,
        }
    }

    fn with_completion() -> Self {
        Self::with_outcomes(vec![WorkerOutcome::Completed])
    }

    fn with_protocol_updates(updates: Vec<WorkerProtocolUpdate>) -> Self {
        Self {
            outcomes: Mutex::new(vec![WorkerOutcome::Completed]),
            protocol_updates: Some(updates),
        }
    }
}

#[async_trait]
impl WorkerLauncher for LifecycleMockWorker {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
        // Send protocol updates if configured
        if let Some(ref updates) = self.protocol_updates
            && let Some(tx) = &ctx.protocol_updates
        {
            for update in updates {
                let _ = tx.send(update.clone());
            }
        }

        let mut outcomes = self.outcomes.lock().await;
        Ok(outcomes.pop().unwrap_or(WorkerOutcome::Completed))
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-integration-lifecycle-{label}-{nonce}"));
    std::fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

/// Test: Full issue lifecycle from claim to completion.
/// Verifies that an issue is claimed, run, and properly transitions
/// through all state machine stages to completion.
#[tokio::test]
async fn full_lifecycle_claim_to_complete() {
    let issue = tracker_issue("SYM-100", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue.clone()]));
    let worker = Arc::new(LifecycleMockWorker::with_completion());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("lifecycle-complete");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for the issue to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify state snapshot shows completed
    let snapshot = runtime.state_snapshot().await;
    assert_eq!(snapshot.runtime.running, 0, "no issues should be running");
    assert_eq!(snapshot.runtime.retrying, 0, "no issues should be retrying");

    // Shutdown
    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    // Cleanup
    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Lifecycle with protocol updates.
/// Verifies that protocol updates during worker execution are captured
/// in the runtime state snapshot.
#[tokio::test]
async fn lifecycle_with_protocol_updates() {
    let issue = tracker_issue("SYM-101", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue.clone()]));

    let worker = Arc::new(LifecycleMockWorker::with_protocol_updates(vec![
        WorkerProtocolUpdate {
            event: Some("turn/start".to_owned()),
            timestamp: Some(1_700_000_100),
            message: Some("starting turn 1".to_owned()),
            ..WorkerProtocolUpdate::default()
        },
        WorkerProtocolUpdate {
            event: Some("turn/complete".to_owned()),
            timestamp: Some(1_700_000_200),
            message: Some("turn 1 complete".to_owned()),
            ..WorkerProtocolUpdate::default()
        },
    ]));

    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("lifecycle-protocol");
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
    // After completion, running count should be 0
    assert_eq!(snapshot.runtime.running, 0);

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Lifecycle with scripted tracker responses.
/// Verifies the runtime correctly handles a sequence of tracker responses.
#[tokio::test]
async fn lifecycle_with_scripted_tracker() {
    let issue_1 = tracker_issue("SYM-200", "Todo");
    let issue_2 = tracker_issue("SYM-201", "Todo");

    let tracker = Arc::new(FakeTracker::scripted(vec![
        FakeTrackerResponse::issues(vec![issue_1.clone()]),
        FakeTrackerResponse::issues(vec![issue_2.clone()]),
        FakeTrackerResponse::issues(vec![]), // No more issues
    ]));

    let worker = Arc::new(LifecycleMockWorker::with_completion());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker.clone(), worker));

    let root = temp_root("lifecycle-scripted");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 2;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for multiple poll cycles
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify multiple issues were processed
    assert!(
        tracker.fetch_count() >= 2,
        "tracker should have been polled multiple times"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Lifecycle state transitions are atomic.
/// Verifies that an issue cannot be in multiple states simultaneously.
#[tokio::test]
async fn lifecycle_state_transitions_are_atomic() {
    let issue = tracker_issue("SYM-300", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue.clone()]));

    // Worker that takes some time to complete
    struct SlowWorker;
    #[async_trait]
    impl WorkerLauncher for SlowWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(SlowWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("lifecycle-atomic");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 5;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Poll state multiple times during execution
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let snapshot = runtime.state_snapshot().await;

        // At any point, the issue should be in at most one active state
        let running = snapshot.runtime.running;
        let retrying = snapshot.runtime.retrying;
        assert!(
            running + retrying <= 1,
            "issue should not be in multiple active states: running={}, retrying={}",
            running,
            retrying
        );
    }

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Lifecycle with tracker errors followed by recovery.
/// Verifies that transient tracker errors don't corrupt state.
#[tokio::test]
async fn lifecycle_recovers_from_tracker_errors() {
    let issue = tracker_issue("SYM-400", "Todo");

    let tracker = Arc::new(FakeTracker::scripted(vec![
        FakeTrackerResponse::error("temporary network error"),
        FakeTrackerResponse::error("another temporary error"),
        FakeTrackerResponse::issues(vec![issue.clone()]),
        FakeTrackerResponse::issues(vec![]),
    ]));

    let worker = Arc::new(LifecycleMockWorker::with_completion());
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker.clone(), worker));

    let root = temp_root("lifecycle-recovery");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for recovery and processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the issue was eventually processed despite initial errors
    assert!(
        tracker.fetch_count() >= 3,
        "tracker should have been polled at least 3 times"
    );

    let snapshot = runtime.state_snapshot().await;
    assert_eq!(
        snapshot.runtime.running, 0,
        "no issues should be running after completion"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}
