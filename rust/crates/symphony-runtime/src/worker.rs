use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use symphony_agent_protocol::{
    LineOrigin, ProtocolMethodKind, ProtocolPolicyOutcome, StreamLineParser, SupportedToolSpec,
    build_initialize_request, build_initialized_notification, build_thread_start_request,
    build_turn_start_request, classify_policy_outcome, decode_stdout_line, extract_thread_id,
    extract_tool_call_id, extract_tool_name, extract_turn_id,
};
use symphony_config::{CodexConfig, TrackerConfig};
use symphony_domain::IssueId;
use symphony_tracker::TrackerIssue;
use symphony_workspace::{
    HookExecutor, HookRequest, HookResult, PreparedWorkspace, WorkspaceError, WorkspaceHooks,
    prepare_workspace_with_hooks, run_after_run_hook, run_before_run_hook, workspace_path,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// Context for spawning a worker with full workspace and prompt information
#[derive(Clone)]
pub struct WorkerContext {
    pub issue: TrackerIssue,
    pub attempt: u32,
    pub root: PathBuf,
    pub hooks: WorkspaceHooks,
    pub tracker_config: TrackerConfig,
    pub codex_config: CodexConfig,
    pub prompt_template: String,
}

impl WorkerContext {
    /// Create a new WorkerContext ensuring workspace path safety
    pub fn new(
        issue: TrackerIssue,
        attempt: u32,
        root: PathBuf,
        hooks: WorkspaceHooks,
        tracker_config: TrackerConfig,
        codex_config: CodexConfig,
        prompt_template: String,
    ) -> Result<Self, WorkspaceError> {
        // Validate workspace path is valid and within root
        let _ = workspace_path(&root, &issue.identifier)?;

        Ok(Self {
            issue,
            attempt,
            root,
            hooks,
            tracker_config,
            codex_config,
            prompt_template,
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
}

#[derive(Clone, Copy, Debug)]
struct LaunchOptions<'a> {
    turn_title: &'a str,
    turn_timeout_ms: u64,
    read_timeout_ms: u64,
    approval_policy: Option<&'a str>,
    thread_sandbox: Option<&'a str>,
    turn_sandbox_policy: Option<&'a str>,
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

impl<'a> From<LaunchOptions<'a>> for ToolCallExecutionContext {
    fn from(options: LaunchOptions<'a>) -> Self {
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
            approval_policy: ctx.codex_config.approval_policy.as_deref(),
            thread_sandbox: ctx.codex_config.thread_sandbox.as_deref(),
            turn_sandbox_policy: ctx.codex_config.turn_sandbox_policy.as_deref(),
            tracker_kind: &ctx.tracker_config.kind,
            tracker_endpoint: &ctx.tracker_config.endpoint,
            tracker_api_key: ctx.tracker_config.api_key.as_deref(),
        };
        let launch_result =
            launch_agent_command(&command, &workspace.path, &prompt, launch_options).await;
        let after_run_result = run_after_run_hook(&workspace, &ctx.hooks, &hook_executor);
        if let Err(error) = after_run_result {
            return Err(WorkerError::Workspace(error));
        }

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

/// Render prompt template with issue context
/// Replaces common placeholders with actual issue data
pub fn render_prompt(ctx: &WorkerContext) -> Result<String, WorkerError> {
    // Simple placeholder replacement for now
    // Future enhancement: Use proper templating engine (e.g., handlebars, tera)
    let empty = String::new();
    let description = ctx.issue.description.as_deref().unwrap_or(&empty);
    let prompt = ctx
        .prompt_template
        .replace("{{issue_id}}", &ctx.issue.id.0)
        .replace("{{issue_identifier}}", &ctx.issue.identifier)
        .replace("{{issue_title}}", &ctx.issue.title)
        .replace("{{issue_description}}", description)
        .replace("{{attempt}}", &ctx.attempt.to_string());

    Ok(prompt)
}

async fn launch_agent_command(
    command: &str,
    cwd: &std::path::Path,
    prompt: &str,
    options: LaunchOptions<'_>,
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

    let tool_context = ToolCallExecutionContext::from(options);
    let app_server_mode = command_looks_like_app_server(command);
    let stdout_reader = if app_server_mode {
        match perform_startup_handshake(
            &mut stdin,
            stdout,
            cwd,
            prompt,
            Duration::from_millis(options.read_timeout_ms.max(1)),
            options,
        )
        .await?
        {
            StartupHandshakeOutcome::Ready(stdout_reader) => stdout_reader,
            StartupHandshakeOutcome::WorkerOutcome(outcome) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Ok(outcome);
            }
        }
    } else {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;
        BufReader::new(stdout)
    };

    let stdin_for_monitor = if app_server_mode {
        Some(stdin)
    } else {
        drop(stdin);
        None
    };

    let mut protocol_watch = tokio::spawn(async move {
        monitor_protocol_streams(stdout_reader, stderr, stdin_for_monitor, tool_context).await
    });

    let timeout = Duration::from_millis(options.turn_timeout_ms.max(1));
    let deadline = tokio::time::Instant::now() + timeout;
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

enum StartupHandshakeOutcome {
    Ready(BufReader<tokio::process::ChildStdout>),
    WorkerOutcome(WorkerOutcome),
}

enum StartupIdentifierOutcome {
    Identifier(String),
    WorkerOutcome(WorkerOutcome),
}

async fn perform_startup_handshake(
    stdin: &mut tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    cwd: &std::path::Path,
    prompt: &str,
    read_timeout: Duration,
    options: LaunchOptions<'_>,
) -> Result<StartupHandshakeOutcome, WorkerError> {
    let supported_tools = supported_tools_for_options(options);
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
        options.approval_policy,
        options.thread_sandbox,
    );
    write_protocol_message(stdin, &thread_start).await?;

    let mut reader = BufReader::new(stdout);
    let thread_id =
        match wait_for_startup_identifier(&mut reader, read_timeout, extract_thread_id).await? {
            StartupIdentifierOutcome::Identifier(thread_id) => thread_id,
            StartupIdentifierOutcome::WorkerOutcome(outcome) => {
                return Ok(StartupHandshakeOutcome::WorkerOutcome(outcome));
            }
        };

    let turn_start = build_turn_start_request(
        serde_json::json!(3),
        &thread_id,
        prompt,
        cwd,
        options.turn_title,
        options.approval_policy,
        options.turn_sandbox_policy,
    );
    write_protocol_message(stdin, &turn_start).await?;

    match wait_for_startup_identifier(&mut reader, read_timeout, extract_turn_id).await? {
        StartupIdentifierOutcome::Identifier(_) => {}
        StartupIdentifierOutcome::WorkerOutcome(outcome) => {
            return Ok(StartupHandshakeOutcome::WorkerOutcome(outcome));
        }
    }

    Ok(StartupHandshakeOutcome::Ready(reader))
}

async fn wait_for_startup_identifier(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    read_timeout: Duration,
    extractor: fn(&symphony_agent_protocol::AppServerEvent) -> Option<String>,
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
        let read_result = tokio::time::timeout(remaining, reader.read_line(&mut line)).await;
        let bytes_read = match read_result {
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

async fn write_protocol_message(
    stdin: &mut (impl AsyncWrite + Unpin),
    payload: &serde_json::Value,
) -> Result<(), WorkerError> {
    let encoded = serde_json::to_vec(payload)
        .map_err(|error| WorkerError::LaunchFailed(format!("protocol encode failed: {error}")))?;
    stdin
        .write_all(&encoded)
        .await
        .map_err(|error| WorkerError::LaunchFailed(format!("protocol write failed: {error}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|error| WorkerError::LaunchFailed(format!("protocol write failed: {error}")))?;
    Ok(())
}

fn supported_tools_for_options(options: LaunchOptions<'_>) -> Vec<SupportedToolSpec> {
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
                WorkerOutcome::Completed | WorkerOutcome::Continuation => {
                    WorkerProtocolSignal::Continue
                }
            });
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
    write_protocol_message(stdin, &response).await
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
        return cmd;
    }

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-lc").arg(command);
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
    use symphony_tracker::TrackerState;

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
        let root = std::env::temp_dir().join("test-workspaces");
        let hooks = WorkspaceHooks::default();
        let tracker_config = TrackerConfig::default();
        let codex_config = CodexConfig::default();
        let prompt_template =
            "Issue {{issue_id}}: {{issue_title}}\n{{issue_description}}\nAttempt: {{attempt}}"
                .to_owned();

        WorkerContext::new(
            issue,
            1,
            root,
            hooks,
            tracker_config,
            codex_config,
            prompt_template,
        )
        .expect("Failed to create context")
    }

    fn launch_options<'a>(
        turn_title: &'a str,
        turn_timeout_ms: u64,
        read_timeout_ms: u64,
        approval_policy: Option<&'a str>,
        thread_sandbox: Option<&'a str>,
        turn_sandbox_policy: Option<&'a str>,
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

    #[tokio::test]
    async fn protocol_chunk_detects_input_required_events() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/input_required","params":{"input_required":true}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::PermanentFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_maps_turn_timeout_to_retryable_failure() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/failed","error":{"code":"turn_timeout"}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::RetryableFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_maps_turn_cancelled_to_retryable_failure() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"turn/cancelled","params":{}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, Some(WorkerProtocolSignal::RetryableFailure));
    }

    #[tokio::test]
    async fn protocol_chunk_does_not_hard_fail_on_unsupported_tool_call_marker() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"method":"unsupported_tool_call","params":{}}"#,
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, None);

        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            "\n",
            None::<&mut tokio::io::DuplexStream>,
            &tool_context,
        )
        .await;
        assert_eq!(signal, None);
    }

    #[tokio::test]
    async fn protocol_chunk_replies_unsupported_for_tool_call_requests() {
        let mut parser = StreamLineParser::default();
        let tool_context = non_linear_tool_context();
        let (mut write_half, mut read_half) = tokio::io::duplex(1_024);
        let signal = protocol_chunk_signal(
            &mut parser,
            LineOrigin::Stdout,
            r#"{"id":"call-7","method":"item/tool/call","params":{"name":"custom_tool","arguments":{"query":"{ viewer { id } }"}}}
"#,
            Some(&mut write_half),
            &tool_context,
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
        let enabled = supported_tools_for_options(launch_options(
            "SYM-1: prompt",
            2_000,
            200,
            None,
            None,
            None,
        ));
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "linear_graphql");

        let disabled = supported_tools_for_options(LaunchOptions {
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

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_hard_fails_on_input_required_signal() {
        let command = "printf '%s\\n' '{\"method\":\"turn/input_required\",\"params\":{\"input_required\":true}}'; sleep 30";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_retries_on_turn_timeout_signal() {
        let command = "printf '%s\\n' '{\"method\":\"turn/failed\",\"error\":{\"code\":\"turn_timeout\"}}'; sleep 30";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_retries_on_response_timeout_signal() {
        let command = "printf '%s\\n' '{\"method\":\"startup/failed\",\"error\":{\"code\":\"response_timeout\"}}'; sleep 30";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options("SYM-1: prompt", 2_000, 200, None, None, None),
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_handshake_path_reports_permanent_failure() {
        let command = "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-1\"}}}'; printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-1\"}}}'; printf '%s\\n' '{\"method\":\"turn/input_required\",\"params\":{\"input_required\":true}}'; sleep 30 # app-server";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                500,
                Some("auto"),
                Some("workspace-write"),
                Some("workspace-write"),
            ),
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_unsupported_tool_call_is_not_permanent_failure() {
        let command = "printf '%s\\n' '{\"result\":{\"thread\":{\"id\":\"thread-1\"}}}'; printf '%s\\n' '{\"result\":{\"turn\":{\"id\":\"turn-1\"}}}'; printf '%s\\n' '{\"id\":\"call-1\",\"method\":\"item/tool/call\",\"params\":{\"name\":\"unknown_tool\"}}'; printf '%s\\n' '{\"method\":\"turn/completed\",\"result\":{}}' # app-server";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                500,
                Some("auto"),
                Some("workspace-write"),
                Some("workspace-write"),
            ),
        )
        .await
        .expect("launch should return worker outcome");
        assert_ne!(outcome, WorkerOutcome::PermanentFailure);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_agent_command_app_server_handshake_read_timeout_is_retryable() {
        let command = "sleep 30 # app-server";
        let outcome = launch_agent_command(
            command,
            std::env::temp_dir().as_path(),
            "prompt",
            launch_options(
                "SYM-1: prompt",
                2_000,
                100,
                Some("auto"),
                Some("workspace-write"),
                Some("workspace-write"),
            ),
        )
        .await
        .expect("launch should return worker outcome");
        assert_eq!(outcome, WorkerOutcome::RetryableFailure);
    }
}
