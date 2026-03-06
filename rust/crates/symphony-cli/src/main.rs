#![forbid(unsafe_code)]

use anyhow::Context;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use std::future::Future;
use std::net::Ipv4Addr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use symphony_cli::{
    CliStartupError, HostOwnedConfigChange, WorkflowReloadDisposition,
    build_validated_startup_config_from_args, classify_workflow_reload,
};
use symphony_config::{
    CliOverrides, ConfigError, ProcessEnv, RuntimeConfig, apply_cli_overrides,
    from_front_matter_with_env,
};
use symphony_http::{
    ApiResponse, REFRESH_ROUTE, handle_get, handle_get_with_workspace_envelope, handle_post,
    refresh_to_json_with_coalesced,
};
use symphony_observability::{
    SnapshotErrorCode, SnapshotErrorView, StateSnapshot, StateSnapshotEnvelope,
};
use symphony_runtime::Runtime;
use symphony_tracker::TrackerState;
use symphony_tracker_linear::LinearTracker;
use symphony_workflow::{
    LoadedWorkflow, WorkflowChangeStamp, WorkflowReloadOutcome, WorkflowReloader,
};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const EXIT_BOOTSTRAP_ERROR: i32 = 1;
const EXIT_CLI_ARGS: i32 = 2;
const EXIT_RUNTIME_TASK: i32 = 10;
const EXIT_WATCHER_TASK: i32 = 11;
const EXIT_HTTP_TASK: i32 = 12;
const EXIT_LOG_INIT: i32 = 13;
const EXIT_RESTART_REQUIRED: i32 = 14;
const HTTP_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(15);
const BACKGROUND_TASK_PROBE_INTERVAL: Duration = Duration::from_millis(100);
// Give cooperative shutdown a brief chance before force-aborting siblings after host failure.
const BACKGROUND_TASK_SHUTDOWN_GRACE: Duration = Duration::from_millis(250);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundTaskKind {
    RuntimePollLoop,
    WorkflowWatcher,
    HttpServer,
}

impl BackgroundTaskKind {
    fn exit_code(self) -> i32 {
        match self {
            Self::RuntimePollLoop => EXIT_RUNTIME_TASK,
            Self::WorkflowWatcher => EXIT_WATCHER_TASK,
            Self::HttpServer => EXIT_HTTP_TASK,
        }
    }

