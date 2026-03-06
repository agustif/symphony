use std::borrow::Cow;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value as JsonValue;
use symphony_agent_protocol::{
    LineOrigin, ProtocolMethodKind, ProtocolPolicyOutcome, StreamLineParser, SupportedToolSpec,
    build_initialize_request, build_initialized_notification, build_session_id,
    build_thread_start_request, build_turn_start_request, classify_policy_outcome,
    decode_stdout_line, extract_rate_limits, extract_thread_id, extract_tool_call_id,
    extract_tool_name, extract_turn_id, extract_usage,
};
use symphony_config::{CodexConfig, TrackerConfig};
use symphony_domain::{IssueId, Usage};
use symphony_tracker::{TrackerClient, TrackerIssue, TrackerState};
use symphony_workspace::{
    HookExecutor, HookRequest, HookResult, PreparedWorkspace, WorkspaceError, WorkspaceHooks,
    prepare_workspace_with_hooks, run_after_run_hook, run_before_run_hook, workspace_path,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, watch};

/// Context for spawning a worker with full workspace and prompt information
#[derive(Clone)]
pub struct WorkerContext {
    pub issue: TrackerIssue,
    pub attempt: u32,
    pub root: PathBuf,
    pub hooks: WorkspaceHooks,
    pub tracker_config: TrackerConfig,
    pub tracker_client: Arc<dyn TrackerClient>,
    pub active_states: Vec<TrackerState>,
    pub codex_config: CodexConfig,
    pub max_turns: u32,
    pub prompt_template: String,
    pub protocol_updates: Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    pub stop_rx: watch::Receiver<bool>,
}

impl WorkerContext {
    /// Create a new WorkerContext ensuring workspace path safety
    pub fn new(
        issue: TrackerIssue,
        attempt: u32,
        execution: WorkerExecutionConfig,
        runtime: WorkerRuntimeHandles,
    ) -> Result<Self, WorkspaceError> {
        // Validate workspace path is valid and within root
        let _ = workspace_path(&execution.root, &issue.identifier)?;

        Ok(Self {
            issue,
            attempt,
            root: execution.root,
            hooks: execution.hooks,
            tracker_config: execution.tracker_config,
            tracker_client: runtime.tracker_client,
            active_states: runtime.active_states,
            codex_config: execution.codex_config,
            max_turns: runtime.max_turns,
            prompt_template: execution.prompt_template,
            protocol_updates: runtime.protocol_updates,
            stop_rx: runtime.stop_rx,
        })
    }

    /// Get the issue ID for this worker context
    pub fn issue_id(&self) -> IssueId {
        self.issue.id.clone()
    }

    /// Get the workspace path for this issue
    pub fn workspace_path(&self) -> PathBuf {
        workspace_path(&self.root, &self.issue.identifier)
            .expect("workspace path should be valid after construction")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("prompt rendering failed")]
    PromptError,
    #[error("agent failure")]
    AgentFailure,
    #[error("worker launch failed: {0}")]
    LaunchFailed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerOutcome {
    Completed,
    Continuation,
    RetryableFailure,
    PermanentFailure,
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct WorkerProtocolUpdate {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub pid: Option<u32>,
    pub event: Option<String>,
    pub timestamp: Option<u64>,
    pub message: Option<String>,
    pub usage: Option<Usage>,
    pub rate_limits: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct WorkerExecutionConfig {
    pub root: PathBuf,
    pub hooks: WorkspaceHooks,
    pub tracker_config: TrackerConfig,
    pub codex_config: CodexConfig,
    pub prompt_template: String,
}

#[derive(Clone)]
pub struct WorkerRuntimeHandles {
    pub tracker_client: Arc<dyn TrackerClient>,
    pub active_states: Vec<TrackerState>,
    pub max_turns: u32,
    pub protocol_updates: Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    pub stop_rx: watch::Receiver<bool>,
}

#[derive(Clone, Debug)]
struct LaunchOptions<'a> {
    turn_title: &'a str,
    turn_timeout_ms: u64,
    read_timeout_ms: u64,
    approval_policy: Option<JsonValue>,
    thread_sandbox: Option<&'a str>,
    turn_sandbox_policy: Option<JsonValue>,
    tracker_kind: &'a str,
    tracker_endpoint: &'a str,
    tracker_api_key: Option<&'a str>,
}

#[derive(Clone, Debug)]
struct ToolCallExecutionContext {
    tracker_kind: String,
    tracker_endpoint: String,
    tracker_api_key: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct SessionMetadata {
    pid: Option<u32>,
    thread_id: Option<String>,
    turn_id: Option<String>,
}

impl<'a> From<&LaunchOptions<'a>> for ToolCallExecutionContext {
    fn from(options: &LaunchOptions<'a>) -> Self {
        Self {
            tracker_kind: options.tracker_kind.to_owned(),
            tracker_endpoint: options.tracker_endpoint.to_owned(),
            tracker_api_key: options.tracker_api_key.map(ToOwned::to_owned),
        }
    }
}

#[async_trait::async_trait]
pub trait WorkerLauncher: Send + Sync {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ShellWorkerLauncher;

#[async_trait::async_trait]
impl WorkerLauncher for ShellWorkerLauncher {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
        let hook_executor = ShellHookExecutor;
        let workspace = prepare_workspace_with_hooks(
            &ctx.root,
            &ctx.issue.identifier,
            &ctx.hooks,
            &hook_executor,
        )?;
        run_before_run_hook(&workspace, &ctx.hooks, &hook_executor)?;

        let launch_result = async {
            let prompt = render_prompt(&ctx)?;
            let command = ctx.codex_config.command.trim().to_owned();
            if command.is_empty() {
                return Err(WorkerError::LaunchFailed("empty codex command".to_owned()));
            }

            let turn_title = format!("{}: {}", ctx.issue.identifier, ctx.issue.title);
            let launch_options = LaunchOptions {
                turn_title: &turn_title,
                turn_timeout_ms: ctx.codex_config.turn_timeout_ms,
                read_timeout_ms: ctx.codex_config.read_timeout_ms,
                approval_policy: ctx.codex_config.approval_policy.clone(),
                thread_sandbox: ctx.codex_config.thread_sandbox.as_deref(),
                turn_sandbox_policy: ctx.codex_config.turn_sandbox_policy.clone(),
                tracker_kind: &ctx.tracker_config.kind,
                tracker_endpoint: &ctx.tracker_config.endpoint,
                tracker_api_key: ctx.tracker_config.api_key.as_deref(),
            };
            launch_agent_command(&command, &workspace.path, &prompt, launch_options, &ctx).await
        }
        .await;
        run_after_run_hook_best_effort(&workspace, &ctx.hooks, &hook_executor, &ctx.issue.id);

        launch_result
    }
}

/// Prepare workspace for a worker with hooks
/// Ensures workspace path safety and executes before-run hooks
pub fn prepare_worker_workspace(
    ctx: &WorkerContext,
    executor: &impl HookExecutor,
) -> Result<PreparedWorkspace, WorkspaceError> {
    symphony_workspace::prepare_workspace_with_hooks(
        &ctx.root,
        &ctx.issue.identifier,
        &ctx.hooks,
        executor,
    )
}

fn run_after_run_hook_best_effort(
    workspace: &PreparedWorkspace,
    hooks: &WorkspaceHooks,
    executor: &impl HookExecutor,
    issue_id: &IssueId,
) {
    if let Err(error) = run_after_run_hook(workspace, hooks, executor) {
        tracing::warn!(
            issue_id = %issue_id.0,
            workspace_path = %workspace.path.display(),
            error = %error,
            "after_run hook failed and was ignored"
        );
    }
}

/// Render prompt template with issue context using strict placeholder resolution.
pub fn render_prompt(ctx: &WorkerContext) -> Result<String, WorkerError> {
    let mut rendered = String::with_capacity(ctx.prompt_template.len());
    let mut cursor = 0;
    let template = ctx.prompt_template.as_str();

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        rendered.push_str(&template[cursor..start]);

        let placeholder_start = start + 2;
        let Some(end_offset) = template[placeholder_start..].find("}}") else {
            tracing::warn!(
                issue_id = %ctx.issue.id.0,
                "prompt rendering failed: unclosed placeholder"
            );
            return Err(WorkerError::PromptError);
        };

        let placeholder_end = placeholder_start + end_offset;
        let placeholder = template[placeholder_start..placeholder_end].trim();
        let Some(value) = render_prompt_placeholder(ctx, placeholder) else {
            tracing::warn!(
                issue_id = %ctx.issue.id.0,
                placeholder,
                "prompt rendering failed: unsupported placeholder"
            );
            return Err(WorkerError::PromptError);
        };

        rendered.push_str(value.as_ref());
        cursor = placeholder_end + 2;
    }

    if template[cursor..].contains("}}") {
        tracing::warn!(
            issue_id = %ctx.issue.id.0,
            "prompt rendering failed: unmatched closing delimiter"
        );
        return Err(WorkerError::PromptError);
    }

    rendered.push_str(&template[cursor..]);
    Ok(rendered)
}

fn render_prompt_placeholder<'a>(
    ctx: &'a WorkerContext,
    placeholder: &str,
) -> Option<Cow<'a, str>> {
    match placeholder {
        "issue.id" | "issue_id" => Some(Cow::Borrowed(&ctx.issue.id.0)),
        "issue.identifier" | "issue_identifier" => Some(Cow::Borrowed(&ctx.issue.identifier)),
        "issue.title" | "issue_title" => Some(Cow::Borrowed(&ctx.issue.title)),
        "issue.description" | "issue_description" => Some(Cow::Borrowed(
            ctx.issue.description.as_deref().unwrap_or(""),
        )),
        "attempt" => Some(Cow::Owned(ctx.attempt.to_string())),
        _ => None,
    }
}

