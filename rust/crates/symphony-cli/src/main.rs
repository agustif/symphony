#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;
use symphony_cli::{CliStartupError, build_validated_startup_config_from_args};
use symphony_config::{ProcessEnv, from_front_matter_with_env};
use symphony_runtime::Runtime;
use symphony_tracker::TrackerState;
use symphony_tracker_linear::LinearTracker;
use symphony_workflow::WorkflowReloader;
use tokio::sync::{RwLock, broadcast, mpsc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cwd = std::env::current_dir()?;
    let startup_config = match build_validated_startup_config_from_args(std::env::args_os(), &cwd) {
        Ok(config) => config,
        Err(CliStartupError::CliArgs(error)) => {
            error.exit();
        }
        Err(error) => {
            eprintln!("startup validation failed: {error}");
            std::process::exit(error.exit_code().into());
        }
    };

    // 1. Load initial workflow and config
    let mut reloader = WorkflowReloader::load_from_path(&startup_config.workflow_path)?;
    let initial_config =
        from_front_matter_with_env(&reloader.current().document.front_matter, &ProcessEnv)?;

    let config_arc = Arc::new(RwLock::new(initial_config.clone()));

    // 2. Initialize Tracker
    let tracker = Arc::new(
        LinearTracker::new(
            initial_config.tracker.endpoint.clone(),
            initial_config.tracker.api_key.clone().unwrap_or_default(),
        )
        .with_candidate_states(
            initial_config
                .tracker
                .active_states
                .iter()
                .map(TrackerState::new)
                .collect(),
        ),
    );

    // 3. Initialize Runtime
    let runtime = Arc::new(Runtime::new(tracker.clone()));

    let (refresh_tx, refresh_rx) = mpsc::channel(1);
    let (shutdown_tx, _) = broadcast::channel(1);

    let shutdown_rx = shutdown_tx.subscribe();
    let runtime_poll = runtime.clone();
    let config_poll = config_arc.clone();

    // 4. Start Poll Loop
    let poll_handle = tokio::spawn(async move {
        runtime_poll
            .run_poll_loop(config_poll, refresh_rx, shutdown_rx)
            .await;
    });

    // 5. Start Workflow Watcher (simple polling for now)
    let watcher_config = config_arc.clone();
    let mut watcher_shutdown = shutdown_tx.subscribe();
    let watcher_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let symphony_workflow::WorkflowReloadOutcome::Updated { .. } = reloader.reload() {
                        tracing::info!("Workflow updated, reloading config...");
                        match from_front_matter_with_env(&reloader.current().document.front_matter, &ProcessEnv) {
                            Ok(new_config) => {
                                let mut guard = watcher_config.write().await;
                                *guard = new_config;
                                let _ = refresh_tx.try_send(());
                            }
                            Err(e) => tracing::error!("Failed to parse updated config: {:?}", e),
                        }
                    }
                }
                _ = watcher_shutdown.recv() => break,
            }
        }
    });

    println!(
        "Symphony Rust Runtime started (workflow: {})",
        startup_config.workflow_path.display()
    );

    // 6. Handle Shutdown
    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");
    let _ = shutdown_tx.send(());

    let _ = tokio::join!(poll_handle, watcher_handle);

    Ok(())
}