    fn task_label(self) -> &'static str {
        match self {
            Self::RuntimePollLoop => "runtime poll loop",
            Self::WorkflowWatcher => "workflow watcher",
            Self::HttpServer => "HTTP server",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HostLifecycleEvent {
    ShutdownRequested,
    BackgroundTaskExited(BackgroundTaskKind),
    RestartRequired(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BackgroundTaskSignal {
    Exited(BackgroundTaskKind),
    RestartRequired(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefreshEnqueueOutcome {
    Delivered,
    Coalesced,
    Unavailable,
}

impl RefreshEnqueueOutcome {
    fn as_log_value(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Coalesced => "coalesced",
            Self::Unavailable => "unavailable",
        }
    }
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
    let project_slug = initial_config.tracker.project_slug.clone().ok_or_else(|| {
        exit_error(
            EXIT_BOOTSTRAP_ERROR,
            "validated runtime config missing tracker.project_slug",
        )
    })?;
    let tracker = Arc::new(
        LinearTracker::new(
            initial_config.tracker.endpoint.clone(),
            initial_config.tracker.api_key.clone().unwrap_or_default(),
            project_slug,
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
    let (task_exit_tx, mut task_exit_rx) = mpsc::unbounded_channel();

    let shutdown_rx = shutdown_tx.subscribe();
    let runtime_poll = runtime.clone();
    let config_poll = config_arc.clone();
    let task_exit_tx_poll = task_exit_tx.clone();

    // 4. Start Poll Loop
    let poll_handle = tokio::spawn(async move {
        runtime_poll
            .run_poll_loop(config_poll, refresh_rx, shutdown_rx)
            .await;
        let _ = task_exit_tx_poll.send(BackgroundTaskSignal::Exited(
            BackgroundTaskKind::RuntimePollLoop,
        ));
    });

    // 5. Start Workflow Watcher (simple polling for now)
    let watcher_config = config_arc.clone();
    let watcher_runtime = runtime.clone();
    let watcher_overrides = cli_overrides.clone();
    let refresh_tx_watcher = refresh_tx.clone();
    let mut watcher_shutdown = shutdown_tx.subscribe();
    let task_exit_tx_watcher = task_exit_tx.clone();
    let watcher_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        let mut restart_required = None;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let WorkflowReloadAction::RestartRequired(message) = apply_workflow_reload(
                        &mut reloader,
                        &watcher_config,
                        &watcher_runtime,
                        &watcher_overrides,
                        &refresh_tx_watcher,
                    ).await {
                        restart_required = Some(message);
                        break;
                    }
                }
                _ = watcher_shutdown.recv() => break,
            }
        }
        if let Some(message) = restart_required {
            let _ = task_exit_tx_watcher.send(BackgroundTaskSignal::RestartRequired(message));
        } else {
            let _ = task_exit_tx_watcher.send(BackgroundTaskSignal::Exited(
                BackgroundTaskKind::WorkflowWatcher,
            ));
        }
    });

    let http_handle = if let Some(port) = initial_config.server.port {
        let http_shutdown = shutdown_tx.subscribe();
        let http_state = HttpServerState {
            runtime: runtime.clone(),
            config: config_arc.clone(),
            refresh_tx: refresh_tx.clone(),
            last_snapshot: Arc::new(RwLock::new(None)),
        };
        let task_exit_tx_http = task_exit_tx.clone();
        Some(tokio::spawn(async move {
            let result = run_http_server(http_state, port, http_shutdown).await;
            let _ = task_exit_tx_http
                .send(BackgroundTaskSignal::Exited(BackgroundTaskKind::HttpServer));
            result
        }))
    } else {
        None
    };
    drop(task_exit_tx);

    println!(
        "Symphony Rust Runtime started (workflow: {})",
        startup_config.workflow_path.display()
    );

    // 6. Supervise background tasks and handle shutdown
    let lifecycle_event = wait_for_host_lifecycle_event(
        &poll_handle,
        &watcher_handle,
        http_handle.as_ref(),
        &mut task_exit_rx,
        wait_for_shutdown_signal(),
        BACKGROUND_TASK_PROBE_INTERVAL,
    )
    .await?;
    println!("Shutting down...");
    let _ = shutdown_tx.send(());

    match lifecycle_event {
        HostLifecycleEvent::ShutdownRequested => {
            await_background_tasks(poll_handle, watcher_handle, http_handle, None).await?;
        }
        HostLifecycleEvent::BackgroundTaskExited(task_kind) => {
            await_background_tasks(poll_handle, watcher_handle, http_handle, Some(task_kind))
                .await?;
            return Err(unexpected_background_task_exit(task_kind));
        }
        HostLifecycleEvent::RestartRequired(message) => {
            await_background_tasks(poll_handle, watcher_handle, http_handle, None).await?;
            return Err(exit_error(EXIT_RESTART_REQUIRED, message));
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
    config: Arc<RwLock<RuntimeConfig>>,
    refresh_tx: mpsc::Sender<()>,
    last_snapshot: Arc<RwLock<Option<StateSnapshot>>>,
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

async fn await_background_tasks(
    mut poll_handle: JoinHandle<()>,
    mut watcher_handle: JoinHandle<()>,
    mut http_handle: Option<JoinHandle<anyhow::Result<()>>>,
    unexpected_exit: Option<BackgroundTaskKind>,
) -> Result<(), ExitError> {
    if unexpected_exit.is_some() {
        tokio::time::sleep(BACKGROUND_TASK_SHUTDOWN_GRACE).await;
        abort_background_task_if_running(&mut poll_handle, unexpected_exit);
        abort_background_task_if_running(&mut watcher_handle, unexpected_exit);
        abort_http_task_if_running(http_handle.as_mut(), unexpected_exit);
    }

    match unexpected_exit {
        Some(BackgroundTaskKind::RuntimePollLoop) => {
            map_unit_task_result(
                BackgroundTaskKind::RuntimePollLoop,
                poll_handle.await,
                false,
                false,
            )?;
            map_unit_task_result(
                BackgroundTaskKind::WorkflowWatcher,
                watcher_handle.await,
                true,
                true,
            )?;
            await_http_task(http_handle, true, true).await?;
        }
        Some(BackgroundTaskKind::WorkflowWatcher) => {
            map_unit_task_result(
                BackgroundTaskKind::WorkflowWatcher,
                watcher_handle.await,
                false,
                false,
            )?;
            map_unit_task_result(
                BackgroundTaskKind::RuntimePollLoop,
                poll_handle.await,
                true,
                true,
            )?;
            await_http_task(http_handle, true, true).await?;
        }
        Some(BackgroundTaskKind::HttpServer) => {
            await_http_task(http_handle, false, false).await?;
            map_unit_task_result(
                BackgroundTaskKind::RuntimePollLoop,
                poll_handle.await,
                true,
                true,
            )?;
            map_unit_task_result(
                BackgroundTaskKind::WorkflowWatcher,
                watcher_handle.await,
                true,
                true,
            )?;
        }
        None => {
            let (poll_result, watcher_result) = tokio::join!(poll_handle, watcher_handle);
            map_unit_task_result(
                BackgroundTaskKind::RuntimePollLoop,
                poll_result,
                true,
                false,
            )?;
            map_unit_task_result(
                BackgroundTaskKind::WorkflowWatcher,
                watcher_result,
                true,
                false,
            )?;
            await_http_task(http_handle, true, false).await?;
        }
    }

    Ok(())
}

async fn http_fallback(
    State(state): State<HttpServerState>,
    method: Method,
    uri: axum::http::Uri,
) -> Response {
    let path = uri.path();
    let snapshot = resolve_http_snapshot(
        async { Ok(state.runtime.state_snapshot().await) },
        HTTP_SNAPSHOT_TIMEOUT,
        state.last_snapshot.as_ref(),
    )
    .await;

    let response = match method {
        Method::GET => get_http_get_response(path, &snapshot, &state.config).await,
        Method::POST => post_http_response(path, &snapshot, &state.refresh_tx),
        _ => unsupported_method_response(path, &snapshot),
    };

    into_http_response(response)
}

fn post_http_response(
    path: &str,
    envelope: &StateSnapshotEnvelope,
    refresh_tx: &mpsc::Sender<()>,
) -> ApiResponse {
    let probe_snapshot = envelope.snapshot.clone().unwrap_or_default();
    let mut response = handle_post(path, &probe_snapshot);
    if path == REFRESH_ROUTE && response.status == 202 {
        match enqueue_refresh_signal(refresh_tx, "http_refresh_route") {
            RefreshEnqueueOutcome::Delivered => {}
            RefreshEnqueueOutcome::Coalesced => {
                response = ApiResponse::accepted(refresh_to_json_with_coalesced(true));
            }
            RefreshEnqueueOutcome::Unavailable => {
                return ApiResponse::orchestrator_unavailable();
            }
        }
    }

    response
}

fn unsupported_method_response(path: &str, envelope: &StateSnapshotEnvelope) -> ApiResponse {
    let probe_snapshot = envelope.snapshot.clone().unwrap_or_default();
    let get_response = handle_get(path, &probe_snapshot);
    if get_response.status != StatusCode::NOT_FOUND.as_u16() {
        return ApiResponse::method_not_allowed();
    }

    let post_response = handle_post(path, &probe_snapshot);
    if post_response.status != StatusCode::NOT_FOUND.as_u16() {
        return ApiResponse::method_not_allowed();
    }

    ApiResponse::not_found()
}

async fn resolve_http_snapshot<F>(
    snapshot_future: F,
    timeout: Duration,
    last_snapshot: &RwLock<Option<StateSnapshot>>,
) -> StateSnapshotEnvelope
where
    F: Future<Output = Result<StateSnapshot, SnapshotErrorCode>>,
{
    match tokio::time::timeout(timeout, snapshot_future).await {
        Ok(Ok(snapshot)) => {
            let mut last_snapshot_guard = last_snapshot.write().await;
            *last_snapshot_guard = Some(snapshot.clone());
            StateSnapshotEnvelope::ready(snapshot)
        }
        Ok(Err(code)) => {
            degraded_snapshot_from_cache(
                last_snapshot,
                SnapshotErrorView::new(code, code.default_message()),
            )
            .await
        }
        Err(_) => degraded_snapshot_from_cache(last_snapshot, SnapshotErrorView::timeout()).await,
    }
}

async fn degraded_snapshot_from_cache(
    last_snapshot: &RwLock<Option<StateSnapshot>>,
    error: SnapshotErrorView,
) -> StateSnapshotEnvelope {
    let cached_snapshot = last_snapshot.read().await.clone();
    if let Some(snapshot) = cached_snapshot {
        return StateSnapshotEnvelope::stale(snapshot, error);
    }

    StateSnapshotEnvelope::from_error(error)
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

async fn wait_for_host_lifecycle_event<F>(
    poll_handle: &JoinHandle<()>,
    watcher_handle: &JoinHandle<()>,
    http_handle: Option<&JoinHandle<anyhow::Result<()>>>,
    task_exit_rx: &mut mpsc::UnboundedReceiver<BackgroundTaskSignal>,
    shutdown_signal: F,
    probe_interval: Duration,
) -> Result<HostLifecycleEvent, ExitError>
where
    F: Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(shutdown_signal);
    let mut interval = tokio::time::interval(probe_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            shutdown_result = &mut shutdown_signal => {
                shutdown_result.map_err(|error| {
                    exit_error(
                        EXIT_BOOTSTRAP_ERROR,
                        format!("shutdown signal failed: {error}"),
                    )
                })?;
                return Ok(HostLifecycleEvent::ShutdownRequested);
            }
            task_exit = task_exit_rx.recv() => {
                if let Some(signal) = task_exit {
                    return Ok(match signal {
                        BackgroundTaskSignal::Exited(task_kind) => {
                            HostLifecycleEvent::BackgroundTaskExited(task_kind)
                        }
                        BackgroundTaskSignal::RestartRequired(message) => {
                            HostLifecycleEvent::RestartRequired(message)
                        }
                    });
                }
            }
            _ = interval.tick() => {
                if poll_handle.is_finished() {
                    return Ok(HostLifecycleEvent::BackgroundTaskExited(
                        BackgroundTaskKind::RuntimePollLoop,
                    ));
                }
                if watcher_handle.is_finished() {
                    return Ok(HostLifecycleEvent::BackgroundTaskExited(
                        BackgroundTaskKind::WorkflowWatcher,
                    ));
                }
                if http_handle.is_some_and(|handle| handle.is_finished()) {
                    return Ok(HostLifecycleEvent::BackgroundTaskExited(
                        BackgroundTaskKind::HttpServer,
                    ));
                }
            }
        }
    }
}

fn map_unit_task_result(
    task_kind: BackgroundTaskKind,
    result: Result<(), tokio::task::JoinError>,
    allow_clean_exit: bool,
    allow_cancelled: bool,
) -> Result<(), ExitError> {
    match result {
        Ok(()) if allow_clean_exit => Ok(()),
        Ok(()) => Err(unexpected_background_task_exit(task_kind)),
        Err(error) if error.is_cancelled() && allow_cancelled => Ok(()),
        Err(error) => Err(exit_error(
            task_kind.exit_code(),
            format!("{} task failed: {error}", task_kind.task_label()),
        )),
    }
}

fn map_http_task_result(
    result: Result<anyhow::Result<()>, tokio::task::JoinError>,
    allow_clean_exit: bool,
    allow_cancelled: bool,
) -> Result<(), ExitError> {
    match result {
        Ok(Ok(())) if allow_clean_exit => Ok(()),
        Ok(Ok(())) => Err(unexpected_background_task_exit(
            BackgroundTaskKind::HttpServer,
        )),
        Ok(Err(error)) => Err(exit_error(
            EXIT_HTTP_TASK,
            format!("HTTP server task failed: {error}"),
        )),
        Err(error) if error.is_cancelled() && allow_cancelled => Ok(()),
        Err(error) => Err(exit_error(
            EXIT_HTTP_TASK,
            format!("HTTP server join failed: {error}"),
        )),
    }
}

async fn await_http_task(
    http_handle: Option<JoinHandle<anyhow::Result<()>>>,
    allow_clean_exit: bool,
    allow_cancelled: bool,
) -> Result<(), ExitError> {
    if let Some(handle) = http_handle {
        map_http_task_result(handle.await, allow_clean_exit, allow_cancelled)?;
    }

    Ok(())
}

fn abort_background_task_if_running(
    handle: &mut JoinHandle<()>,
    unexpected_exit: Option<BackgroundTaskKind>,
) {
    let skip_abort = matches!(unexpected_exit, Some(BackgroundTaskKind::RuntimePollLoop))
        && handle.is_finished();
    if !skip_abort && !handle.is_finished() {
        handle.abort();
    }
}

fn abort_http_task_if_running(
    handle: Option<&mut JoinHandle<anyhow::Result<()>>>,
    unexpected_exit: Option<BackgroundTaskKind>,
) {
    if matches!(unexpected_exit, Some(BackgroundTaskKind::HttpServer)) {
        return;
    }

    if let Some(handle) = handle
        && !handle.is_finished()
    {
        handle.abort();
    }
}

fn unexpected_background_task_exit(task_kind: BackgroundTaskKind) -> ExitError {
    exit_error(
        task_kind.exit_code(),
        format!(
            "{} task exited unexpectedly before shutdown",
            task_kind.task_label()
        ),
    )
}

fn increment_runtime_config_version(
    current_config: &RuntimeConfig,
    mut next_config: RuntimeConfig,
) -> RuntimeConfig {
    next_config.version = current_config.version.saturating_add(1);
    next_config
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WorkflowReloadAction {
    NoChange,
    Applied,
    RestartRequired(String),
}

async fn apply_workflow_reload(
    reloader: &mut WorkflowReloader,
    config: &Arc<RwLock<RuntimeConfig>>,
    runtime: &Arc<Runtime<LinearTracker>>,
    cli_overrides: &CliOverrides,
    refresh_tx: &mpsc::Sender<()>,
) -> WorkflowReloadAction {
    let previous_reloader = reloader.clone();
    match reloader.reload() {
        WorkflowReloadOutcome::Unchanged { .. } => WorkflowReloadAction::NoChange,
        WorkflowReloadOutcome::Retained { retained, error } => {
            tracing::error!(
                workflow_path = %reloader.path().display(),
                retained_content_hash = retained.content_hash,
                retained_content_bytes = retained.content_bytes,
                retained_modified_unix_ms = ?retained.modified_unix_ms,
                error = %error,
                "{}",
                retained_reload_message(reloader.path(), &retained, &error),
            );
            WorkflowReloadAction::NoChange
        }
        WorkflowReloadOutcome::Updated { previous, current } => {
            match effective_runtime_config(reloader.current(), cli_overrides) {
                Ok(next_config) => {
                    let current_config = config.read().await.clone();
                    if let WorkflowReloadDisposition::RestartRequired { reasons } =
                        classify_workflow_reload(&current_config, &next_config)
                    {
                        let restart_reason_paths = reasons
                            .iter()
                            .map(|reason| reason.field_path())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let message = restart_required_reload_message(
                            reloader.path(),
                            &previous,
                            &current,
                            reasons.as_slice(),
                        );
                        tracing::warn!(
                            workflow_path = %reloader.path().display(),
                            previous_content_hash = previous.content_hash,
                            previous_content_bytes = previous.content_bytes,
                            current_content_hash = current.content_hash,
                            current_content_bytes = current.content_bytes,
                            restart_required_for = %restart_reason_paths,
                            "{}",
                            message,
                        );
                        *reloader = previous_reloader;
                        return WorkflowReloadAction::RestartRequired(message);
                    }
                    let applied_config = {
                        let mut guard = config.write().await;
                        let applied = increment_runtime_config_version(&guard, next_config);
                        *guard = applied.clone();
                        applied
                    };
                    runtime
                        .set_prompt_template(reloader.current().document.prompt_body.clone())
                        .await;
                    let refresh_outcome = enqueue_refresh_signal(refresh_tx, "workflow_watcher");
                    tracing::info!(
                        workflow_path = %reloader.path().display(),
                        previous_content_hash = previous.content_hash,
                        previous_content_bytes = previous.content_bytes,
                        current_content_hash = current.content_hash,
                        current_content_bytes = current.content_bytes,
                        config_version = applied_config.version,
                        workspace_root = %applied_config.workspace.root.display(),
                        refresh_delivery = refresh_outcome.as_log_value(),
                        "workflow reload applied"
                    );
                    WorkflowReloadAction::Applied
                }
                Err(error) => {
                    tracing::error!(
                        workflow_path = %reloader.path().display(),
                        previous_content_hash = previous.content_hash,
                        previous_content_bytes = previous.content_bytes,
                        current_content_hash = current.content_hash,
                        current_content_bytes = current.content_bytes,
                        error = %error,
                        "{}",
                        invalid_effective_config_reload_message(
                            reloader.path(),
                            &previous,
                            &current,
                            &error,
                        ),
                    );
                    *reloader = previous_reloader;
                    WorkflowReloadAction::NoChange
                }
            }
        }
    }
}

fn restart_required_reload_message(
    workflow_path: &Path,
    previous: &WorkflowChangeStamp,
    current: &WorkflowChangeStamp,
    reasons: &[HostOwnedConfigChange],
) -> String {
    format!(
        "workflow reload changed host-owned config for {}; restart required before applying new workflow (previous_hash={}, current_hash={}, reasons={})",
        workflow_path.display(),
        previous.content_hash,
        current.content_hash,
        reasons
            .iter()
            .map(|reason| reason.field_path())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn retained_reload_message(
    workflow_path: &Path,
    retained: &WorkflowChangeStamp,
    error: &impl std::fmt::Display,
) -> String {
    format!(
        "workflow reload invalid for {}; retaining last applied workflow (hash={}, bytes={}, modified_unix_ms={:?}): {}",
        workflow_path.display(),
        retained.content_hash,
        retained.content_bytes,
        retained.modified_unix_ms,
        error
    )
}

fn invalid_effective_config_reload_message(
    workflow_path: &Path,
    previous: &WorkflowChangeStamp,
    current: &WorkflowChangeStamp,
    error: &impl std::fmt::Display,
) -> String {
    format!(
        "workflow reload produced an invalid effective runtime config for {}; retaining previously applied config and prompt (previous_hash={}, current_hash={}): {}",
        workflow_path.display(),
        previous.content_hash,
        current.content_hash,
        error
    )
}

fn enqueue_refresh_signal(refresh_tx: &mpsc::Sender<()>, source: &str) -> RefreshEnqueueOutcome {
    match refresh_tx.try_send(()) {
        Ok(()) => {
            tracing::info!(refresh_source = source, "refresh signal queued");
            RefreshEnqueueOutcome::Delivered
        }
        Err(TrySendError::Full(_)) => {
            tracing::info!(
                refresh_source = source,
                "refresh signal coalesced with an already queued refresh"
            );
            RefreshEnqueueOutcome::Coalesced
        }
        Err(TrySendError::Closed(_)) => {
            tracing::warn!(
                refresh_source = source,
                "refresh signal unavailable: receiver closed"
            );
            RefreshEnqueueOutcome::Unavailable
        }
    }
}

async fn current_workspace_root(config: &RwLock<RuntimeConfig>) -> PathBuf {
    config.read().await.workspace.root.clone()
}

async fn get_http_get_response(
    path: &str,
    snapshot: &StateSnapshotEnvelope,
    config: &RwLock<RuntimeConfig>,
) -> ApiResponse {
    let workspace_root = current_workspace_root(config).await;
    handle_get_with_workspace_envelope(path, snapshot, Some(workspace_root.as_path()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use symphony_http::issue_route;
    use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};

    #[test]
    fn startup_effective_config_uses_cli_overrides_over_front_matter() {
        let root = temp_dir("startup_effective_config_uses_cli_overrides_over_front_matter");
        let workflow_path = root.join("WORKFLOW.md");
        let config_path = root.join("missing-runtime-config.json");
        let workspace_root = root.join("cli-workspaces");

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
        let config_path = root.join("missing-runtime-config.json");
        let workspace_root = root.join("cli-workspaces");

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

    #[tokio::test]
    async fn await_background_tasks_maps_runtime_task_failure_to_exit_code() {
        let poll_handle = tokio::spawn(async move {
            panic!("poll failed");
        });
        let watcher_handle = tokio::spawn(async {});

        let error = await_background_tasks(poll_handle, watcher_handle, None, None)
            .await
            .expect_err("runtime task failure should surface");

        assert_eq!(error.code, EXIT_RUNTIME_TASK);
        assert!(error.message.contains("runtime poll loop task failed"));
    }

    #[tokio::test]
    async fn await_background_tasks_maps_http_task_failure_to_exit_code() {
        let poll_handle = tokio::spawn(async {});
        let watcher_handle = tokio::spawn(async {});
        let http_handle = tokio::spawn(async { Err(anyhow::anyhow!("bind failed")) });

        let error = await_background_tasks(poll_handle, watcher_handle, Some(http_handle), None)
            .await
            .expect_err("http task failure should surface");

        assert_eq!(error.code, EXIT_HTTP_TASK);
        assert!(error.message.contains("HTTP server task failed"));
    }

    #[tokio::test]
    async fn run_http_server_returns_error_when_port_is_already_bound() {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("port should bind");
        let port = listener
            .local_addr()
            .expect("listener addr should resolve")
            .port();
        let (refresh_tx, _) = mpsc::channel(1);
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let tracker = Arc::new(LinearTracker::new(
            "https://example.invalid/graphql".to_owned(),
            "token".to_owned(),
            "SYM".to_owned(),
        ));
        let state = HttpServerState {
            runtime: Arc::new(Runtime::new(tracker)),
            config: Arc::new(RwLock::new(runtime_config_with_workspace_root(temp_dir(
                "run_http_server_returns_error_when_port_is_already_bound",
            )))),
            refresh_tx,
            last_snapshot: Arc::new(RwLock::new(None)),
        };

        let error = run_http_server(state, port, shutdown_rx)
            .await
            .expect_err("bind conflict should fail startup");

        assert!(error.to_string().contains("failed to bind HTTP listener"));
    }

    #[tokio::test]
    async fn wait_for_host_lifecycle_event_detects_early_background_task_exit() {
        let poll_handle = tokio::spawn(async {});
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let (_task_exit_tx, mut task_exit_rx) = mpsc::unbounded_channel();

        let event = wait_for_host_lifecycle_event(
            &poll_handle,
            &watcher_handle,
            None,
            &mut task_exit_rx,
            std::future::pending::<anyhow::Result<()>>(),
            Duration::from_millis(1),
        )
        .await
        .expect("background task exit should be detected");

        assert_eq!(
            event,
            HostLifecycleEvent::BackgroundTaskExited(BackgroundTaskKind::RuntimePollLoop)
        );

        watcher_handle.abort();
    }

    #[tokio::test]
    async fn wait_for_host_lifecycle_event_uses_task_exit_notifications_without_probe_delay() {
        let poll_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let (task_exit_tx, mut task_exit_rx) = mpsc::unbounded_channel();
        task_exit_tx
            .send(BackgroundTaskSignal::Exited(
                BackgroundTaskKind::WorkflowWatcher,
            ))
            .expect("task exit signal should be queued");

        let event = tokio::time::timeout(
            Duration::from_millis(50),
            wait_for_host_lifecycle_event(
                &poll_handle,
                &watcher_handle,
                None,
                &mut task_exit_rx,
                std::future::pending::<anyhow::Result<()>>(),
                Duration::from_secs(60),
            ),
        )
        .await
        .expect("task exit notification should bypass long probe interval")
        .expect("background task exit should be detected");

        assert_eq!(
            event,
            HostLifecycleEvent::BackgroundTaskExited(BackgroundTaskKind::WorkflowWatcher)
        );

        poll_handle.abort();
        watcher_handle.abort();
    }

    #[tokio::test]
    async fn wait_for_host_lifecycle_event_returns_restart_required_signal() {
        let poll_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let (task_exit_tx, mut task_exit_rx) = mpsc::unbounded_channel();
        task_exit_tx
            .send(BackgroundTaskSignal::RestartRequired(
                "restart required for host-owned config".to_owned(),
            ))
            .expect("restart-required signal should be queued");

        let event = tokio::time::timeout(
            Duration::from_millis(50),
            wait_for_host_lifecycle_event(
                &poll_handle,
                &watcher_handle,
                None,
                &mut task_exit_rx,
                std::future::pending::<anyhow::Result<()>>(),
                Duration::from_secs(60),
            ),
        )
        .await
        .expect("restart-required signal should bypass long probe interval")
        .expect("restart-required event should be detected");

        assert_eq!(
            event,
            HostLifecycleEvent::RestartRequired(
                "restart required for host-owned config".to_owned()
            )
        );

        poll_handle.abort();
        watcher_handle.abort();
    }

    #[tokio::test]
    async fn await_background_tasks_reports_unexpected_clean_watcher_exit() {
        let poll_handle = tokio::spawn(async {});
        let watcher_handle = tokio::spawn(async {});

        let error = await_background_tasks(
            poll_handle,
            watcher_handle,
            None,
            Some(BackgroundTaskKind::WorkflowWatcher),
        )
        .await
        .expect_err("unexpected watcher exit should fail fast");

        assert_eq!(error.code, EXIT_WATCHER_TASK);
        assert!(
            error
                .message
                .contains("workflow watcher task exited unexpectedly")
        );
    }

    #[tokio::test]
    async fn await_background_tasks_aborts_runtime_after_unexpected_watcher_exit() {
        let poll_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let watcher_handle = tokio::spawn(async {});

        let error = tokio::time::timeout(
            Duration::from_secs(2),
            await_background_tasks(
                poll_handle,
                watcher_handle,
                None,
                Some(BackgroundTaskKind::WorkflowWatcher),
            ),
        )
        .await
        .expect("sibling abort should prevent hangs")
        .expect_err("unexpected watcher exit should surface");

        assert_eq!(error.code, EXIT_WATCHER_TASK);
        assert!(
            error
                .message
                .contains("workflow watcher task exited unexpectedly")
        );
    }

    #[tokio::test]
    async fn await_background_tasks_aborts_runtime_and_watcher_after_unexpected_http_exit() {
        let poll_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let http_handle = tokio::spawn(async { Err(anyhow::anyhow!("bind failed")) });

        let error = tokio::time::timeout(
            Duration::from_secs(2),
            await_background_tasks(
                poll_handle,
                watcher_handle,
                Some(http_handle),
                Some(BackgroundTaskKind::HttpServer),
            ),
        )
        .await
        .expect("sibling abort should prevent hangs")
        .expect_err("unexpected http exit should surface");

        assert_eq!(error.code, EXIT_HTTP_TASK);
        assert!(error.message.contains("HTTP server task failed"));
    }

    #[tokio::test]
    async fn await_background_tasks_aborts_watcher_and_http_after_unexpected_runtime_exit() {
        let poll_handle = tokio::spawn(async {});
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let http_handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        });

        let error = tokio::time::timeout(
            Duration::from_secs(2),
            await_background_tasks(
                poll_handle,
                watcher_handle,
                Some(http_handle),
                Some(BackgroundTaskKind::RuntimePollLoop),
            ),
        )
        .await
        .expect("sibling abort should prevent hangs")
        .expect_err("unexpected runtime exit should surface");

        assert_eq!(error.code, EXIT_RUNTIME_TASK);
        assert!(
            error
                .message
                .contains("runtime poll loop task exited unexpectedly")
        );
    }

    #[tokio::test]
    async fn resolve_http_snapshot_returns_timeout_without_cache() {
        let last_snapshot = RwLock::new(None);

        let envelope = resolve_http_snapshot(
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(state_snapshot(runtime_snapshot(1, 0), vec![]))
            },
            Duration::from_millis(1),
            &last_snapshot,
        )
        .await;

        assert!(envelope.snapshot.is_none());
        assert_eq!(
            envelope
                .error
                .as_ref()
                .expect("timeout error should be present")
                .code,
            SnapshotErrorCode::Timeout
        );
    }

    #[tokio::test]
    async fn resolve_http_snapshot_returns_stale_cached_snapshot_on_timeout() {
        let cached_snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Running", 0)],
        );
        let last_snapshot = RwLock::new(Some(cached_snapshot.clone()));

        let envelope = resolve_http_snapshot(
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(state_snapshot(runtime_snapshot(2, 0), vec![]))
            },
            Duration::from_millis(1),
            &last_snapshot,
        )
        .await;

        assert!(envelope.is_stale());
        assert_eq!(envelope.snapshot, Some(cached_snapshot));
        assert_eq!(
            envelope
                .error
                .as_ref()
                .expect("stale timeout error should be present")
                .code,
            SnapshotErrorCode::Timeout
        );
    }

    #[tokio::test]
    async fn resolve_http_snapshot_returns_stale_cached_snapshot_on_unavailable() {
        let cached_snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-2", "SYM-2", "Running", 0)],
        );
        let last_snapshot = RwLock::new(Some(cached_snapshot.clone()));

        let envelope = resolve_http_snapshot(
            async { Err(SnapshotErrorCode::Unavailable) },
            Duration::from_secs(1),
            &last_snapshot,
        )
        .await;

        assert!(envelope.is_stale());
        assert_eq!(envelope.snapshot, Some(cached_snapshot));
        assert_eq!(
            envelope
                .error
                .as_ref()
                .expect("stale unavailable error should be present")
                .code,
            SnapshotErrorCode::Unavailable
        );
    }

    #[tokio::test]
    async fn post_http_response_coalesces_refresh_when_signal_is_already_queued() {
        let snapshot = StateSnapshotEnvelope::ready(state_snapshot(runtime_snapshot(0, 0), vec![]));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);
        refresh_tx
            .try_send(())
            .expect("first refresh should fill the queue");

        let response = post_http_response(REFRESH_ROUTE, &snapshot, &refresh_tx);

        assert_eq!(response.status, 202);
        assert_eq!(response.body["queued"], true);
        assert_eq!(response.body["coalesced"], true);
        assert!(refresh_rx.recv().await.is_some());
    }

    #[test]
    fn enqueue_refresh_signal_reports_delivery_states() {
        let (refresh_tx, _refresh_rx) = mpsc::channel(1);
        assert_eq!(
            enqueue_refresh_signal(&refresh_tx, "test"),
            RefreshEnqueueOutcome::Delivered
        );
        assert_eq!(
            enqueue_refresh_signal(&refresh_tx, "test"),
            RefreshEnqueueOutcome::Coalesced
        );
        let (closed_tx, closed_rx) = mpsc::channel(1);
        drop(closed_rx);
        assert_eq!(
            enqueue_refresh_signal(&closed_tx, "test"),
            RefreshEnqueueOutcome::Unavailable
        );
    }

    #[tokio::test]
    async fn post_http_response_reports_unavailable_when_refresh_receiver_is_closed() {
        let snapshot = StateSnapshotEnvelope::ready(state_snapshot(runtime_snapshot(0, 0), vec![]));
        let (refresh_tx, refresh_rx) = mpsc::channel(1);
        drop(refresh_rx);

        let response = post_http_response(REFRESH_ROUTE, &snapshot, &refresh_tx);

        assert_eq!(response.status, 503);
        assert_eq!(response.body["error"]["code"], "orchestrator_unavailable");
    }

    #[tokio::test]
    async fn get_http_get_response_uses_latest_workspace_root_from_config() {
        let initial_root =
            temp_dir("get_http_get_response_uses_latest_workspace_root_from_config-initial");
        let reloaded_root =
            temp_dir("get_http_get_response_uses_latest_workspace_root_from_config-reload");
        let config = Arc::new(RwLock::new(runtime_config_with_workspace_root(
            initial_root,
        )));
        let snapshot = StateSnapshotEnvelope::ready(state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-9", "SYM-9", "Running", 0)],
        ));
        {
            let mut guard = config.write().await;
            *guard = runtime_config_with_workspace_root(reloaded_root.clone());
        }

        let route = issue_route("SYM-9");
        let response = get_http_get_response(&route, &snapshot, &config).await;

        assert_eq!(
            response.body["workspace"]["path"],
            reloaded_root.join("SYM-9").to_string_lossy().as_ref()
        );
    }

    #[test]
    fn refresh_post_does_not_require_ready_snapshot() {
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        let response = post_http_response(
            REFRESH_ROUTE,
            &StateSnapshotEnvelope::timeout(),
            &refresh_tx,
        );

        assert_eq!(response.status, 202);
        assert_eq!(response.body["queued"], true);
        assert!(refresh_rx.try_recv().is_ok());
    }

    #[test]
    fn refresh_post_returns_unavailable_when_signal_cannot_be_delivered() {
        let (refresh_tx, refresh_rx) = mpsc::channel(1);
        drop(refresh_rx);

        let response = post_http_response(
            REFRESH_ROUTE,
            &StateSnapshotEnvelope::timeout(),
            &refresh_tx,
        );

        assert_eq!(response.status, 503);
        assert_eq!(response.body["error"]["code"], "orchestrator_unavailable");
    }

    #[test]
    fn increment_runtime_config_version_tracks_successive_reload_versions() {
        let current = runtime_config_with_workspace_root(PathBuf::from("/tmp/current"));
        let next = runtime_config_with_workspace_root(PathBuf::from("/tmp/next"));

        let applied = increment_runtime_config_version(&current, next);

        assert_eq!(applied.version, current.version + 1);
        assert_eq!(applied.workspace.root, PathBuf::from("/tmp/next"));
    }

    #[test]
    fn retained_reload_message_describes_retained_last_good_workflow() {
        let message = retained_reload_message(
            Path::new("/tmp/WORKFLOW.md"),
            &WorkflowChangeStamp {
                content_hash: 11,
                content_bytes: 42,
                modified_unix_ms: Some(99),
            },
            &"parse error",
        );

        assert!(message.contains("retaining last applied workflow"));
        assert!(message.contains("/tmp/WORKFLOW.md"));
        assert!(message.contains("parse error"));
    }

    #[test]
    fn invalid_effective_config_reload_message_mentions_retained_config_and_prompt() {
        let message = invalid_effective_config_reload_message(
            Path::new("/tmp/WORKFLOW.md"),
            &WorkflowChangeStamp {
                content_hash: 1,
                content_bytes: 10,
                modified_unix_ms: Some(100),
            },
            &WorkflowChangeStamp {
                content_hash: 2,
                content_bytes: 11,
                modified_unix_ms: Some(200),
            },
            &"invalid config",
        );

        assert!(message.contains("retaining previously applied config and prompt"));
        assert!(message.contains("invalid config"));
        assert!(message.contains("previous_hash=1"));
        assert!(message.contains("current_hash=2"));
    }

    #[test]
    fn restart_required_reload_message_lists_stable_field_paths() {
        let message = restart_required_reload_message(
            Path::new("/tmp/WORKFLOW.md"),
            &WorkflowChangeStamp {
                content_hash: 1,
                content_bytes: 10,
                modified_unix_ms: Some(100),
            },
            &WorkflowChangeStamp {
                content_hash: 2,
                content_bytes: 12,
                modified_unix_ms: Some(200),
            },
            &[
                HostOwnedConfigChange::TrackerEndpoint,
                HostOwnedConfigChange::TrackerApiKey,
                HostOwnedConfigChange::TrackerProjectSlug,
                HostOwnedConfigChange::TrackerActiveStates,
                HostOwnedConfigChange::ServerPort,
                HostOwnedConfigChange::LogLevel,
            ],
        );

        assert!(message.contains("tracker.endpoint"));
        assert!(message.contains("tracker.api_key"));
        assert!(message.contains("tracker.project_slug"));
        assert!(message.contains("tracker.active_states"));
        assert!(message.contains("server.port"));
        assert!(message.contains("log_level.level"));
    }

    #[tokio::test]
    async fn apply_workflow_reload_requests_restart_when_server_port_changes() {
        let root = temp_dir("apply_workflow_reload_requests_restart_when_server_port_changes");
        let workflow_path = root.join("WORKFLOW.md");
        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
---
Prompt body.
"#,
        );

        let mut reloader = WorkflowReloader::load_from_path(&workflow_path)
            .expect("workflow should load for startup");
        let initial_config = effective_runtime_config(reloader.current(), &CliOverrides::default())
            .expect("initial config should build");
        let initial_prompt = reloader.current().document.prompt_body.clone();
        let initial_stamp = reloader.current().change_stamp.clone();
        let config = Arc::new(RwLock::new(initial_config.clone()));
        let runtime = Arc::new(Runtime::new(Arc::new(test_tracker())));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
server:
  port: 4051
---
Prompt body.
"#,
        );