#[derive(Clone)]
struct LaunchSessionContext {
    issue: TrackerIssue,
    tracker_client: Arc<dyn TrackerClient>,
    active_states: Vec<TrackerState>,
    max_turns: u32,
    protocol_updates: Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    stop_rx: watch::Receiver<bool>,
}

impl From<&WorkerContext> for LaunchSessionContext {
    fn from(ctx: &WorkerContext) -> Self {
        Self {
            issue: ctx.issue.clone(),
            tracker_client: Arc::clone(&ctx.tracker_client),
            active_states: ctx.active_states.clone(),
            max_turns: ctx.max_turns.max(1),
            protocol_updates: ctx.protocol_updates.clone(),
            stop_rx: ctx.stop_rx.clone(),
        }
    }
}

struct AppServerLaunchContext<'a> {
    cwd: &'a std::path::Path,
    first_prompt: &'a str,
    options: LaunchOptions<'a>,
    tool_context: ToolCallExecutionContext,
    launch_ctx: LaunchSessionContext,
}

struct TurnStartContext<'a> {
    thread_id: &'a str,
    prompt: &'a str,
    cwd: &'a std::path::Path,
    options: LaunchOptions<'a>,
    turn_number: u32,
    read_timeout: Duration,
}

async fn launch_agent_command(
    command: &str,
    cwd: &std::path::Path,
    prompt: &str,
    options: LaunchOptions<'_>,
    ctx: &WorkerContext,
) -> Result<WorkerOutcome, WorkerError> {
    let mut child = shell_command_async(command);
    child.current_dir(cwd);
    child.stdin(Stdio::piped());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::piped());

    let mut child = child
        .spawn()
        .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| WorkerError::LaunchFailed("missing child stdin pipe".to_owned()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WorkerError::LaunchFailed("missing child stdout pipe".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkerError::LaunchFailed("missing child stderr pipe".to_owned()))?;

    let tool_context = ToolCallExecutionContext::from(&options);
    let launch_ctx = LaunchSessionContext::from(ctx);
    let app_server_mode = command_looks_like_app_server(command);
    if app_server_mode {
        return launch_app_server_command(
            child,
            stdin,
            stdout,
            stderr,
            AppServerLaunchContext {
                cwd,
                first_prompt: prompt,
                options,
                tool_context,
                launch_ctx,
            },
        )
        .await;
    }

    emit_protocol_update(
        &launch_ctx.protocol_updates,
        WorkerProtocolUpdate {
            pid: child.id(),
            event: Some("worker/started".to_owned()),
            timestamp: Some(current_unix_timestamp_secs()),
            ..WorkerProtocolUpdate::default()
        },
    );

    stdin
        .write_all(prompt.as_bytes())
        .await
        .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
    drop(stdin);

    let child_pid = child.id();
    let mut protocol_watch = tokio::spawn(async move {
        monitor_protocol_streams(
            BufReader::new(stdout),
            stderr,
            None::<tokio::process::ChildStdin>,
            tool_context,
            launch_ctx.protocol_updates,
            SessionMetadata {
                pid: child_pid,
                ..SessionMetadata::default()
            },
        )
        .await
    });

    let timeout = Duration::from_millis(options.turn_timeout_ms.max(1));
    let deadline = tokio::time::Instant::now() + timeout;
    let mut stop_rx = launch_ctx.stop_rx;
    let mut protocol_watch_done = false;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            protocol_watch.abort();
            let _ = protocol_watch.await;
            return Ok(WorkerOutcome::RetryableFailure);
        }

        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    protocol_watch.abort();
                    let _ = protocol_watch.await;
                    return Ok(WorkerOutcome::Stopped);
                }
            }
            status = child.wait() => {
                protocol_watch.abort();
                let _ = protocol_watch.await;
                let status = status.map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
                return map_status_to_outcome(status);
            }
            watch = &mut protocol_watch, if !protocol_watch_done => {
                match watch {
                    Ok(WorkerProtocolSignal::Continue) => {
                        protocol_watch_done = true;
                    }
                    Ok(WorkerProtocolSignal::RetryableFailure) => {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        return Ok(WorkerOutcome::RetryableFailure);
                    }
                    Ok(WorkerProtocolSignal::PermanentFailure) => {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        return Ok(WorkerOutcome::PermanentFailure);
                    }
                    Err(error) => {
                        return Err(WorkerError::LaunchFailed(format!("protocol monitor failed: {error}")));
                    }
                }
            }
            _ = tokio::time::sleep(remaining) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                protocol_watch.abort();
                let _ = protocol_watch.await;
                return Ok(WorkerOutcome::RetryableFailure);
            }
        }
    }
}

enum StartupIdentifierOutcome {
    Identifier(String),
    WorkerOutcome(WorkerOutcome),
}

