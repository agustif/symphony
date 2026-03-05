#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;
use symphony_cli::{CliStartupError, build_validated_startup_config_from_args};
use symphony_config::{
    CliOverrides, ConfigError, ProcessEnv, RuntimeConfig, apply_cli_overrides,
    from_front_matter_with_env,
};
use symphony_runtime::Runtime;
use symphony_tracker::TrackerState;
use symphony_tracker_linear::LinearTracker;
use symphony_workflow::{LoadedWorkflow, WorkflowReloader};
use tokio::sync::{RwLock, broadcast, mpsc};

fn effective_runtime_config(
    loaded_workflow: &LoadedWorkflow,
    cli_overrides: &CliOverrides,
) -> Result<RuntimeConfig, ConfigError> {
    let config = from_front_matter_with_env(&loaded_workflow.document.front_matter, &ProcessEnv)?;
    apply_cli_overrides(config, cli_overrides)
}

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
    let cli_overrides = startup_config.cli_overrides.clone();

    // 1. Load initial workflow and config
    let mut reloader = WorkflowReloader::load_from_path(&startup_config.workflow_path)?;
    let initial_config = effective_runtime_config(reloader.current(), &cli_overrides)?;

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
    let watcher_overrides = cli_overrides.clone();
    let mut watcher_shutdown = shutdown_tx.subscribe();
    let watcher_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let symphony_workflow::WorkflowReloadOutcome::Updated { .. } = reloader.reload() {
                        tracing::info!("Workflow updated, reloading config...");
                        match effective_runtime_config(reloader.current(), &watcher_overrides) {
                            Ok(new_config) => {
                                let mut guard = watcher_config.write().await;
                                *guard = new_config;
                                let _ = refresh_tx.try_send(());
                            }
                            Err(error) => tracing::error!(
                                "Failed to build updated config with CLI overrides: {:?}",
                                error
                            ),
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use symphony_workflow::WorkflowReloadOutcome;

    #[test]
    fn startup_effective_config_uses_cli_overrides_over_front_matter() {
        let root = temp_dir("startup_effective_config_uses_cli_overrides_over_front_matter");
        let workflow_path = root.join("WORKFLOW.md");
        let config_path = root.join("symphony.runtime.json");
        let workspace_root = root.join("cli-workspaces");

        write_runtime_config(&config_path);
        write_workflow(
            &workflow_path,
            5_000,
            9,
            "https://workflow.example/graphql",
            "workflow-token",
            "workflow-project",
        );

        let startup = build_validated_startup_config_from_args(
            vec![
                "symphony".to_owned(),
                workflow_path.display().to_string(),
                "--config".to_owned(),
                config_path.display().to_string(),
                "--polling-interval-ms".to_owned(),
                "120000".to_owned(),
                "--max-turns".to_owned(),
                "42".to_owned(),
                "--workspace-root".to_owned(),
                workspace_root.display().to_string(),
                "--tracker-endpoint".to_owned(),
                "https://cli.example/graphql".to_owned(),
                "--tracker-api-key".to_owned(),
                "cli-token".to_owned(),
                "--tracker-project-slug".to_owned(),
                "cli-project".to_owned(),
            ],
            &root,
        )
        .expect("startup args should parse and validate");

        let reloader = WorkflowReloader::load_from_path(&startup.workflow_path)
            .expect("workflow should load for startup");
        let effective = effective_runtime_config(reloader.current(), &startup.cli_overrides)
            .expect("effective config should build from startup args");

        assert_eq!(effective.polling.interval_ms, 120_000);
        assert_eq!(effective.agent.max_turns, 42);
        assert_eq!(effective.workspace.root, workspace_root);
        assert_eq!(effective.tracker.endpoint, "https://cli.example/graphql");
        assert_eq!(effective.tracker.api_key.as_deref(), Some("cli-token"));
        assert_eq!(
            effective.tracker.project_slug.as_deref(),
            Some("cli-project")
        );
    }

    #[test]
    fn workflow_reload_reapplies_cli_overrides_after_front_matter_change() {
        let root = temp_dir("workflow_reload_reapplies_cli_overrides_after_front_matter_change");
        let workflow_path = root.join("WORKFLOW.md");
        let config_path = root.join("symphony.runtime.json");
        let workspace_root = root.join("cli-workspaces");

        write_runtime_config(&config_path);
        write_workflow(
            &workflow_path,
            10_000,
            6,
            "https://workflow-v1.example/graphql",
            "workflow-token-v1",
            "workflow-project-v1",
        );

        let startup = build_validated_startup_config_from_args(
            vec![
                "symphony".to_owned(),
                workflow_path.display().to_string(),
                "--config".to_owned(),
                config_path.display().to_string(),
                "--polling-interval-ms".to_owned(),
                "90000".to_owned(),
                "--max-turns".to_owned(),
                "31".to_owned(),
                "--workspace-root".to_owned(),
                workspace_root.display().to_string(),
                "--tracker-endpoint".to_owned(),
                "https://cli.example/graphql".to_owned(),
                "--tracker-api-key".to_owned(),
                "cli-token".to_owned(),
                "--tracker-project-slug".to_owned(),
                "cli-project".to_owned(),
            ],
            &root,
        )
        .expect("startup args should parse and validate");

        let mut reloader = WorkflowReloader::load_from_path(&startup.workflow_path)
            .expect("workflow should load for startup");
        let initial_effective =
            effective_runtime_config(reloader.current(), &startup.cli_overrides)
                .expect("initial effective config should build");

        assert_eq!(initial_effective.polling.interval_ms, 90_000);
        assert_eq!(initial_effective.agent.max_turns, 31);
        assert_eq!(initial_effective.workspace.root, workspace_root);
        assert_eq!(
            initial_effective.tracker.endpoint,
            "https://cli.example/graphql"
        );
        assert_eq!(
            initial_effective.tracker.api_key.as_deref(),
            Some("cli-token")
        );
        assert_eq!(
            initial_effective.tracker.project_slug.as_deref(),
            Some("cli-project")
        );

        write_workflow(
            &workflow_path,
            2_000,
            3,
            "https://workflow-v2.example/graphql",
            "workflow-token-v2",
            "workflow-project-v2",
        );

        let outcome = reloader.reload();
        assert!(matches!(outcome, WorkflowReloadOutcome::Updated { .. }));

        let reloaded_effective =
            effective_runtime_config(reloader.current(), &startup.cli_overrides)
                .expect("reloaded effective config should build");

        assert_eq!(reloaded_effective.polling.interval_ms, 90_000);
        assert_eq!(reloaded_effective.agent.max_turns, 31);
        assert_eq!(reloaded_effective.workspace.root, workspace_root);
        assert_eq!(
            reloaded_effective.tracker.endpoint,
            "https://cli.example/graphql"
        );
        assert_eq!(
            reloaded_effective.tracker.api_key.as_deref(),
            Some("cli-token")
        );
        assert_eq!(
            reloaded_effective.tracker.project_slug.as_deref(),
            Some("cli-project")
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("symphony-cli-main-{name}-{nonce}"));
        fs::create_dir_all(&path).expect("temp directory should be creatable");
        path
    }

    fn write_runtime_config(path: &Path) {
        fs::write(path, "{}\n").expect("runtime config file should be writable");
    }

    fn write_workflow(
        workflow_path: &Path,
        polling_interval_ms: u64,
        max_turns: u32,
        endpoint: &str,
        api_key: &str,
        project_slug: &str,
    ) {
        fs::write(
            workflow_path,
            format!(
                r#"---
tracker:
  kind: linear
  endpoint: {endpoint}
  api_key: {api_key}
  project_slug: {project_slug}
polling:
  interval_ms: {polling_interval_ms}
agent:
  max_turns: {max_turns}
---
Prompt body.
"#
            ),
        )
        .expect("workflow file should be writable");
    }
}