        let action = apply_workflow_reload(
            &mut reloader,
            &config,
            &runtime,
            &CliOverrides::default(),
            &refresh_tx,
        )
        .await;

        match action {
            WorkflowReloadAction::RestartRequired(message) => {
                assert!(message.contains("server.port"));
            }
            other => panic!("expected restart-required action, got {other:?}"),
        }
        assert_eq!(config.read().await.server.port, initial_config.server.port);
        assert_eq!(reloader.current().document.prompt_body, initial_prompt);
        assert_eq!(reloader.current().change_stamp, initial_stamp);
        assert!(refresh_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn apply_workflow_reload_requests_restart_when_tracker_settings_change() {
        let root = temp_dir("apply_workflow_reload_requests_restart_when_tracker_settings_change");
        let workflow_path = root.join("WORKFLOW.md");
        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
---
Prompt body.
"#,
        );

        let mut reloader = WorkflowReloader::load_from_path(&workflow_path)
            .expect("workflow should load for startup");
        let initial_config = effective_runtime_config(reloader.current(), &CliOverrides::default())
            .expect("initial config should build");
        let initial_prompt = reloader.current().document.prompt_body.clone();
        let initial_stamp = reloader.current().change_stamp.clone();
        let config = Arc::new(RwLock::new(initial_config.clone()));
        let runtime = Arc::new(Runtime::new(Arc::new(test_tracker())));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://other.example/graphql
  api_key: token-two
  project_slug: OTHER
  active_states:
    - Todo
polling:
  interval_ms: 5000
---
Prompt body.
"#,
        );