async fn launch_app_server_command(
    mut child: tokio::process::Child,
    mut stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    mut stderr: tokio::process::ChildStderr,
    mut app_server: AppServerLaunchContext<'_>,
) -> Result<WorkerOutcome, WorkerError> {
    let read_timeout = Duration::from_millis(app_server.options.read_timeout_ms.max(1));
    let turn_timeout = Duration::from_millis(app_server.options.turn_timeout_ms.max(1));
    let mut session = SessionMetadata {
        pid: child.id(),
        ..SessionMetadata::default()
    };
    emit_protocol_update(
        &app_server.launch_ctx.protocol_updates,
        WorkerProtocolUpdate {
            pid: session.pid,
            event: Some("worker/started".to_owned()),
            timestamp: Some(current_unix_timestamp_secs()),
            ..WorkerProtocolUpdate::default()
        },
    );

    let (mut reader, thread_id) = match perform_session_handshake(
        &mut stdin,
        stdout,
        app_server.cwd,
        read_timeout,
        app_server.options.clone(),
        &mut app_server.launch_ctx.stop_rx,
    )
    .await?
    {
        SessionHandshakeOutcome::Ready(reader, thread_id) => (reader, thread_id),
        SessionHandshakeOutcome::WorkerOutcome(outcome) => {
            stop_child_process(&mut child).await;
            return Ok(outcome);
        }
    };
    session.thread_id = Some(thread_id.clone());
    emit_protocol_update(
        &app_server.launch_ctx.protocol_updates,
        WorkerProtocolUpdate {
            session_id: build_session_id(Some(&thread_id), None),
            thread_id: Some(thread_id.clone()),
            pid: session.pid,
            event: Some("thread/start".to_owned()),
            timestamp: Some(current_unix_timestamp_secs()),
            ..WorkerProtocolUpdate::default()
        },
    );

    let mut parser = StreamLineParser::default();
    for turn_number in 1..=app_server.launch_ctx.max_turns {
        let prompt = if turn_number == 1 {
            app_server.first_prompt.to_owned()
        } else {
            continuation_prompt(turn_number, app_server.launch_ctx.max_turns)
        };

        let turn_id = match start_turn_handshake(
            &mut stdin,
            &mut reader,
            TurnStartContext {
                thread_id: &thread_id,
                prompt: &prompt,
                cwd: app_server.cwd,
                options: app_server.options.clone(),
                turn_number,
                read_timeout,
            },
            &mut app_server.launch_ctx.stop_rx,
        )
        .await?
        {
            StartupIdentifierOutcome::Identifier(turn_id) => turn_id,
            StartupIdentifierOutcome::WorkerOutcome(outcome) => {
                stop_child_process(&mut child).await;
                return Ok(outcome);
            }
        };

        session.turn_id = Some(turn_id.clone());
        emit_protocol_update(
            &app_server.launch_ctx.protocol_updates,
            WorkerProtocolUpdate {
                session_id: build_session_id(Some(&thread_id), Some(&turn_id)),
                thread_id: Some(thread_id.clone()),
                turn_id: Some(turn_id.clone()),
                pid: session.pid,
                event: Some("turn/start".to_owned()),
                timestamp: Some(current_unix_timestamp_secs()),
                message: Some(format!("turn {turn_number} started")),
                ..WorkerProtocolUpdate::default()
            },
        );

        let signal = monitor_app_server_turn(
            &mut child,
            &mut reader,
            &mut stderr,
            TurnMonitorContext {
                stdin: &mut stdin,
                parser: &mut parser,
                tool_context: &app_server.tool_context,
                protocol_updates: &app_server.launch_ctx.protocol_updates,
                session: &mut session,
                stop_rx: &mut app_server.launch_ctx.stop_rx,
                timeout: turn_timeout,
            },
        )
        .await?;

        if *app_server.launch_ctx.stop_rx.borrow() {
            return Ok(WorkerOutcome::Stopped);
        }

        match signal {
            WorkerProtocolSignal::Continue => {
                if turn_number < app_server.launch_ctx.max_turns
                    && issue_remains_active(&app_server.launch_ctx).await?
                {
                    continue;
                }
                stop_child_process(&mut child).await;
                return Ok(WorkerOutcome::Completed);
            }
            WorkerProtocolSignal::RetryableFailure => {
                stop_child_process(&mut child).await;
                return Ok(WorkerOutcome::RetryableFailure);
            }
            WorkerProtocolSignal::PermanentFailure => {
                stop_child_process(&mut child).await;
                return Ok(WorkerOutcome::PermanentFailure);
            }
        }
    }

    stop_child_process(&mut child).await;
    Ok(WorkerOutcome::Completed)
}

enum SessionHandshakeOutcome {
    Ready(BufReader<tokio::process::ChildStdout>, String),
    WorkerOutcome(WorkerOutcome),
}

async fn perform_session_handshake(
    stdin: &mut tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    cwd: &std::path::Path,
    read_timeout: Duration,
    options: LaunchOptions<'_>,
    stop_rx: &mut watch::Receiver<bool>,
) -> Result<SessionHandshakeOutcome, WorkerError> {
    let supported_tools = supported_tools_for_options(&options);
    let initialize = build_initialize_request(
        serde_json::json!(1),
        "symphony-rust",
        env!("CARGO_PKG_VERSION"),
        supported_tools.as_slice(),
    );
    write_protocol_message(stdin, &initialize).await?;

    let initialized = build_initialized_notification();
    write_protocol_message(stdin, &initialized).await?;

    let thread_start = build_thread_start_request(
        serde_json::json!(2),
        cwd,
        options.approval_policy.as_ref(),
        options.thread_sandbox,
    );
    write_protocol_message(stdin, &thread_start).await?;

    let mut reader = BufReader::new(stdout);
    let thread_id =
        match wait_for_startup_identifier(&mut reader, read_timeout, extract_thread_id, stop_rx)
            .await?
        {
            StartupIdentifierOutcome::Identifier(thread_id) => thread_id,
            StartupIdentifierOutcome::WorkerOutcome(outcome) => {
                return Ok(SessionHandshakeOutcome::WorkerOutcome(outcome));
            }
        };

    Ok(SessionHandshakeOutcome::Ready(reader, thread_id))
}

async fn start_turn_handshake(
    stdin: &mut tokio::process::ChildStdin,
    reader: &mut BufReader<tokio::process::ChildStdout>,
    turn: TurnStartContext<'_>,
    stop_rx: &mut watch::Receiver<bool>,
) -> Result<StartupIdentifierOutcome, WorkerError> {
    let turn_start = build_turn_start_request(
        serde_json::json!(2_u64.saturating_add(turn.turn_number as u64)),
        turn.thread_id,
        turn.prompt,
        turn.cwd,
        turn.options.turn_title,
        turn.options.approval_policy.as_ref(),
        turn.options.turn_sandbox_policy.as_ref(),
    );
    write_protocol_message(stdin, &turn_start).await?;
    wait_for_startup_identifier(reader, turn.read_timeout, extract_turn_id, stop_rx).await
}

async fn wait_for_startup_identifier(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    read_timeout: Duration,
    extractor: fn(&symphony_agent_protocol::AppServerEvent) -> Option<String>,
    stop_rx: &mut watch::Receiver<bool>,
) -> Result<StartupIdentifierOutcome, WorkerError> {
    let deadline = tokio::time::Instant::now() + read_timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(StartupIdentifierOutcome::WorkerOutcome(
                WorkerOutcome::RetryableFailure,
            ));
        }

        let mut line = String::new();
        let bytes_read = tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    return Ok(StartupIdentifierOutcome::WorkerOutcome(WorkerOutcome::Stopped));
                }
                continue;
            }
            read_result = tokio::time::timeout(remaining, reader.read_line(&mut line)) => {
                match read_result {
                    Ok(Ok(bytes_read)) => bytes_read,
                    Ok(Err(error)) => {
                        return Err(WorkerError::LaunchFailed(format!(
                            "startup handshake read failed: {error}"
                        )));
                    }
                    Err(_) => {
                        return Ok(StartupIdentifierOutcome::WorkerOutcome(
                            WorkerOutcome::RetryableFailure,
                        ));
                    }
                }
            }
        };

        if bytes_read == 0 {
            return Ok(StartupIdentifierOutcome::WorkerOutcome(
                WorkerOutcome::RetryableFailure,
            ));
        }

        let event = match decode_stdout_line(&line) {
            Ok(event) => event,
            Err(_) => continue,
        };

        if let Some(policy_outcome) = classify_policy_outcome(&event) {
            return Ok(StartupIdentifierOutcome::WorkerOutcome(
                map_policy_outcome_to_worker_outcome(policy_outcome),
            ));
        }

        if let Some(identifier) = extractor(&event) {
            return Ok(StartupIdentifierOutcome::Identifier(identifier));
        }
    }
}

struct TurnMonitorContext<'a> {
    stdin: &'a mut tokio::process::ChildStdin,
    parser: &'a mut StreamLineParser,
    tool_context: &'a ToolCallExecutionContext,
    protocol_updates: &'a Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    session: &'a mut SessionMetadata,
    stop_rx: &'a mut watch::Receiver<bool>,
    timeout: Duration,
}

