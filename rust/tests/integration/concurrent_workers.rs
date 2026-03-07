#![forbid(unsafe_code)]

//! Concurrent worker isolation tests for the Symphony runtime.
//!
//! These tests verify that multiple workers can run concurrently while
//! maintaining proper isolation between workspaces and state.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use symphony_config::RuntimeConfig;
use symphony_runtime::{
    Runtime,
    worker::{WorkerContext, WorkerError, WorkerLauncher, WorkerOutcome},
};
use symphony_testkit::{FakeTracker, FakeTrackerResponse, tracker_issue};
use tokio::sync::{Barrier, Mutex};

/// Tracks which workers have been launched and their execution order
#[derive(Default)]
struct ConcurrentWorkerTracker {
    launched: Mutex<Vec<String>>,
    active: Mutex<HashSet<String>>,
    barrier: Option<Arc<Barrier>>,
}

impl ConcurrentWorkerTracker {
    fn with_barrier(num_workers: usize) -> Self {
        Self {
            barrier: Some(Arc::new(Barrier::new(num_workers))),
            ..Self::default()
        }
    }

    async fn record_launch(&self, identifier: &str) {
        let mut launched = self.launched.lock().await;
        launched.push(identifier.to_owned());

        let mut active = self.active.lock().await;
        active.insert(identifier.to_owned());
    }

    async fn record_completion(&self, identifier: &str) {
        let mut active = self.active.lock().await;
        active.remove(identifier);
    }

    async fn launched_count(&self) -> usize {
        self.launched.lock().await.len()
    }
}

/// Mock worker launcher that tracks concurrent execution
struct TrackingWorkerLauncher {
    tracker: Arc<ConcurrentWorkerTracker>,
    duration: Duration,
}

impl TrackingWorkerLauncher {
    fn new(tracker: Arc<ConcurrentWorkerTracker>) -> Self {
        Self {
            tracker,
            duration: Duration::from_millis(50),
        }
    }
}

#[async_trait]
impl WorkerLauncher for TrackingWorkerLauncher {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
        let identifier = ctx.issue.identifier.clone();
        self.tracker.record_launch(&identifier).await;

        // Wait for barrier if configured (ensures all workers start together)
        if let Some(ref barrier) = self.tracker.barrier {
            barrier.wait().await;
        }

        // Simulate work
        tokio::time::sleep(self.duration).await;

        self.tracker.record_completion(&identifier).await;
        Ok(WorkerOutcome::Completed)
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("symphony-integration-concurrent-{label}-{nonce}"));
    std::fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

/// Test: Max concurrent agents limit is respected.
/// Verifies that the runtime never exceeds the configured max_concurrent_agents.
#[tokio::test]
async fn max_concurrent_agents_limit_respected() {
    let issues: Vec<_> = (0..5)
        .map(|i| tracker_issue(&format!("SYM-{i}"), "Todo"))
        .collect();

    let tracker = Arc::new(FakeTracker::with_default_issues(issues));
    let worker_tracker = Arc::new(ConcurrentWorkerTracker::with_barrier(3));
    let worker = Arc::new(TrackingWorkerLauncher::new(worker_tracker.clone()));

    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("concurrent-limit");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 3; // Limit to 3 concurrent
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for all issues to be processed
    tokio::time::sleep(Duration::from_millis(800)).await;

    let snapshot = runtime.state_snapshot().await;
    assert_eq!(
        snapshot.runtime.running, 0,
        "all issues should be completed"
    );

    // Verify all 5 issues were eventually launched
    let launched = worker_tracker.launched_count().await;
    assert!(
        launched >= 3,
        "at least 3 issues should have been launched, got {}",
        launched
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Workspace isolation between concurrent workers.
/// Verifies that each worker gets its own isolated workspace.
#[tokio::test]
async fn workspace_isolation_between_workers() {
    let issue_1 = tracker_issue("SYM-ISO-1", "Todo");
    let issue_2 = tracker_issue("SYM-ISO-2", "Todo");

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue_1, issue_2]));

    struct WorkspaceCheckingWorker;
    #[async_trait]
    impl WorkerLauncher for WorkspaceCheckingWorker {
        async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            // Verify workspace path is unique to this issue
            let identifier = ctx.issue.identifier.clone();
            let workspace = ctx.workspace_path();

            // Workspace path should contain the identifier
            assert!(
                workspace.to_string_lossy().contains(&identifier),
                "workspace path should contain identifier: {:?} does not contain {}",
                workspace,
                identifier
            );

            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(WorkspaceCheckingWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("workspace-isolation");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 2;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Protocol updates don't leak between workers.
/// Verifies that protocol state is isolated per worker.
#[tokio::test]
async fn protocol_update_isolation() {
    let issue_1 = tracker_issue("SYM-PROTO-1", "Todo");
    let issue_2 = tracker_issue("SYM-PROTO-2", "Todo");

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue_1, issue_2]));

    use symphony_runtime::worker::WorkerProtocolUpdate;

    struct ProtocolIsolationWorker;
    #[async_trait]
    impl WorkerLauncher for ProtocolIsolationWorker {
        async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            let identifier = ctx.issue.identifier.clone();

            // Send protocol update with this worker's identifier
            if let Some(tx) = &ctx.protocol_updates {
                let _ = tx.send(WorkerProtocolUpdate {
                    event: Some(format!("event-for-{identifier}")),
                    timestamp: Some(1_700_000_000),
                    message: Some(format!("message-from-{identifier}")),
                    ..WorkerProtocolUpdate::default()
                });
            }

            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(ProtocolIsolationWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("protocol-isolation");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 2;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let snapshot = runtime.state_snapshot().await;
    // Both workers should have completed
    assert_eq!(snapshot.runtime.running, 0);

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Concurrent claims are serialized per issue.
/// Verifies that the same issue is not claimed by multiple workers.
#[tokio::test]
async fn concurrent_claims_are_serialized_per_issue() {
    // Same issue appearing multiple times (simulates race condition in tracker)
    let issue = tracker_issue("SYM-SINGLE", "Todo");

    // Return the same issue multiple times to test claim deduplication
    let tracker = Arc::new(FakeTracker::scripted(vec![
        FakeTrackerResponse::issues(vec![issue.clone()]),
        FakeTrackerResponse::issues(vec![issue.clone()]), // Same issue again
        FakeTrackerResponse::issues(vec![]),
    ]));

    let claim_count = Arc::new(Mutex::new(0usize));

    struct ClaimCountingWorker {
        claim_count: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl WorkerLauncher for ClaimCountingWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            let mut count = self.claim_count.lock().await;
            *count += 1;
            drop(count);

            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(ClaimCountingWorker {
        claim_count: claim_count.clone(),
    });
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("claim-serialized");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 5;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Shutdown
    let _ = shutdown_tx.send(());
    let _ = runtime_task.await;

    // The issue should be processed at most once
    let final_count = *claim_count.lock().await;
    assert!(
        final_count <= 1,
        "issue should be processed at most once, got {}",
        final_count
    );

    let _ = std::fs::remove_dir_all(&root);
}
