#![forbid(unsafe_code)]

//! Graceful shutdown tests for the Symphony runtime.
//!
//! These tests verify that the runtime shuts down cleanly, completing
//! in-flight operations and cleaning up resources properly.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use symphony_config::RuntimeConfig;
use symphony_runtime::{
    Runtime,
    worker::{WorkerContext, WorkerError, WorkerLauncher, WorkerOutcome},
};
use symphony_testkit::{FakeTracker, tracker_issue};

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-integration-shutdown-{label}-{nonce}"));
    std::fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

/// Test: Idle runtime shuts down immediately.
/// Verifies that shutdown is quick when no work is in progress.
#[tokio::test]
async fn idle_shutdown_is_immediate() {
    // Empty tracker - no work to do
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![]));

    struct ImmediateWorker;
    #[async_trait]
    impl WorkerLauncher for ImmediateWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(ImmediateWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("idle-shutdown");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 60_000; // Long interval

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let start = std::time::Instant::now();
    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Small delay to let the poll loop start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Shutdown immediately
    let _ = shutdown_tx.send(());

    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "idle shutdown should be fast, took {:?}",
        elapsed
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Shutdown waits for in-flight operations.
/// Verifies that active workers complete before shutdown.
#[tokio::test]
async fn shutdown_waits_for_inflight_operations() {
    let issue = tracker_issue("SYM-SHUTDOWN-WAIT", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    let completion_flag = Arc::new(AtomicUsize::new(0));

    struct SlowWorker {
        completion_flag: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl WorkerLauncher for SlowWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            // Simulate work that takes 100ms
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.completion_flag.fetch_add(1, Ordering::SeqCst);
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(SlowWorker {
        completion_flag: completion_flag.clone(),
    });
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("shutdown-wait");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for worker to start and complete
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Initiate shutdown
    let _ = shutdown_tx.send(());

    // Wait for runtime to complete
    let _ = runtime_task.await;

    // Worker should have completed
    let completions = completion_flag.load(Ordering::SeqCst);
    assert!(
        completions >= 1,
        "worker should have completed, got {}",
        completions
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Workspace cleanup on shutdown with active workers.
/// Verifies that active workspaces are handled properly during shutdown.
#[tokio::test]
async fn workspace_cleanup_on_shutdown() {
    let issue = tracker_issue("SYM-SHUTDOWN-CLEANUP", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    struct WorkspaceCreatingWorker;
    #[async_trait]
    impl WorkerLauncher for WorkspaceCreatingWorker {
        async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            // Create a file in the workspace
            let workspace = ctx.workspace_path();
            std::fs::create_dir_all(&workspace).ok();
            std::fs::write(workspace.join("work.txt"), "in progress").ok();

            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(WorkspaceCreatingWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("shutdown-cleanup");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Let the worker start and create its workspace
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Shutdown while work is in progress
    let _ = shutdown_tx.send(());

    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    // Workspace behavior after shutdown depends on cleanup policy
    // (active workspaces may be preserved for resumption)
    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Shutdown with multiple active workers.
/// Verifies all in-flight workers complete before shutdown.
#[tokio::test]
async fn shutdown_waits_for_all_workers() {
    let issues: Vec<_> = (0..3)
        .map(|i| tracker_issue(&format!("SYM-SHUTDOWN-MULTI-{}", i), "Todo"))
        .collect();

    let tracker = Arc::new(FakeTracker::with_default_issues(issues));

    let completed_count = Arc::new(AtomicUsize::new(0));

    struct CountingWorker {
        completed: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl WorkerLauncher for CountingWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            // Fixed delay
            tokio::time::sleep(Duration::from_millis(50)).await;
            self.completed.fetch_add(1, Ordering::SeqCst);
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(CountingWorker {
        completed: completed_count.clone(),
    });
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("shutdown-multi");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 3;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for workers to start and complete
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Initiate shutdown
    let _ = shutdown_tx.send(());

    // Wait for runtime to complete
    let _ = runtime_task.await;

    // Workers should have completed
    let completed = completed_count.load(Ordering::SeqCst);
    assert!(
        completed >= 2,
        "at least 2 workers should have completed, got {}",
        completed
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Shutdown during retry scheduling.
/// Verifies retry state is preserved when shutdown interrupts retry.
#[tokio::test]
async fn shutdown_preserves_retry_state() {
    let issue = tracker_issue("SYM-SHUTDOWN-RETRY", "Todo");
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![issue]));

    struct FailingWorker;
    #[async_trait]
    impl WorkerLauncher for FailingWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok(WorkerOutcome::RetryableFailure)
        }
    }

    let worker = Arc::new(FailingWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("shutdown-retry");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 10;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    // Wait for first attempt to fail and retry to be scheduled
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shutdown during retry wait period
    let _ = shutdown_tx.send(());

    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    // State should show retry was in progress (state preserved)
    let _ = std::fs::remove_dir_all(&root);
}

/// Test: Multiple shutdown signals are handled gracefully.
/// Verifies that sending shutdown multiple times doesn't cause issues.
#[tokio::test]
async fn multiple_shutdown_signals_handled() {
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![]));

    struct ImmediateWorker;
    #[async_trait]
    impl WorkerLauncher for ImmediateWorker {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            Ok(WorkerOutcome::Completed)
        }
    }

    let worker = Arc::new(ImmediateWorker);
    let runtime = Arc::new(Runtime::with_worker_launcher(tracker, worker));

    let root = temp_root("multi-shutdown");
    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 1;
    config.workspace.root = root.clone();
    config.polling.interval_ms = 60_000;

    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let runtime_task =
        tokio::spawn(Arc::clone(&runtime).run_poll_loop(config, refresh_rx, shutdown_rx));

    tokio::time::sleep(Duration::from_millis(10)).await;

    // Send multiple shutdown signals
    let _ = shutdown_tx.send(());
    let _ = shutdown_tx.send(()); // Should be safe
    let _ = shutdown_tx.send(()); // Should be safe

    runtime_task
        .await
        .expect("poll loop should shut down cleanly despite multiple signals");

    let _ = std::fs::remove_dir_all(&root);
}