async fn monitor_app_server_turn(
    child: &mut tokio::process::Child,
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    stderr: &mut tokio::process::ChildStderr,
    monitor: TurnMonitorContext<'_>,
) -> Result<WorkerProtocolSignal, WorkerError> {
    let TurnMonitorContext {
        stdin,
        parser,
        tool_context,
        protocol_updates,
        session,
        stop_rx,
        timeout,
    } = monitor;
    let mut stdout_buffer = vec![0_u8; 2048];
    let mut stderr_buffer = vec![0_u8; 2048];
    let deadline = tokio::time::Instant::now() + timeout;
    let mut stdout_open = true;
    let mut stderr_open = true;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(WorkerProtocolSignal::RetryableFailure);
        }

        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    stop_child_process(child).await;
                    return Ok(WorkerProtocolSignal::Continue);
                }
            }
            status = child.wait() => {
                let status = status.map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
                return Ok(match map_status_to_outcome(status)? {
                    WorkerOutcome::RetryableFailure => WorkerProtocolSignal::RetryableFailure,
                    WorkerOutcome::PermanentFailure => WorkerProtocolSignal::PermanentFailure,
                    WorkerOutcome::Completed | WorkerOutcome::Continuation | WorkerOutcome::Stopped => WorkerProtocolSignal::Continue,
                });
            }
            read = stdout.read(&mut stdout_buffer), if stdout_open => {
                match read {
                    Ok(0) => stdout_open = false,
                    Ok(read_bytes) => {
                        let chunk = String::from_utf8_lossy(&stdout_buffer[..read_bytes]);
                        if let Some(signal) = protocol_chunk_signal(
                            parser,
                            LineOrigin::Stdout,
                            &chunk,
                            Some(stdin),
                            tool_context,
                            protocol_updates,
                            session,
                        ).await {
                            return Ok(signal);
                        }
                    }
                    Err(error) => {
                        return Err(WorkerError::LaunchFailed(format!("protocol stdout read failed: {error}")));
                    }
                }
            }
            read = stderr.read(&mut stderr_buffer), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(read_bytes) => {
                        let chunk = String::from_utf8_lossy(&stderr_buffer[..read_bytes]);
                        let _ = protocol_chunk_signal(
                            parser,
                            LineOrigin::Stderr,
                            &chunk,
                            None::<&mut tokio::process::ChildStdin>,
                            tool_context,
                            protocol_updates,
                            session,
                        ).await;
                    }
                    Err(error) => {
                        return Err(WorkerError::LaunchFailed(format!("protocol stderr read failed: {error}")));
                    }
                }
            }
        }
    }
}

async fn issue_remains_active(ctx: &LaunchSessionContext) -> Result<bool, WorkerError> {
    let states = ctx
        .tracker_client
        .fetch_states_by_ids(std::slice::from_ref(&ctx.issue.id))
        .await
        .map_err(|error| {
            WorkerError::LaunchFailed(format!("issue state refresh failed: {error}"))
        })?;

    Ok(states
        .get(&ctx.issue.id)
        .is_some_and(|state| tracker_state_matches_any(state, &ctx.active_states)))
}

fn tracker_state_matches_any(state: &TrackerState, active_states: &[TrackerState]) -> bool {
    if active_states.is_empty() {
        return false;
    }

    let Some(target) = state.normalized_key() else {
        return false;
    };
    active_states
        .iter()
        .filter_map(TrackerState::normalized_key)
        .any(|active| active == target)
}

fn continuation_prompt(turn_number: u32, max_turns: u32) -> String {
    format!(
        "Continuation guidance:\n\n- The previous Codex turn completed normally, but the issue is still active.\n- This is continuation turn {turn_number} of {max_turns} for the current agent run.\n- Resume from the current workspace and thread context instead of restarting.\n- Focus only on the remaining ticket work."
    )
}

async fn stop_child_process(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn emit_protocol_update(
    sender: &Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    update: WorkerProtocolUpdate,
) {
    if let Some(sender) = sender {
        let _ = sender.send(update);
    }
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

async fn write_protocol_message(
    stdin: &mut (impl AsyncWrite + Unpin),
    payload: &serde_json::Value,
) -> Result<(), WorkerError> {
    let encoded = serde_json::to_vec(payload)
        .map_err(|error| WorkerError::LaunchFailed(format!("protocol encode failed: {error}")))?;
    for chunk in [encoded.as_slice(), b"\n"] {
        if let Err(error) = stdin.write_all(chunk).await {
            if is_closed_pipe_error(&error) {
                return Ok(());
            }
            return Err(WorkerError::LaunchFailed(format!(
                "protocol write failed: {error}"
            )));
        }
    }
    if let Err(error) = stdin.flush().await {
        if is_closed_pipe_error(&error) {
            return Ok(());
        }
        return Err(WorkerError::LaunchFailed(format!(
            "protocol flush failed: {error}"
        )));
    }
    Ok(())
}

async fn write_protocol_message_best_effort(
    stdin: &mut (impl AsyncWrite + Unpin),
    payload: &serde_json::Value,
) -> Result<(), WorkerError> {
    let encoded = serde_json::to_vec(payload)
        .map_err(|error| WorkerError::LaunchFailed(format!("protocol encode failed: {error}")))?;

    for chunk in [encoded.as_slice(), b"\n"] {
        if let Err(error) = stdin.write_all(chunk).await {
            if is_closed_pipe_error(&error) {
                return Ok(());
            }
            return Err(WorkerError::LaunchFailed(format!(
                "protocol write failed: {error}"
            )));
        }
    }

    if let Err(error) = stdin.flush().await {
        if is_closed_pipe_error(&error) {
            return Ok(());
        }
        return Err(WorkerError::LaunchFailed(format!(
            "protocol flush failed: {error}"
        )));
    }

    Ok(())
}

fn is_closed_pipe_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
    ) || error.raw_os_error() == Some(32)
}

fn supported_tools_for_options(options: &LaunchOptions<'_>) -> Vec<SupportedToolSpec> {
    if options.tracker_kind.trim().eq_ignore_ascii_case("linear")
        && options
            .tracker_api_key
            .is_some_and(|api_key| !api_key.trim().is_empty())
    {
        return vec![SupportedToolSpec::new("linear_graphql")];
    }

    Vec::new()
}

fn command_looks_like_app_server(command: &str) -> bool {
    command.to_ascii_lowercase().contains("app-server")
}

fn map_status_to_outcome(status: std::process::ExitStatus) -> Result<WorkerOutcome, WorkerError> {
    if status.success() {
        return Ok(WorkerOutcome::Completed);
    }

    if status.code() == Some(75) {
        return Ok(WorkerOutcome::Continuation);
    }

    Ok(WorkerOutcome::RetryableFailure)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerProtocolSignal {
    Continue,
    RetryableFailure,
    PermanentFailure,
}

async fn monitor_protocol_streams<W>(
    mut stdout: impl AsyncRead + Unpin,
    mut stderr: impl AsyncRead + Unpin,
    mut stdin: Option<W>,
    tool_context: ToolCallExecutionContext,
    protocol_updates: Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    mut session: SessionMetadata,
) -> WorkerProtocolSignal
where
    W: AsyncWrite + Unpin,
{
    let mut stdout_buffer = vec![0_u8; 2048];
    let mut stderr_buffer = vec![0_u8; 2048];
    let mut parser = StreamLineParser::default();
    let mut stdout_open = true;
    let mut stderr_open = true;

    loop {
        if !stdout_open && !stderr_open {
            return WorkerProtocolSignal::Continue;
        }

        tokio::select! {
            read = stdout.read(&mut stdout_buffer), if stdout_open => {
                match read {
                    Ok(0) => stdout_open = false,
                    Ok(read_bytes) => {
                        let chunk = String::from_utf8_lossy(&stdout_buffer[..read_bytes]);
                        if let Some(signal) = protocol_chunk_signal(
                            &mut parser,
                            LineOrigin::Stdout,
                            &chunk,
                            stdin.as_mut(),
                            &tool_context,
                            &protocol_updates,
                            &mut session,
                        )
                        .await {
                            return signal;
                        }
                    }
                    Err(_) => stdout_open = false,
                }
            }
            read = stderr.read(&mut stderr_buffer), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(read_bytes) => {
                        let chunk = String::from_utf8_lossy(&stderr_buffer[..read_bytes]);
                        if let Some(signal) =
                            protocol_chunk_signal(
                                &mut parser,
                                LineOrigin::Stderr,
                                &chunk,
                                None::<&mut W>,
                                &tool_context,
                                &protocol_updates,
                                &mut session,
                            )
                                .await
                        {
                            return signal;
                        }
                    }
                    Err(_) => stderr_open = false,
                }
            }
        }
    }
}

