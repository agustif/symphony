#![forbid(unsafe_code)]

use anyhow::Context;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use symphony_cli::{CliStartupError, build_validated_startup_config_from_args};
use symphony_config::{
    CliOverrides, ConfigError, ProcessEnv, RuntimeConfig, apply_cli_overrides,
    from_front_matter_with_env,
};
use symphony_domain::IssueId;
use symphony_http::{ApiResponse, REFRESH_ROUTE, handle_get, handle_post};
use symphony_observability::{IssueSnapshot, StateSnapshot};
use symphony_runtime::Runtime;
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};
use symphony_tracker_linear::LinearTracker;
use symphony_workflow::{LoadedWorkflow, WorkflowReloader};
use tokio::sync::{RwLock, broadcast, mpsc};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const EXIT_BOOTSTRAP_ERROR: i32 = 1;
const EXIT_CLI_ARGS: i32 = 2;
const EXIT_RUNTIME_TASK: i32 = 10;
const EXIT_WATCHER_TASK: i32 = 11;
const EXIT_HTTP_TASK: i32 = 12;
const EXIT_LOG_INIT: i32 = 13;

#[derive(Debug)]
struct ExitError {
    code: i32,
    message: String,
}

impl std::fmt::Display for ExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExitError {}

fn exit_error(code: i32, message: impl Into<String>) -> ExitError {
    ExitError {
        code,
        message: message.into(),
    }
}

fn effective_runtime_config(
    loaded_workflow: &LoadedWorkflow,
    cli_overrides: &CliOverrides,
) -> Result<RuntimeConfig, ConfigError> {
    let config = from_front_matter_with_env(&loaded_workflow.document.front_matter, &ProcessEnv)?;
    apply_cli_overrides(config, cli_overrides)
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(error.code);
    }
}

async fn run() -> Result<(), ExitError> {
    let cwd = std::env::current_dir().map_err(|error| {
        exit_error(
            EXIT_BOOTSTRAP_ERROR,
            format!("failed to resolve current working directory: {error}"),
        )
    })?;
    let startup_config = match build_validated_startup_config_from_args(std::env::args_os(), &cwd) {
        Ok(config) => config,
        Err(CliStartupError::CliArgs(error)) => {
            let _ = error.print();
            return Err(exit_error(EXIT_CLI_ARGS, "invalid CLI arguments"));
        }
        Err(error) => {
            return Err(exit_error(
                i32::from(error.exit_code()),
                format!("startup validation failed: {error}"),
            ));
        }
    };
    let cli_overrides = startup_config.cli_overrides.clone();

    // 1. Load initial workflow and config
    let mut reloader =
        WorkflowReloader::load_from_path(&startup_config.workflow_path).map_err(|error| {
            exit_error(
                EXIT_BOOTSTRAP_ERROR,
                format!("workflow load failed: {error}"),
            )
        })?;
    let initial_config =
        effective_runtime_config(reloader.current(), &cli_overrides).map_err(|error| {
            exit_error(
                EXIT_BOOTSTRAP_ERROR,
                format!("effective runtime config failed: {error}"),
            )
        })?;
    let _log_guard = init_tracing(
        &initial_config.log_level.level,
        startup_config.logs_root.as_deref(),
    )
    .map_err(|error| exit_error(EXIT_LOG_INIT, format!("logging init failed: {error}")))?;

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
    runtime
        .set_prompt_template(reloader.current().document.prompt_body.clone())
        .await;

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
    let watcher_runtime = runtime.clone();
    let watcher_overrides = cli_overrides.clone();
    let refresh_tx_watcher = refresh_tx.clone();
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
                                watcher_runtime
                                    .set_prompt_template(reloader.current().document.prompt_body.clone())
                                    .await;
                                let _ = refresh_tx_watcher.try_send(());
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

    let http_handle = if let Some(port) = initial_config.server.port {
        let http_shutdown = shutdown_tx.subscribe();
        let http_state = HttpServerState {
            runtime: runtime.clone(),
            tracker: tracker.clone(),
            refresh_tx: refresh_tx.clone(),
        };
        Some(tokio::spawn(async move {
            run_http_server(http_state, port, http_shutdown).await
        }))
    } else {
        None
    };

    println!(
        "Symphony Rust Runtime started (workflow: {})",
        startup_config.workflow_path.display()
    );

    // 6. Handle Shutdown
    wait_for_shutdown_signal().await.map_err(|error| {
        exit_error(
            EXIT_BOOTSTRAP_ERROR,
            format!("shutdown signal failed: {error}"),
        )
    })?;
    println!("Shutting down...");
    let _ = shutdown_tx.send(());

    let (poll_result, watcher_result) = tokio::join!(poll_handle, watcher_handle);
    if let Err(error) = poll_result {
        return Err(exit_error(
            EXIT_RUNTIME_TASK,
            format!("runtime poll loop task failed: {error}"),
        ));
    }
    if let Err(error) = watcher_result {
        return Err(exit_error(
            EXIT_WATCHER_TASK,
            format!("workflow watcher task failed: {error}"),
        ));
    }
    if let Some(handle) = http_handle {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(exit_error(
                    EXIT_HTTP_TASK,
                    format!("HTTP server task failed: {error}"),
                ));
            }
            Err(error) => {
                return Err(exit_error(
                    EXIT_HTTP_TASK,
                    format!("HTTP server join failed: {error}"),
                ));
            }
        }
    }

    Ok(())
}