        let action = apply_workflow_reload(
            &mut reloader,
            &config,
            &runtime,
            &CliOverrides::default(),
            &refresh_tx,
        )
        .await;

        match action {
            WorkflowReloadAction::RestartRequired(message) => {
                assert!(message.contains("tracker.endpoint"));
                assert!(message.contains("tracker.api_key"));
                assert!(message.contains("tracker.project_slug"));
                assert!(message.contains("tracker.active_states"));
            }
            other => panic!("expected restart-required action, got {other:?}"),
        }
        assert_eq!(
            config.read().await.tracker.endpoint,
            initial_config.tracker.endpoint
        );
        assert_eq!(reloader.current().document.prompt_body, initial_prompt);
        assert_eq!(reloader.current().change_stamp, initial_stamp);
        assert!(refresh_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn apply_workflow_reload_retains_last_good_workflow_on_parse_failure() {
        let root = temp_dir("apply_workflow_reload_retains_last_good_workflow_on_parse_failure");
        let workflow_path = root.join("WORKFLOW.md");
        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
---
Prompt body.
"#,
        );

        let mut reloader = WorkflowReloader::load_from_path(&workflow_path)
            .expect("workflow should load for startup");
        let initial_config = effective_runtime_config(reloader.current(), &CliOverrides::default())
            .expect("initial config should build");
        let initial_prompt = reloader.current().document.prompt_body.clone();
        let initial_stamp = reloader.current().change_stamp.clone();
        let config = Arc::new(RwLock::new(initial_config.clone()));
        let runtime = Arc::new(Runtime::new(Arc::new(test_tracker())));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: [
  project_slug: SYM
---
Broken prompt body.
"#,
        );

        let action = apply_workflow_reload(
            &mut reloader,
            &config,
            &runtime,
            &CliOverrides::default(),
            &refresh_tx,
        )
        .await;

        assert_eq!(action, WorkflowReloadAction::NoChange);
        assert_eq!(config.read().await.clone(), initial_config);
        assert_eq!(reloader.current().document.prompt_body, initial_prompt);
        assert_eq!(reloader.current().change_stamp, initial_stamp);
        assert!(refresh_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn apply_workflow_reload_retains_last_good_workflow_on_invalid_effective_config() {
        let root = temp_dir(
            "apply_workflow_reload_retains_last_good_workflow_on_invalid_effective_config",
        );
        let workflow_path = root.join("WORKFLOW.md");
        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
---
Prompt body.
"#,
        );

        let mut reloader = WorkflowReloader::load_from_path(&workflow_path)
            .expect("workflow should load for startup");
        let initial_config = effective_runtime_config(reloader.current(), &CliOverrides::default())
            .expect("initial config should build");
        let initial_prompt = reloader.current().document.prompt_body.clone();
        let initial_stamp = reloader.current().change_stamp.clone();
        let config = Arc::new(RwLock::new(initial_config.clone()));
        let runtime = Arc::new(Runtime::new(Arc::new(test_tracker())));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: {}
---
Broken prompt body.
"#,
        );

        let action = apply_workflow_reload(
            &mut reloader,
            &config,
            &runtime,
            &CliOverrides::default(),
            &refresh_tx,
        )
        .await;

        assert_eq!(action, WorkflowReloadAction::NoChange);
        assert_eq!(config.read().await.clone(), initial_config);
        assert_eq!(reloader.current().document.prompt_body, initial_prompt);
        assert_eq!(reloader.current().change_stamp, initial_stamp);
        assert!(refresh_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn apply_workflow_reload_applies_runtime_safe_changes_without_restart() {
        let root = temp_dir("apply_workflow_reload_applies_runtime_safe_changes_without_restart");
        let workflow_path = root.join("WORKFLOW.md");
        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 5000
agent:
  max_turns: 5
---
Prompt body.
"#,
        );

        let mut reloader = WorkflowReloader::load_from_path(&workflow_path)
            .expect("workflow should load for startup");
        let initial_config = effective_runtime_config(reloader.current(), &CliOverrides::default())
            .expect("initial config should build");
        let config = Arc::new(RwLock::new(initial_config));
        let runtime = Arc::new(Runtime::new(Arc::new(test_tracker())));
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        write_raw_workflow(
            &workflow_path,
            r#"---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: token-one
  project_slug: SYM
polling:
  interval_ms: 15000
agent:
  max_turns: 9
---
Updated prompt body.
"#,
        );

        let action = apply_workflow_reload(
            &mut reloader,
            &config,
            &runtime,
            &CliOverrides::default(),
            &refresh_tx,
        )
        .await;

        assert_eq!(action, WorkflowReloadAction::Applied);
        let applied = config.read().await.clone();
        assert_eq!(applied.polling.interval_ms, 15_000);
        assert_eq!(applied.agent.max_turns, 9);
        assert_eq!(applied.version, 1);
        assert_eq!(
            reloader.current().document.prompt_body,
            "Updated prompt body."
        );
        assert!(refresh_rx.try_recv().is_ok());
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

    fn write_raw_workflow(workflow_path: &Path, contents: &str) {
        fs::write(workflow_path, contents).expect("workflow file should be writable");
    }

    fn test_tracker() -> LinearTracker {
        LinearTracker::new(
            "https://api.linear.app/graphql".to_owned(),
            "token".to_owned(),
            "SYM".to_owned(),
        )
        .with_candidate_states(vec![
            TrackerState::new("Todo"),
            TrackerState::new("In Progress"),
        ])
    }

    fn runtime_config_with_workspace_root(workspace_root: PathBuf) -> RuntimeConfig {
        RuntimeConfig {
            workspace: symphony_config::WorkspaceConfig {
                root: workspace_root,
            },
            ..RuntimeConfig::default()
        }
    }
}