async fn protocol_chunk_signal<W>(
    parser: &mut StreamLineParser,
    origin: LineOrigin,
    chunk: &str,
    mut stdin: Option<&mut W>,
    tool_context: &ToolCallExecutionContext,
    protocol_updates: &Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    session: &mut SessionMetadata,
) -> Option<WorkerProtocolSignal>
where
    W: AsyncWrite + Unpin,
{
    for parsed in parser.push_chunk(origin, chunk) {
        let Ok(parsed_line) = parsed else {
            continue;
        };
        let Some(event) = parsed_line.stdout_event() else {
            continue;
        };
        emit_protocol_event_update(protocol_updates, session, event);
        if matches!(event.method_kind(), ProtocolMethodKind::ToolCall) {
            if let Some(writer) = stdin.as_mut()
                && handle_tool_call_event(event, &mut **writer, tool_context)
                    .await
                    .is_err()
            {
                return Some(WorkerProtocolSignal::RetryableFailure);
            }
            continue;
        }
        if let Some(policy_outcome) = classify_policy_outcome(event) {
            return Some(match map_policy_outcome_to_worker_outcome(policy_outcome) {
                WorkerOutcome::RetryableFailure => WorkerProtocolSignal::RetryableFailure,
                WorkerOutcome::PermanentFailure => WorkerProtocolSignal::PermanentFailure,
                WorkerOutcome::Completed | WorkerOutcome::Continuation | WorkerOutcome::Stopped => {
                    WorkerProtocolSignal::Continue
                }
            });
        }
        if matches!(
            event.method_kind(),
            ProtocolMethodKind::TurnCompleted | ProtocolMethodKind::TurnEnded
        ) {
            return Some(WorkerProtocolSignal::Continue);
        }
    }
    None
}

fn emit_protocol_event_update(
    sender: &Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
    session: &mut SessionMetadata,
    event: &symphony_agent_protocol::AppServerEvent,
) {
    if let Some(thread_id) = extract_thread_id(event) {
        session.thread_id = Some(thread_id);
    }
    if let Some(turn_id) = extract_turn_id(event) {
        session.turn_id = Some(turn_id);
    }

    emit_protocol_update(
        sender,
        WorkerProtocolUpdate {
            session_id: build_session_id(session.thread_id.as_deref(), session.turn_id.as_deref()),
            thread_id: session.thread_id.clone(),
            turn_id: session.turn_id.clone(),
            pid: session.pid,
            event: (!event.method.trim().is_empty()).then(|| event.method.clone()),
            timestamp: Some(current_unix_timestamp_secs()),
            message: extract_protocol_message(event),
            usage: extract_usage(event).map(|usage| Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
            rate_limits: extract_rate_limits(event),
        },
    );
}

fn extract_protocol_message(event: &symphony_agent_protocol::AppServerEvent) -> Option<String> {
    let payloads = [
        event.result.as_ref(),
        Some(&event.params),
        event.error.as_ref(),
    ];
    for payload in payloads.into_iter().flatten() {
        for key in ["message", "reason", "code"] {
            if let Some(value) = payload.get(key).and_then(serde_json::Value::as_str)
                && !value.trim().is_empty()
            {
                return Some(value.to_owned());
            }
        }
    }
    None
}

async fn handle_tool_call_event(
    event: &symphony_agent_protocol::AppServerEvent,
    stdin: &mut (impl AsyncWrite + Unpin),
    tool_context: &ToolCallExecutionContext,
) -> Result<(), WorkerError> {
    let Some(call_id) = extract_tool_call_id(event) else {
        return Ok(());
    };
    let result = tool_call_result(event, tool_context).await;
    let response = serde_json::json!({ "id": call_id, "result": result });
    write_protocol_message_best_effort(stdin, &response).await
}

async fn tool_call_result(
    event: &symphony_agent_protocol::AppServerEvent,
    tool_context: &ToolCallExecutionContext,
) -> serde_json::Value {
    let Some(tool_name) = extract_tool_name(event) else {
        return serde_json::json!({
            "success": false,
            "error": "unsupported_tool_call",
            "output": {"message": "missing tool name"},
        });
    };

    if !tool_name.eq_ignore_ascii_case("linear_graphql") {
        return serde_json::json!({
            "success": false,
            "error": "unsupported_tool_call",
            "output": {"tool_name": tool_name},
        });
    }

    execute_linear_graphql_tool(event, tool_context).await
}

async fn execute_linear_graphql_tool(
    event: &symphony_agent_protocol::AppServerEvent,
    tool_context: &ToolCallExecutionContext,
) -> serde_json::Value {
    const LINEAR_TOOL_TIMEOUT_SECS: u64 = 30;

    if !tool_context.tracker_kind.eq_ignore_ascii_case("linear") {
        return serde_json::json!({
            "success": false,
            "error": "linear_tracker_not_configured",
            "output": {"tracker_kind": tool_context.tracker_kind},
        });
    }

    let Some(api_key) = tool_context
        .tracker_api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return serde_json::json!({
            "success": false,
            "error": "missing_linear_api_key",
        });
    };

    let (query, variables) = match parse_linear_graphql_input(event) {
        Ok(input) => input,
        Err(error) => {
            return serde_json::json!({
                "success": false,
                "error": "invalid_tool_input",
                "output": {"message": error},
            });
        }
    };

    if !contains_exactly_one_graphql_operation(&query) {
        return serde_json::json!({
            "success": false,
            "error": "invalid_tool_input",
            "output": {"message": "query must contain exactly one GraphQL operation"},
        });
    }

    let request_body = serde_json::json!({
        "query": query,
        "variables": variables,
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(LINEAR_TOOL_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return serde_json::json!({
                "success": false,
                "error": "client_build_failure",
                "output": {"message": error.to_string()},
            });
        }
    };
    let response = match client
        .post(&tool_context.tracker_endpoint)
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return serde_json::json!({
                "success": false,
                "error": "transport_failure",
                "output": {"message": error.to_string()},
            });
        }
    };

    let status = response.status().as_u16();
    let body = match response.json::<serde_json::Value>().await {
        Ok(body) => body,
        Err(error) => {
            return serde_json::json!({
                "success": false,
                "error": "response_error",
                "output": {"status": status, "message": format!("invalid graphql payload: {error}")},
            });
        }
    };

    if status >= 400 {
        return serde_json::json!({
            "success": false,
            "error": "transport_failure",
            "output": {"status": status, "body": body},
        });
    }

    if body
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return serde_json::json!({
            "success": false,
            "error": "graphql_error",
            "output": body,
        });
    }

    serde_json::json!({
        "success": true,
        "output": body,
    })
}

fn parse_linear_graphql_input(
    event: &symphony_agent_protocol::AppServerEvent,
) -> Result<(String, serde_json::Value), &'static str> {
    let Some(arguments) = extract_tool_arguments(event) else {
        return Err("tool call is missing arguments");
    };

    match arguments {
        serde_json::Value::String(raw_query) => {
            let query = raw_query.trim();
            if query.is_empty() {
                return Err("query must be a non-empty string");
            }
            Ok((query.to_owned(), serde_json::json!({})))
        }
        serde_json::Value::Object(map) => {
            let Some(query) = map
                .get("query")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|query| !query.is_empty())
            else {
                return Err("query must be a non-empty string");
            };

            let variables = map
                .get("variables")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if !variables.is_object() {
                return Err("variables must be a JSON object");
            }
            Ok((query.to_owned(), variables))
        }
        _ => Err("arguments must be a string or object"),
    }
}

fn extract_tool_arguments(
    event: &symphony_agent_protocol::AppServerEvent,
) -> Option<&serde_json::Value> {
    let params = &event.params;
    params
        .get("arguments")
        .or_else(|| params.get("args"))
        .or_else(|| params.get("input"))
        .or_else(|| params.get("payload"))
        .or_else(|| params.get("tool").and_then(|tool| tool.get("arguments")))
        .or_else(|| params.get("call").and_then(|call| call.get("arguments")))
}

fn contains_exactly_one_graphql_operation(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with('{') {
        return true;
    }

    let operation_markers = trimmed
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .filter(|token| {
            token.eq_ignore_ascii_case("query")
                || token.eq_ignore_ascii_case("mutation")
                || token.eq_ignore_ascii_case("subscription")
        })
        .count();

    operation_markers == 1
}