fn init_tracing(
    log_level: &str,
    logs_root: Option<&Path>,
) -> anyhow::Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    let env_filter = EnvFilter::try_new(log_level)
        .or_else(|_| EnvFilter::try_new("info"))
        .context("failed to build log level filter")?;
    let fmt_layer = tracing_subscriber::fmt::layer();

    if let Some(logs_root) = logs_root {
        std::fs::create_dir_all(logs_root).with_context(|| {
            format!(
                "failed to create logs root directory {}",
                logs_root.display()
            )
        })?;
        let appender = tracing_appender::rolling::never(logs_root, "symphony.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer.with_writer(writer).with_ansi(false))
            .init();
        return Ok(Some(guard));
    }

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
    Ok(None)
}

async fn wait_for_shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result?;
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
    }

    Ok(())
}

#[derive(Clone)]
struct HttpServerState {
    runtime: Arc<Runtime<LinearTracker>>,
    tracker: Arc<LinearTracker>,
    refresh_tx: mpsc::Sender<()>,
}

async fn run_http_server(
    state: HttpServerState,
    port: u16,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let app = Router::new().fallback(any(http_fallback)).with_state(state);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .with_context(|| format!("failed to bind HTTP listener on 127.0.0.1:{port}"))?;
    let listen_addr = listener
        .local_addr()
        .context("failed to resolve bound HTTP listener address")?;
    tracing::info!(http_listen_addr = %listen_addr, "HTTP server started");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
        })
        .await
        .context("HTTP server terminated with error")?;
    Ok(())
}

async fn http_fallback(
    State(state): State<HttpServerState>,
    method: Method,
    uri: axum::http::Uri,
) -> Response {
    let path = uri.path();
    let snapshot = match build_state_snapshot(state.runtime.as_ref(), state.tracker.as_ref()).await
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return into_http_response(ApiResponse::unavailable(format!(
                "failed to load runtime snapshot: {error}"
            )));
        }
    };

    let response = match method {
        Method::GET => handle_get(path, &snapshot),
        Method::POST => {
            let response = handle_post(path, &snapshot);
            if path == REFRESH_ROUTE
                && response.status == 202
                && let Err(error) = state.refresh_tx.try_send(())
            {
                tracing::debug!("refresh signal already queued: {error}");
            }
            response
        }
        _ => unsupported_method_response(path, &snapshot),
    };

    into_http_response(response)
}

fn unsupported_method_response(path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    let get_response = handle_get(path, snapshot);
    if get_response.status != StatusCode::NOT_FOUND.as_u16() {
        return ApiResponse::method_not_allowed(format!("method not allowed for `{path}`"));
    }

    let post_response = handle_post(path, snapshot);
    if post_response.status != StatusCode::NOT_FOUND.as_u16() {
        return ApiResponse::method_not_allowed(format!("method not allowed for `{path}`"));
    }

    ApiResponse::not_found("route not found")
}

async fn build_state_snapshot(
    runtime: &Runtime<LinearTracker>,
    tracker: &LinearTracker,
) -> Result<StateSnapshot, TrackerError> {
    let runtime_snapshot = runtime.snapshot().await;
    let state = runtime.state().await;
    let mut issue_ids = HashSet::new();
    issue_ids.extend(state.claimed.iter().cloned());
    issue_ids.extend(state.running.keys().cloned());
    issue_ids.extend(state.retry_attempts.keys().cloned());

    let mut ordered_ids = issue_ids.into_iter().collect::<Vec<_>>();
    ordered_ids.sort_by(|left, right| left.0.cmp(&right.0));

    let candidates = tracker.fetch_candidates().await?;
    let candidates_by_id = candidates
        .into_iter()
        .map(|issue| (issue.id.clone(), issue))
        .collect::<HashMap<IssueId, TrackerIssue>>();
    let states_by_id = if ordered_ids.is_empty() {
        HashMap::new()
    } else {
        tracker.fetch_states_by_ids(&ordered_ids).await?
    };

    let mut issues = Vec::with_capacity(ordered_ids.len());
    for issue_id in ordered_ids {
        let retry_attempts = state
            .retry_attempts
            .get(&issue_id)
            .map_or(0, |entry| entry.attempt);
        let identifier = state
            .running
            .get(&issue_id)
            .and_then(|entry| entry.identifier.clone())
            .or_else(|| {
                candidates_by_id
                    .get(&issue_id)
                    .map(|issue| issue.identifier.clone())
            })
            .unwrap_or_else(|| issue_id.0.clone());
        let tracker_state = states_by_id
            .get(&issue_id)
            .map(|state| state.as_str().to_owned())
            .or_else(|| {
                candidates_by_id
                    .get(&issue_id)
                    .map(|issue| issue.state.as_str().to_owned())
            });
        let state_name = tracker_state.unwrap_or_else(|| {
            if retry_attempts > 0 {
                "Retrying".to_owned()
            } else {
                "Running".to_owned()
            }
        });

        issues.push(IssueSnapshot {
            id: issue_id,
            identifier,
            state: state_name,
            retry_attempts,
        });
    }
    issues.sort_by(|left, right| left.identifier.cmp(&right.identifier));

    Ok(StateSnapshot {
        runtime: runtime_snapshot,
        issues,
    })
}

fn into_http_response(response: ApiResponse) -> Response {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        [(header::CONTENT_TYPE, response.content_type)],
        Body::from(response.rendered_body),
    )
        .into_response()
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