fn map_policy_outcome_to_worker_outcome(policy_outcome: ProtocolPolicyOutcome) -> WorkerOutcome {
    match policy_outcome {
        ProtocolPolicyOutcome::RetryableFailure(_) => WorkerOutcome::RetryableFailure,
        ProtocolPolicyOutcome::PermanentFailure(_) => WorkerOutcome::PermanentFailure,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ShellHookExecutor;

impl HookExecutor for ShellHookExecutor {
    fn execute(&self, request: &HookRequest) -> Result<HookResult, String> {
        let mut child = shell_command_sync(&request.command);
        child.current_dir(&request.workspace_path);
        child.stdin(Stdio::null());
        child.stdout(Stdio::piped());
        child.stderr(Stdio::piped());

        let mut child = child.spawn().map_err(|error| error.to_string())?;
        let timeout = Duration::from_millis(request.timeout_ms.max(1));
        let started = Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let output = child
                        .wait_with_output()
                        .map_err(|error| error.to_string())?;
                    return Ok(HookResult {
                        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                        timed_out: false,
                        exit_code: output.status.code(),
                        truncated: false,
                    });
                }
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        let _ = child.kill();
                        let output = child
                            .wait_with_output()
                            .map_err(|error| error.to_string())?;
                        return Ok(HookResult {
                            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                            timed_out: true,
                            exit_code: output.status.code(),
                            truncated: false,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }
}

fn shell_command_async(command: &str) -> tokio::process::Command {
    if cfg!(windows) {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd.kill_on_drop(true);
        return cmd;
    }

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-lc").arg(command);
    cmd.kill_on_drop(true);
    cmd
}

fn shell_command_sync(command: &str) -> std::process::Command {
    if cfg!(windows) {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/C").arg(command);
        return cmd;
    }

    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-lc").arg(command);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use async_trait::async_trait;
    use symphony_tracker::{TrackerClient, TrackerError, TrackerState};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path},
    };

    struct StaticTrackerClient;

    struct FixedTrackerClient {
        issues: Vec<TrackerIssue>,
    }

    #[async_trait]
    impl TrackerClient for StaticTrackerClient {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl TrackerClient for FixedTrackerClient {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(self.issues.clone())
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("symphony-worker-{label}-{timestamp}"))
    }

    fn protocol_test_state() -> (
        Option<mpsc::UnboundedSender<WorkerProtocolUpdate>>,
        SessionMetadata,
    ) {
        (None, SessionMetadata::default())
    }

    fn create_test_context() -> WorkerContext {
        let issue = TrackerIssue {
            id: IssueId("SYM-123".to_owned()),
            identifier: "SYM-123".to_owned(),
            title: "Test Issue".to_owned(),
            description: Some("Test description".to_owned()),
            priority: None,
            state: TrackerState::new("Todo"),
            branch_name: None,
            url: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: None,
            updated_at: None,
        };
        let root = test_root("workspace");
        let hooks = WorkspaceHooks::default();
        let tracker_config = TrackerConfig::default();
        let codex_config = CodexConfig::default();
        let prompt_template =
            "Issue {{issue_id}}: {{issue_title}}\n{{issue_description}}\nAttempt: {{attempt}}"
                .to_owned();
        let tracker_client: Arc<dyn TrackerClient> = Arc::new(StaticTrackerClient);
        let (_stop_tx, stop_rx) = watch::channel(false);

        WorkerContext::new(
            issue,
            1,
            WorkerExecutionConfig {
                root,
                hooks,
                tracker_config,
                codex_config,
                prompt_template,
            },
            WorkerRuntimeHandles {
                tracker_client,
                active_states: Vec::new(),
                max_turns: 3,
                protocol_updates: None,
                stop_rx,
            },
        )
        .expect("Failed to create context")
    }

    fn create_test_context_with_runtime_handles(
        runtime_handles: WorkerRuntimeHandles,
    ) -> WorkerContext {
        let issue = TrackerIssue {
            id: IssueId("SYM-123".to_owned()),
            identifier: "SYM-123".to_owned(),
            title: "Test Issue".to_owned(),
            description: Some("Test description".to_owned()),
            priority: None,
            state: TrackerState::new("Todo"),
            branch_name: None,
            url: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: None,
            updated_at: None,
        };

        WorkerContext::new(
            issue,
            1,
            WorkerExecutionConfig {
                root: test_root("workspace"),
                hooks: WorkspaceHooks::default(),
                tracker_config: TrackerConfig::default(),
                codex_config: CodexConfig::default(),
                prompt_template:
                    "Issue {{issue_id}}: {{issue_title}}\n{{issue_description}}\nAttempt: {{attempt}}"
                        .to_owned(),
            },
            runtime_handles,
        )
        .expect("Failed to create context")
    }

    fn launch_options<'a>(
        turn_title: &'a str,
        turn_timeout_ms: u64,
        read_timeout_ms: u64,
        approval_policy: Option<JsonValue>,
        thread_sandbox: Option<&'a str>,
        turn_sandbox_policy: Option<JsonValue>,
    ) -> LaunchOptions<'a> {
        LaunchOptions {
            turn_title,
            turn_timeout_ms,
            read_timeout_ms,
            approval_policy,
            thread_sandbox,
            turn_sandbox_policy,
            tracker_kind: "linear",
            tracker_endpoint: "https://api.linear.app/graphql",
            tracker_api_key: Some("linear-api-key"),
        }
    }

    fn non_linear_tool_context() -> ToolCallExecutionContext {
        ToolCallExecutionContext {
            tracker_kind: "jira".to_owned(),
            tracker_endpoint: "https://example.invalid/graphql".to_owned(),
            tracker_api_key: None,
        }
    }

    fn linear_tool_context() -> ToolCallExecutionContext {
        ToolCallExecutionContext {
            tracker_kind: "linear".to_owned(),
            tracker_endpoint: "https://linear.example/graphql".to_owned(),
            tracker_api_key: Some("linear-api-key".to_owned()),
        }
    }

    fn tool_call_event(params: serde_json::Value) -> symphony_agent_protocol::AppServerEvent {
        symphony_agent_protocol::AppServerEvent {
            id: Some(serde_json::json!("tool-call-1")),
            method: "item/tool/call".to_owned(),
            params,
            result: None,
            error: None,
        }
    }

    #[test]
    fn worker_context_creates_with_safety_checks() {
        let ctx = create_test_context();
        assert_eq!(ctx.issue.id.0, "SYM-123");
        assert_eq!(ctx.attempt, 1);
        assert_eq!(ctx.issue.title, "Test Issue");
    }

    #[test]
    fn worker_context_provides_issue_id() {
        let ctx = create_test_context();
        assert_eq!(ctx.issue_id().0, "SYM-123");
    }

    #[test]
    fn worker_context_provides_workspace_path() {
        let ctx = create_test_context();
        let ws_path = ctx.workspace_path();
        assert!(ws_path.ends_with("SYM-123"));
    }

    #[test]
    fn render_prompt_replaces_placeholders() {
        let ctx = create_test_context();
        let prompt = render_prompt(&ctx).expect("Failed to render prompt");
        assert!(prompt.contains("Issue SYM-123"));
        assert!(prompt.contains("Test Issue"));
        assert!(prompt.contains("Test description"));
        assert!(prompt.contains("Attempt: 1"));
    }

    #[test]
    fn render_prompt_handles_missing_description() {
        let mut ctx = create_test_context();
        ctx.issue.description = None;
        let prompt = render_prompt(&ctx).expect("Failed to render prompt");
        // Should not panic with None description
        assert!(prompt.contains("Issue SYM-123"));
    }

    #[test]
    fn render_prompt_rejects_unknown_placeholders() {
        let mut ctx = create_test_context();
        ctx.prompt_template = "Issue {{issue.missing_field}}".to_owned();

        let result = render_prompt(&ctx);

        assert!(matches!(result, Err(WorkerError::PromptError)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_worker_launcher_runs_after_run_hook_best_effort_on_prompt_error() {
        let mut ctx = create_test_context();
        let marker_path = ctx.workspace_path().join("after-run.marker");
        ctx.prompt_template = "Issue {{issue.unknown}}".to_owned();
        ctx.codex_config.command = "printf 'worker should not start\\n'".to_owned();
        ctx.hooks.after_run = Some(format!(
            "printf 'after-run' > '{}' && exit 7",
            marker_path.display()
        ));

        let result = ShellWorkerLauncher.launch(ctx).await;

        assert!(matches!(result, Err(WorkerError::PromptError)));
        assert_eq!(
            fs::read_to_string(&marker_path).expect("after_run hook should leave a marker"),
            "after-run"
        );
    }

    #[tokio::test]
    async fn protocol_chunk_detects_input_required_events() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (protocol_updates, mut session) = protocol_test_state();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/input_required","params":{"input_required":true}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::PermanentFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_maps_turn_timeout_to_retryable_failure() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (protocol_updates, mut session) = protocol_test_state();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/failed","error":{"code":"turn_timeout"}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::RetryableFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_maps_turn_cancelled_to_retryable_failure() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (protocol_updates, mut session) = protocol_test_state();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/cancelled","params":{}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::RetryableFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_does_not_hard_fail_on_unsupported_tool_call_marker() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (protocol_updates, mut session) = protocol_test_state();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"unsupported_tool_call","params":{}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);
    }

    #[tokio::test]
    async fn protocol_chunk_replies_unsupported_for_tool_call_requests() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (mut write_half, mut read_half) = tokio::io::duplex(1_024);
        let (protocol_updates, mut session) = protocol_test_state();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"id":"call-7","method":"item/tool/call","params":{"name":"custom_tool","arguments":{"query":"{ viewer { id } }"}}}
"#,
            Some(&mut write_half),
            &tool_context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        drop(write_half);
        let mut response = Vec::new();
        read_half
            .read_to_end(&mut response)
            .await
            .expect("response should be readable");
        let encoded = String::from_utf8(response).expect("response should be utf8");
        let first_line = encoded
            .lines()
            .next()
            .expect("response should contain one json line");
        let payload: serde_json::Value =
            serde_json::from_str(first_line).expect("response line should be valid json");
        assert_eq!(payload["id"], "call-7");
        assert_eq!(payload["result"]["success"], false);
        assert_eq!(payload["result"]["error"], "unsupported_tool_call");
    }

    #[test]
    fn supported_tools_include_linear_graphql_only_when_linear_tracker_is_configured() {
        let enabled = supported_tools_for_options(&launch_options(
            "SYM-1: prompt",
            2_000,
            200,
            None,
            None,
            None,
        ));
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "linear_graphql");

        let disabled = supported_tools_for_options(&LaunchOptions {
            turn_title: "SYM-1: prompt",
            turn_timeout_ms: 2_000,
            read_timeout_ms: 200,
            approval_policy: None,
            thread_sandbox: None,
            turn_sandbox_policy: None,
            tracker_kind: "linear",
            tracker_endpoint: "https://linear.example/graphql",
            tracker_api_key: None,
        });
        assert!(disabled.is_empty());
    }

    #[test]
    fn parse_linear_graphql_input_accepts_string_and_object_shapes() {
        let raw_event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": "{ viewer { id } }"
        }));
        let (raw_query, raw_variables) =
            parse_linear_graphql_input(&raw_event).expect("raw string query should parse");
        assert_eq!(raw_query, "{ viewer { id } }");
        assert_eq!(raw_variables, serde_json::json!({}));

        let object_event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {
                "query": "query Viewer { viewer { id } }",
                "variables": {"limit": 5}
            }
        }));
        let (query, variables) =
            parse_linear_graphql_input(&object_event).expect("object query should parse");
        assert_eq!(query, "query Viewer { viewer { id } }");
        assert_eq!(variables, serde_json::json!({"limit": 5}));
    }

    #[test]
    fn graphql_operation_counter_requires_exactly_one_operation() {
        assert!(contains_exactly_one_graphql_operation("{ viewer { id } }"));
        assert!(contains_exactly_one_graphql_operation(
            "query Viewer { viewer { id } }"
        ));
        assert!(!contains_exactly_one_graphql_operation(
            "query A { viewer { id } } query B { teams { id } }"
        ));
    }

    #[tokio::test]
    async fn tool_call_result_reports_missing_linear_configuration() {
        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {"query": "{ viewer { id } }"}
        }));
        let result = tool_call_result(&event, &non_linear_tool_context()).await;
        assert_eq!(result["success"], false);
        assert_eq!(result["error"], "linear_tracker_not_configured");
    }

    #[tokio::test]
    async fn tool_call_result_reports_transport_failure_when_endpoint_is_unreachable() {
        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {"query": "{ viewer { id } }"}
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = "http://127.0.0.1:1/graphql".to_owned();
        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], false);
        assert_eq!(result["error"], "transport_failure");
    }

    #[tokio::test]
    async fn tool_call_result_executes_linear_graphql_successfully() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer linear-api-key"))
            .and(body_partial_json(serde_json::json!({
                "query": "query Viewer { viewer { id } }",
                "variables": {"limit": 5}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "viewer": {"id": "viewer-1"}
                }
            })))
            .mount(&server)
            .await;

        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {
                "query": "query Viewer { viewer { id } }",
                "variables": {"limit": 5}
            }
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());

        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], true);
        assert_eq!(result["output"]["data"]["viewer"]["id"], "viewer-1");
    }

    #[tokio::test]
    async fn tool_call_result_surfaces_graphql_errors_from_linear() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "errors": [{"message": "permission denied"}]
            })))
            .mount(&server)
            .await;

        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {"query": "{ viewer { id } }"}
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());

        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], false);
        assert_eq!(result["error"], "graphql_error");
        assert_eq!(
            result["output"]["errors"][0]["message"],
            "permission denied"
        );
    }

    #[tokio::test]
    async fn tool_call_result_surfaces_status_errors_from_linear() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(503)
                    .insert_header("content-type", "application/json")
                    .set_body_json(serde_json::json!({"message": "upstream unavailable"})),
            )
            .mount(&server)
            .await;

        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {"query": "{ viewer { id } }"}
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());

        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], false);
        assert_eq!(result["error"], "transport_failure");
        assert_eq!(result["output"]["status"], 503);
    }

    #[tokio::test]
    async fn tool_call_result_reports_successful_linear_graphql_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer linear-api-key"))
            .and(body_partial_json(serde_json::json!({
                "query": "query Viewer { viewer { id } }",
                "variables": {"limit": 5}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "viewer": {
                        "id": "usr_123"
                    }
                }
            })))
            .mount(&server)
            .await;

        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {
                "query": "query Viewer { viewer { id } }",
                "variables": {"limit": 5}
            }
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());
        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], true);
        assert_eq!(result["output"]["data"]["viewer"]["id"], "usr_123");
    }

    #[tokio::test]
    async fn tool_call_result_reports_graphql_error_payloads() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "errors": [{"message": "permission denied"}]
            })))
            .mount(&server)
            .await;

        let event = tool_call_event(serde_json::json!({
            "name": "linear_graphql",
            "arguments": {"query": "{ viewer { id } }"}
        }));
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());
        let result = tool_call_result(&event, &context).await;
        assert_eq!(result["success"], false);
        assert_eq!(result["error"], "graphql_error");
        assert_eq!(
            result["output"]["errors"][0]["message"],
            "permission denied"
        );
    }

    #[tokio::test]
    async fn protocol_chunk_executes_supported_tool_calls_and_writes_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"viewer": {"id": "usr_456"}}
            })))
            .mount(&server)
            .await;

        let mut parser = StreamLineParser::default();
        let (mut write_half, mut read_half) = tokio::io::duplex(2_048);
        let mut context = linear_tool_context();
        context.tracker_endpoint = format!("{}/graphql", server.uri());
        let (protocol_updates, mut session) = protocol_test_state();

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"id":"call-9","method":"item/tool/call","params":{"name":"linear_graphql","arguments":{"query":"{ viewer { id } }"}}}
"#,
            Some(&mut write_half),
            &context,
            &protocol_updates,
            &mut session,
        )
        .await;
        assert_eq!(signal, None);

        drop(write_half);
        let mut response = Vec::new();
        read_half
            .read_to_end(&mut response)
            .await
            .expect("response should be readable");
        let encoded = String::from_utf8(response).expect("response should be utf8");
        let first_line = encoded
            .lines()
            .next()
            .expect("response should contain one json line");
        let payload: serde_json::Value =
            serde_json::from_str(first_line).expect("response line should be valid json");
        assert_eq!(payload["id"], "call-9");
        assert_eq!(payload["result"]["success"], true);
        assert_eq!(
            payload["result"]["output"]["data"]["viewer"]["id"],
            "usr_456"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_hard_fails_on_input_required_signal() {
        let command = "printf '%s\\n' '{\"method\":\"turn/input_required\",\"params\":{\"input_required\":true}}'; sleep 30";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_retries_on_turn_timeout_signal() {
        let command = "printf '%s\\n' '{\"method\":\"turn/failed\",\"error\":{\"code\":\"turn_timeout\"}}'; sleep 30";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_retries_on_response_timeout_signal() {
        let command = "printf '%s\\n' '{\"method\":\"startup/failed\",\"error\":{\"code\":\"response_timeout\"}}'; sleep 30";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_handshake_path_reports_permanent_failure() {
        let command = "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-1\"}}}'; printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-1\"}}}'; printf '%s\\n' '{\"method\":\"turn/input_required\",\"params\":{\"input_required\":true}}'; sleep 30 # app-server";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                4_000,
                2_000,
                Some(serde_json::json!("auto")),
                Some("workspace-write"),
                Some(serde_json::json!("workspace-write")),
            ),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_unsupported_tool_call_is_not_permanent_failure() {
        let command = "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-1\"}}}'; printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-1\"}}}'; printf '%s\\n' '{\"id\":\"call-1\",\"method\":\"item/tool/call\",\"params\":{\"name\":\"unknown_tool\"}}'; printf '%s\\n' '{\"method\":\"turn/completed\",\"result\":{}}' # app-server";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                500,
                Some(serde_json::json!("auto")),
                Some("workspace-write"),
                Some(serde_json::json!("workspace-write")),
            ),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_ne!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_handshake_read_timeout_is_retryable() {
        let command = "sleep 30 # app-server";
        let ctx = create_test_context();
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                100,
                Some(serde_json::json!("auto")),
                Some("workspace-write"),
                Some(serde_json::json!("workspace-write")),
            ),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_reuses_one_thread_and_process_across_turns() {
        let issue = TrackerIssue {
            id: IssueId("SYM-123".to_owned()),
            identifier: "SYM-123".to_owned(),
            title: "Test Issue".to_owned(),
            description: Some("Test description".to_owned()),
            priority: None,
            state: TrackerState::new("Todo"),
            branch_name: None,
            url: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: None,
            updated_at: None,
        };
        let tracker_client: Arc<dyn TrackerClient> = Arc::new(FixedTrackerClient {
            issues: vec![issue.clone()],
        });
        let (protocol_update_tx, mut protocol_update_rx) = mpsc::unbounded_channel();
        let (_stop_tx, stop_rx) = watch::channel(false);
        let ctx = create_test_context_with_runtime_handles(WorkerRuntimeHandles {
            tracker_client,
            active_states: vec![TrackerState::new("Todo")],
            max_turns: 2,
            protocol_updates: Some(protocol_update_tx),
            stop_rx,
        });
        let command = concat!(
            "IFS= read -r _; ",
            "IFS= read -r _; ",
            "IFS= read -r _; ",
            "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-1\"}}}'; ",
            "IFS= read -r _; ",
            "printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-1\"}}}'; ",
            "sleep 0.05; ",
            "printf '%s\\n' '{\"method\":\"turn/completed\",\"result\":{}}'; ",
            "IFS= read -r _; ",
            "printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-2\"}}}'; ",
            "sleep 0.05; ",
            "printf '%s\\n' '{\"method\":\"turn/completed\",\"result\":{}}' ",
            "# app-server"
        );

        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                500,
                Some(serde_json::json!("auto")),
                Some("workspace-write"),
                Some(serde_json::json!("workspace-write")),
            ),
            &ctx,
        )
        .await
        .expect("launch should return worker outcome");

        let mut turn_updates = Vec::new();
        while let Ok(update) = protocol_update_rx.try_recv() {
            if update.event.as_deref() == Some("turn/start") {
                turn_updates.push(update);
            }
        }

        assert_eq!(outcome, WorkerOutcome::Completed);

        assert_eq!(turn_updates.len(), 2);
        assert_eq!(turn_updates[0].thread_id.as_deref(), Some("thread-1"));
        assert_eq!(turn_updates[1].thread_id.as_deref(), Some("thread-1"));
        assert_eq!(turn_updates[0].turn_id.as_deref(), Some("turn-1"));
        assert_eq!(turn_updates[1].turn_id.as_deref(), Some("turn-2"));
        assert_eq!(turn_updates[0].pid, turn_updates[1].pid);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_stop_signal_kills_child_process() {
        let (protocol_update_tx, mut protocol_update_rx) = mpsc::unbounded_channel();
        let tracker_client: Arc<dyn TrackerClient> = Arc::new(StaticTrackerClient);
        let (stop_tx, stop_rx) = watch::channel(false);
        let ctx = create_test_context_with_runtime_handles(WorkerRuntimeHandles {
            tracker_client,
            active_states: Vec::new(),
            max_turns: 1,
            protocol_updates: Some(protocol_update_tx),
            stop_rx,
        });
        let command = concat!(
            "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-stop\"}}}'; ",
            "sleep 0.05; ",
            "printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-stop\"}}}'; ",
            "sleep 30 ",
            "# app-server"
        );

        let launch = tokio::spawn(async move {
            launch_agent_command(
                command,
                std::env::temp_dir().as_path(),
                "prompt",
                launch_options(
                    "SYM-1: prompt",
                    5_000,
                    500,
                    Some(serde_json::json!("auto")),
                    Some("workspace-write"),
                    Some(serde_json::json!("workspace-write")),
                ),
                &ctx,
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = stop_tx.send(true);

        let outcome = launch
            .await
            .expect("task join should succeed")
            .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::Stopped);

        let mut pid = None;
        while let Ok(update) = protocol_update_rx.try_recv() {
            if update.event.as_deref() == Some("worker/started") {
                pid = update.pid;
            }
        }
        let pid = pid.expect("worker start update should include pid");

        for _ in 0..20 {
            let status = std::process::Command::new("sh")
                .arg("-lc")
                .arg(format!("kill -0 {pid} >/dev/null 2>&1"))
                .status()
                .expect("kill probe should run");
            if !status.success() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        panic!("child process {pid} still appears to be running after stop");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_abort_kills_child_process() {
        let (protocol_update_tx, mut protocol_update_rx) = mpsc::unbounded_channel();
        let tracker_client: Arc<dyn TrackerClient> = Arc::new(StaticTrackerClient);
        let (_stop_tx, stop_rx) = watch::channel(false);
        let ctx = create_test_context_with_runtime_handles(WorkerRuntimeHandles {
            tracker_client,
            active_states: Vec::new(),
            max_turns: 1,
            protocol_updates: Some(protocol_update_tx),
            stop_rx,
        });
        let command = concat!(
            "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-abort\"}}}'; ",
            "sleep 0.05; ",
            "printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-abort\"}}}'; ",
            "sleep 30 ",
            "# app-server"
        );

        let launch = tokio::spawn(async move {
            launch_agent_command(
                command,
                std::env::temp_dir().as_path(),
                "prompt",
                launch_options(
                    "SYM-1: prompt",
                    5_000,
                    500,
                    Some(serde_json::json!("auto")),
                    Some("workspace-write"),
                    Some(serde_json::json!("workspace-write")),
                ),
                &ctx,
            )
            .await
        });

        let pid = loop {
            let update = tokio::time::timeout(Duration::from_secs(2), protocol_update_rx.recv())
                .await
                .expect("worker start update should arrive")
                .expect("protocol channel should stay open");
            if update.event.as_deref() == Some("worker/started") {
                break update.pid.expect("worker start update should include pid");
            }
        };

        launch.abort();
        let _ = launch.await;

        for _ in 0..20 {
            let status = std::process::Command::new("sh")
                .arg("-lc")
                .arg(format!("kill -0 {pid} >/dev/null 2>&1"))
                .status()
                .expect("kill probe should run");
            if !status.success() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        panic!("child process {pid} still appears to be running after abort");
    }
}
