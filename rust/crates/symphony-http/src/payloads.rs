use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use symphony_observability::{
    IssueSnapshot, IssueStatusTotalsSnapshot, RuntimeHealthView, RuntimeSnapshot,
    RuntimeSummaryView, StateSnapshot, StateSummaryView, TaskMapSnapshot,
    format_json_compact_sorted, sanitize_json_value, sanitize_summary_text,
};
use symphony_workspace::workspace_path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeLegacyView {
    pub running: usize,
    pub retrying: usize,
}

impl RuntimeLegacyView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running,
            "retrying": self.retrying,
        })
    }
}

impl From<&RuntimeSnapshot> for RuntimeLegacyView {
    fn from(snapshot: &RuntimeSnapshot) -> Self {
        Self {
            running: snapshot.running,
            retrying: snapshot.retrying,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueLegacyView {
    pub id: String,
    pub identifier: String,
    pub state: String,
    pub retry_attempts: u32,
}

impl IssueLegacyView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "identifier": self.identifier,
            "state": self.state,
            "retry_attempts": self.retry_attempts,
        })
    }
}

impl From<&IssueSnapshot> for IssueLegacyView {
    fn from(issue: &IssueSnapshot) -> Self {
        Self {
            id: issue.id.0.clone(),
            identifier: issue.identifier.clone(),
            state: issue.state.clone(),
            retry_attempts: issue.retry_attempts,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageView {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

impl UsageView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "total_tokens": self.total_tokens,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodexTotalsView {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

impl CodexTotalsView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "total_tokens": self.total_tokens,
            "seconds_running": self.seconds_running,
        })
    }
}

impl From<&RuntimeSnapshot> for CodexTotalsView {
    fn from(snapshot: &RuntimeSnapshot) -> Self {
        Self {
            input_tokens: snapshot.input_tokens,
            output_tokens: snapshot.output_tokens,
            total_tokens: snapshot.total_tokens,
            seconds_running: normalize_operator_float(snapshot.seconds_running),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThroughputView {
    pub window_seconds: f64,
    pub input_tokens_per_second: f64,
    pub output_tokens_per_second: f64,
    pub total_tokens_per_second: f64,
}

impl ThroughputView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "window_seconds": self.window_seconds,
            "input_tokens_per_second": self.input_tokens_per_second,
            "output_tokens_per_second": self.output_tokens_per_second,
            "total_tokens_per_second": self.total_tokens_per_second,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeActivityView {
    pub poll_in_progress: bool,
    pub last_poll_started_at: Option<String>,
    pub last_poll_completed_at: Option<String>,
    pub next_poll_due_at: Option<String>,
    pub last_runtime_activity_at: Option<String>,
    pub throughput: ThroughputView,
}

impl RuntimeActivityView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "poll_in_progress": self.poll_in_progress,
            "last_poll_started_at": self.last_poll_started_at,
            "last_poll_completed_at": self.last_poll_completed_at,
            "next_poll_due_at": self.next_poll_due_at,
            "last_runtime_activity_at": self.last_runtime_activity_at,
            "throughput": self.throughput.to_json(),
        })
    }
}

impl From<&RuntimeSnapshot> for RuntimeActivityView {
    fn from(snapshot: &RuntimeSnapshot) -> Self {
        Self {
            poll_in_progress: snapshot.activity.poll_in_progress,
            last_poll_started_at: iso8601(snapshot.activity.last_poll_started_at),
            last_poll_completed_at: iso8601(snapshot.activity.last_poll_completed_at),
            next_poll_due_at: iso8601(snapshot.activity.next_poll_due_at),
            last_runtime_activity_at: iso8601(snapshot.activity.last_runtime_activity_at),
            throughput: ThroughputView {
                window_seconds: normalize_operator_float(
                    snapshot.activity.throughput.window_seconds,
                ),
                input_tokens_per_second: normalize_operator_float(
                    snapshot.activity.throughput.input_tokens_per_second,
                ),
                output_tokens_per_second: normalize_operator_float(
                    snapshot.activity.throughput.output_tokens_per_second,
                ),
                total_tokens_per_second: normalize_operator_float(
                    snapshot.activity.throughput.total_tokens_per_second,
                ),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateCountsView {
    pub running: usize,
    pub retrying: usize,
}

impl StateCountsView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running,
            "retrying": self.retrying,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningIssueView {
    pub issue_id: String,
    pub issue_identifier: String,
    pub state: String,
    pub session_id: Option<String>,
    pub turn_count: u32,
    pub last_event: Option<String>,
    pub last_message: Option<String>,
    pub started_at: Option<String>,
    pub last_event_at: Option<String>,
    pub tokens: UsageView,
}

impl RunningIssueView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "issue_id": self.issue_id,
            "issue_identifier": self.issue_identifier,
            "state": self.state,
            "session_id": self.session_id,
            "turn_count": self.turn_count,
            "last_event": self.last_event,
            "last_message": self.last_message,
            "started_at": self.started_at,
            "last_event_at": self.last_event_at,
            "tokens": self.tokens.to_json(),
        })
    }
}

impl From<&IssueSnapshot> for RunningIssueView {
    fn from(issue: &IssueSnapshot) -> Self {
        Self {
            issue_id: issue.id.0.clone(),
            issue_identifier: issue.identifier.clone(),
            state: issue.state.clone(),
            session_id: issue.session_id.clone(),
            turn_count: issue.turn_count,
            last_event: issue.last_event.clone(),
            last_message: summarize_codex_message(
                issue.last_event.as_deref(),
                issue.last_message.as_deref(),
            ),
            started_at: iso8601(issue.started_at),
            last_event_at: iso8601(issue.last_event_at),
            tokens: UsageView {
                input_tokens: issue.input_tokens,
                output_tokens: issue.output_tokens,
                total_tokens: issue.total_tokens,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryIssueView {
    pub issue_id: String,
    pub issue_identifier: String,
    pub attempt: u32,
    pub due_at: Option<String>,
    pub error: Option<String>,
}

impl RetryIssueView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "issue_id": self.issue_id,
            "issue_identifier": self.issue_identifier,
            "attempt": self.attempt,
            "due_at": self.due_at,
            "error": self.error,
        })
    }
}

impl From<&IssueSnapshot> for RetryIssueView {
    fn from(issue: &IssueSnapshot) -> Self {
        Self {
            issue_id: issue.id.0.clone(),
            issue_identifier: issue.identifier.clone(),
            attempt: issue.retry_attempts,
            due_at: iso8601(issue.retry_due_at),
            error: issue.retry_error.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateApiView {
    pub generated_at: String,
    pub counts: StateCountsView,
    pub running: Vec<RunningIssueView>,
    pub retrying: Vec<RetryIssueView>,
    pub codex_totals: CodexTotalsView,
    pub rate_limits: Option<serde_json::Value>,
    pub activity: RuntimeActivityView,
    pub health: RuntimeHealthView,
    pub issue_totals: IssueStatusTotalsSnapshot,
    pub task_maps: TaskMapSnapshot,
    pub summary: StateOperatorSummaryView,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateOperatorSummaryView {
    pub runtime: RuntimeSummaryView,
    pub state: StateSummaryView,
}

impl StateOperatorSummaryView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "runtime": self.runtime,
            "state": self.state,
        })
    }
}

impl StateApiView {
    pub fn to_json(&self) -> serde_json::Value {
        let running = self
            .running
            .iter()
            .map(RunningIssueView::to_json)
            .collect::<Vec<_>>();
        let retrying = self
            .retrying
            .iter()
            .map(RetryIssueView::to_json)
            .collect::<Vec<_>>();

        serde_json::json!({
            "generated_at": self.generated_at,
            "counts": self.counts.to_json(),
            "running": running,
            "retrying": retrying,
            "codex_totals": self.codex_totals.to_json(),
            "rate_limits": self.rate_limits,
            "activity": self.activity.to_json(),
            "health": self.health,
            "issue_totals": self.issue_totals,
            "task_maps": self.task_maps,
            "summary": self.summary.to_json(),
        })
    }
}

impl From<&StateSnapshot> for StateApiView {
    fn from(snapshot: &StateSnapshot) -> Self {
        let snapshot = snapshot.sanitized();
        let running_issues = snapshot.running_issues();
        let retrying_issues = snapshot.retrying_issues();
        let runtime_spec = snapshot.runtime.spec_view();
        let issue_totals = snapshot.issue_totals();
        let task_maps = snapshot.task_maps_from_counts(running_issues.len(), retrying_issues.len());
        let state_summary = StateSnapshot::summary_from_parts(
            &issue_totals,
            &task_maps,
            &running_issues,
            &retrying_issues,
        );
        let running = running_issues
            .into_iter()
            .map(RunningIssueView::from)
            .collect::<Vec<_>>();
        let retrying = retrying_issues
            .into_iter()
            .map(RetryIssueView::from)
            .collect::<Vec<_>>();

        Self {
            generated_at: now_iso8601_utc(),
            counts: StateCountsView {
                running: snapshot.runtime.running,
                retrying: snapshot.runtime.retrying,
            },
            running,
            retrying,
            codex_totals: CodexTotalsView::from(&snapshot.runtime),
            rate_limits: snapshot.runtime.rate_limits.clone(),
            activity: RuntimeActivityView::from(&snapshot.runtime),
            health: runtime_spec.health,
            issue_totals,
            task_maps,
            summary: StateOperatorSummaryView {
                runtime: runtime_spec.summary,
                state: state_summary,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceView {
    pub path: Option<String>,
}

impl WorkspaceView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptsView {
    pub restart_count: u32,
    pub current_retry_attempt: u32,
}

impl AttemptsView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "restart_count": self.restart_count,
            "current_retry_attempt": self.current_retry_attempt,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueLogsView {
    pub codex_session_logs: Vec<serde_json::Value>,
}

impl IssueLogsView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "codex_session_logs": self.codex_session_logs,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentEventView {
    pub at: String,
    pub event: Option<String>,
    pub message: Option<String>,
}

impl RecentEventView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "at": self.at,
            "event": self.event,
            "message": self.message,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueRunningDetailView {
    pub session_id: Option<String>,
    pub turn_count: u32,
    pub state: String,
    pub started_at: Option<String>,
    pub last_event: Option<String>,
    pub last_message: Option<String>,
    pub last_event_at: Option<String>,
    pub tokens: UsageView,
}

impl IssueRunningDetailView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "session_id": self.session_id,
            "turn_count": self.turn_count,
            "state": self.state,
            "started_at": self.started_at,
            "last_event": self.last_event,
            "last_message": self.last_message,
            "last_event_at": self.last_event_at,
            "tokens": self.tokens.to_json(),
        })
    }
}

impl From<&IssueSnapshot> for IssueRunningDetailView {
    fn from(issue: &IssueSnapshot) -> Self {
        Self {
            session_id: issue.session_id.clone(),
            turn_count: issue.turn_count,
            state: issue.state.clone(),
            started_at: iso8601(issue.started_at),
            last_event: issue.last_event.clone(),
            last_message: summarize_codex_message(
                issue.last_event.as_deref(),
                issue.last_message.as_deref(),
            ),
            last_event_at: iso8601(issue.last_event_at),
            tokens: UsageView {
                input_tokens: issue.input_tokens,
                output_tokens: issue.output_tokens,
                total_tokens: issue.total_tokens,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueRetryDetailView {
    pub attempt: u32,
    pub due_at: Option<String>,
    pub error: Option<String>,
}

impl IssueRetryDetailView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "attempt": self.attempt,
            "due_at": self.due_at,
            "error": self.error,
        })
    }
}

impl From<&IssueSnapshot> for IssueRetryDetailView {
    fn from(issue: &IssueSnapshot) -> Self {
        Self {
            attempt: issue.retry_attempts,
            due_at: iso8601(issue.retry_due_at),
            error: issue.retry_error.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueApiView {
    pub issue_identifier: String,
    pub issue_id: String,
    pub status: String,
    pub workspace: WorkspaceView,
    pub attempts: AttemptsView,
    pub running: Option<IssueRunningDetailView>,
    pub retry: Option<IssueRetryDetailView>,
    pub logs: IssueLogsView,
    pub recent_events: Vec<RecentEventView>,
    pub last_error: Option<String>,
    pub tracked: serde_json::Value,
}

impl IssueApiView {
    pub fn to_json(&self) -> serde_json::Value {
        let running = self.running.as_ref().map(IssueRunningDetailView::to_json);
        let retry = self.retry.as_ref().map(IssueRetryDetailView::to_json);
        let recent_events = self
            .recent_events
            .iter()
            .map(RecentEventView::to_json)
            .collect::<Vec<_>>();

        serde_json::json!({
            "issue_identifier": self.issue_identifier,
            "issue_id": self.issue_id,
            "status": self.status,
            "workspace": self.workspace.to_json(),
            "attempts": self.attempts.to_json(),
            "running": running,
            "retry": retry,
            "logs": self.logs.to_json(),
            "recent_events": recent_events,
            "last_error": self.last_error,
            "tracked": self.tracked,
        })
    }

    pub fn from_issue(issue: &IssueSnapshot, workspace_root: Option<&Path>) -> Self {
        let issue = sanitize_single_issue(issue);
        let is_retrying = issue.retry_attempts > 0;
        let running = (!is_retrying).then(|| IssueRunningDetailView::from(&issue));
        let retry = is_retrying.then(|| IssueRetryDetailView::from(&issue));
        let recent_events = issue
            .last_event_at
            .and_then(|timestamp| iso8601(Some(timestamp)))
            .map(|at| RecentEventView {
                at,
                event: issue.last_event.clone(),
                message: summarize_codex_message(
                    issue.last_event.as_deref(),
                    issue.last_message.as_deref(),
                ),
            })
            .into_iter()
            .collect::<Vec<_>>();
        let current_retry_attempt = issue.retry_attempts;

        Self {
            issue_identifier: issue.identifier.clone(),
            issue_id: issue.id.0.clone(),
            status: if is_retrying {
                "retrying".to_owned()
            } else {
                "running".to_owned()
            },
            workspace: WorkspaceView {
                path: issue.workspace_path.clone().or_else(|| {
                    workspace_root.and_then(|root| {
                        workspace_path(root, &issue.identifier)
                            .ok()
                            .map(|path| path.to_string_lossy().into_owned())
                    })
                }),
            },
            attempts: AttemptsView {
                restart_count: current_retry_attempt.saturating_sub(1),
                current_retry_attempt,
            },
            running,
            retry,
            logs: IssueLogsView {
                codex_session_logs: Vec::new(),
            },
            recent_events,
            last_error: issue.retry_error.clone(),
            tracked: serde_json::json!({}),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshAcceptedView {
    pub queued: bool,
    pub coalesced: bool,
    pub requested_at: String,
    pub operations: Vec<&'static str>,
}

impl RefreshAcceptedView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "queued": self.queued,
            "coalesced": self.coalesced,
            "requested_at": self.requested_at,
            "operations": self.operations,
        })
    }
}

impl Default for RefreshAcceptedView {
    fn default() -> Self {
        Self {
            queued: true,
            coalesced: false,
            requested_at: now_iso8601_utc(),
            operations: vec!["poll", "reconcile"],
        }
    }
}

pub fn now_iso8601_utc() -> String {
    let now = DateTime::<Utc>::from(SystemTime::now());
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn normalize_operator_float(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn iso8601(timestamp_secs: Option<u64>) -> Option<String> {
    let seconds = timestamp_secs?;
    let timestamp = DateTime::<Utc>::from_timestamp(seconds as i64, 0)?;
    Some(timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn summarize_codex_message(event: Option<&str>, message: Option<&str>) -> Option<String> {
    let sanitized = message.map(sanitize_summary_text);
    let trimmed = sanitized
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let parsed = trimmed.and_then(parse_summary_value);
    let summary = humanize_codex_event(event, parsed.as_ref())
        .or_else(|| humanize_codex_payload(parsed.as_ref()))
        .or_else(|| parsed.as_ref().map(format_json_compact_sorted))
        .or_else(|| trimmed.map(inline_text))
        .or_else(|| humanize_event_label(event))?;

    Some(truncate_message(&summary, 140))
}

fn humanize_codex_event(event: Option<&str>, message: Option<&Value>) -> Option<String> {
    let normalized = normalize_key(event)?;
    match normalized.as_str() {
        "session_new" => Some(match extract_session_identifier(message) {
            Some(session_id) => format!("session started ({session_id})"),
            None => "session started".to_owned(),
        }),
        "session_started" => Some(match map_string(message, &["session_id"]) {
            Some(session_id) => format!("session started ({session_id})"),
            None => "session started".to_owned(),
        }),
        "notification" => humanize_notification_payload(message),
        "turn_input_required" => Some("turn blocked: waiting for user input".to_owned()),
        "turn_approval_required" => Some(format_approval_required(message)),
        "approval_auto_approved" => {
            let method = map_string(message, &["method"])
                .or_else(|| map_nested_string(message, &["payload", "method"]));
            let decision = map_string(message, &["decision"]);
            let base = method
                .as_deref()
                .and_then(|method| humanize_codex_method(method, message))
                .map(|text| format!("{text} (auto-approved)"))
                .unwrap_or_else(|| "approval request auto-approved".to_owned());
            Some(match decision {
                Some(decision) => format!("{base}: {}", inline_text(&decision)),
                None => base,
            })
        }
        "tool_input_auto_answered" => {
            let answer = map_string(message, &["answer"]);
            let base = "tool input auto-answered".to_owned();
            Some(match answer {
                Some(answer) => format!("{base}: {}", inline_text(&answer)),
                None => base,
            })
        }
        "tool_call_completed" => Some(humanize_dynamic_tool_event(
            "dynamic tool call completed",
            message,
        )),
        "tool_call_failed" => Some(humanize_dynamic_tool_event(
            "dynamic tool call failed",
            message,
        )),
        "unsupported_tool_call" => Some(humanize_dynamic_tool_event(
            "unsupported dynamic tool call rejected",
            message,
        )),
        "turn_failed" => Some(format_reason_prefix("turn failed", message)),
        "turn_cancelled" => Some("turn cancelled".to_owned()),
        "turn_completed" => Some("turn completed".to_owned()),
        "approval_required" | "approval_requested" => Some(format_approval_required(message)),
        "startup_failed" => Some(format_reason_prefix("startup failed", message)),
        "turn_ended_with_error" => Some(format_reason_prefix("turn ended with error", message)),
        _ => None,
    }
}

fn humanize_codex_payload(message: Option<&Value>) -> Option<String> {
    let message = message?;
    if let Some(summary) = humanize_notification_payload(Some(message)) {
        return Some(summary);
    }

    if let Some(method) = map_string(Some(message), &["method"]) {
        return humanize_codex_method(&method, Some(message));
    }

    if let Some(session_id) = extract_session_identifier(Some(message)) {
        return Some(format!("session started ({session_id})"));
    }

    if let Some(error_summary) = humanize_structured_error(Some(message)) {
        return Some(error_summary);
    }

    if let Some(reason) = map_string(Some(message), &["reason"]) {
        return Some(inline_text(&reason));
    }

    if let Some(notification) = map_string(Some(message), &["message"])
        .or_else(|| map_nested_string(Some(message), &["payload", "message"]))
        .or_else(|| map_string(Some(message), &["content"]))
        .or_else(|| map_nested_string(Some(message), &["payload", "content"]))
    {
        return Some(inline_text(&notification));
    }

    Some(format_json_compact_sorted(message))
}

fn humanize_codex_method(method: &str, message: Option<&Value>) -> Option<String> {
    let normalized = normalize_key(Some(method))?;
    match normalized.as_str() {
        "initialize" => Some("initializing runtime session".to_owned()),
        "initialized" => Some("runtime session initialized".to_owned()),
        "session_new" => Some(match extract_session_identifier(message) {
            Some(session_id) => format!("session started ({session_id})"),
            None => "session started".to_owned(),
        }),
        "item_tool_requestuserinput" => Some("tool input request".to_owned()),
        "item_tool_call" => Some(humanize_dynamic_tool_event("dynamic tool call", message)),
        "thread_start" => Some("thread started".to_owned()),
        "turn_start" => Some("turn started".to_owned()),
        "turn_completed" => Some("turn completed".to_owned()),
        "turn_end" | "turn_ended" => Some("turn ended".to_owned()),
        "turn_approval_required" | "approval_requested" | "approval_required" => {
            Some(format_approval_required(message))
        }
        "turn_input_required" | "input_required" => {
            Some("turn blocked: waiting for user input".to_owned())
        }
        "worker_started" => Some("worker started".to_owned()),
        "worker_stopped" => Some("worker stopped".to_owned()),
        "unsupported_tool_call" => Some(humanize_dynamic_tool_event(
            "unsupported dynamic tool call rejected",
            message,
        )),
        "notification" => humanize_notification_payload(message),
        "turn_failed" => Some(format_reason_prefix("turn failed", message)),
        _ => {
            let tool_name = map_string(message, &["name"])
                .or_else(|| map_nested_string(message, &["payload", "name"]))
                .or_else(|| map_nested_string(message, &["output", "tool_name"]));
            if normalized.contains("tool_call") {
                return Some(match tool_name {
                    Some(tool_name) => format!("dynamic tool call: {}", inline_text(&tool_name)),
                    None => "dynamic tool call".to_owned(),
                });
            }

            Some(normalized.replace('_', " "))
        }
    }
}

fn humanize_event_label(event: Option<&str>) -> Option<String> {
    let normalized = normalize_key(event)?;
    Some(match normalized.as_str() {
        "worker_started" => "worker started".to_owned(),
        "worker_stopped" => "worker stopped".to_owned(),
        "session_new" => "session started".to_owned(),
        "thread_start" => "thread started".to_owned(),
        "turn_start" => "turn started".to_owned(),
        "turn_end" => "turn ended".to_owned(),
        "turn_completed" => "turn completed".to_owned(),
        "turn_cancelled" => "turn cancelled".to_owned(),
        "turn_failed" => "turn failed".to_owned(),
        "approval_required" => "approval required".to_owned(),
        "approval_requested" => "approval required".to_owned(),
        "turn_approval_required" => "approval required".to_owned(),
        "approval_auto_approved" => "approval request auto-approved".to_owned(),
        "tool_call_completed" => "dynamic tool call completed".to_owned(),
        "tool_call_failed" => "dynamic tool call failed".to_owned(),
        "unsupported_tool_call" => "unsupported dynamic tool call rejected".to_owned(),
        other => other.replace('_', " "),
    })
}

fn humanize_dynamic_tool_event(prefix: &str, message: Option<&Value>) -> String {
    let tool_name = map_string(message, &["name"])
        .or_else(|| map_nested_string(message, &["payload", "name"]))
        .or_else(|| map_nested_string(message, &["output", "tool_name"]));
    let base = match tool_name {
        Some(name) => format!("{prefix}: {}", inline_text(&name)),
        None => prefix.to_owned(),
    };

    match humanize_structured_error(message) {
        Some(detail) if !base.contains(&detail) => format!("{base} ({detail})"),
        _ => base,
    }
}

fn humanize_notification_payload(message: Option<&Value>) -> Option<String> {
    let message = message?;

    if contains_truthy_flag(message, &["approval_required", "requires_approval"])
        || contains_string_marker(
            message,
            &[
                "approval_required",
                "approval_requested",
                "approval/requested",
                "approval/required",
                "turn/approval_required",
            ],
        )
    {
        return Some(format_approval_required(Some(message)));
    }

    if contains_truthy_flag(message, &["input_required", "requires_input"])
        || contains_string_marker(
            message,
            &[
                "turn_input_required",
                "input_required",
                "turn/input_required",
                "item/tool/requestUserInput",
            ],
        )
    {
        return Some("turn blocked: waiting for user input".to_owned());
    }

    if let Some(error_summary) = humanize_structured_error(Some(message)) {
        return Some(error_summary);
    }

    None
}

fn humanize_structured_error(message: Option<&Value>) -> Option<String> {
    let code = extract_error_code(message)?;
    let label = humanize_error_code(&code);
    let detail = extract_error_detail(message);

    Some(match detail {
        Some(detail) if detail != label => format!("{label}: {}", inline_text(&detail)),
        _ => label,
    })
}

fn format_approval_required(message: Option<&Value>) -> String {
    match map_string(message, &["command"])
        .or_else(|| map_nested_string(message, &["payload", "command"]))
        .or_else(|| map_string(message, &["tool_name"]))
        .or_else(|| map_nested_string(message, &["output", "tool_name"]))
    {
        Some(command) => format!("approval required: {}", inline_text(&command)),
        None => "approval required".to_owned(),
    }
}

fn extract_session_identifier(message: Option<&Value>) -> Option<String> {
    map_string(message, &["session_id"])
        .or_else(|| map_nested_string(message, &["session", "id"]))
        .or_else(|| {
            Some(build_session_label(
                map_nested_string(message, &["thread", "id"])
                    .or_else(|| map_nested_string(message, &["thread", "thread_id"]))
                    .or_else(|| map_string(message, &["thread_id"])),
                map_nested_string(message, &["turn", "id"])
                    .or_else(|| map_nested_string(message, &["turn", "turn_id"]))
                    .or_else(|| map_string(message, &["turn_id"])),
            ))
            .filter(|value| !value.is_empty())
        })
}

fn build_session_label(thread_id: Option<String>, turn_id: Option<String>) -> String {
    match (thread_id, turn_id) {
        (Some(thread_id), Some(turn_id)) => format!("{thread_id}/{turn_id}"),
        (Some(thread_id), None) => thread_id,
        (None, Some(turn_id)) => turn_id,
        (None, None) => String::new(),
    }
}

fn extract_error_code(message: Option<&Value>) -> Option<String> {
    map_nested_string(message, &["error", "code"])
        .or_else(|| map_string(message, &["error"]))
        .or_else(|| map_string(message, &["code"]))
        .or_else(|| map_string(message, &["reason"]))
        .or_else(|| map_nested_string(message, &["details", "reason"]))
}

fn extract_error_detail(message: Option<&Value>) -> Option<String> {
    map_nested_string(message, &["output", "message"])
        .or_else(|| map_nested_string(message, &["error", "message"]))
        .or_else(|| map_string(message, &["message"]))
        .or_else(|| map_nested_string(message, &["details", "message"]))
        .or_else(|| map_first_array_object_string(message, &["output", "errors"], "message"))
        .or_else(|| {
            map_first_array_object_string(message, &["output", "body", "errors"], "message")
        })
        .or_else(|| map_string(message, &["status"]).map(|status| format!("status {status}")))
        .or_else(|| {
            map_nested_string(message, &["output", "status"])
                .map(|status| format!("status {status}"))
        })
        .or_else(|| map_nested_string(message, &["output", "tracker_kind"]))
}

fn humanize_error_code(code: &str) -> String {
    match normalize_key(Some(code)).as_deref() {
        Some("unsupported_tool_call") => "unsupported dynamic tool call rejected".to_owned(),
        Some("linear_tracker_not_configured") => "linear tracker not configured".to_owned(),
        Some("missing_linear_api_key") => "missing linear api key".to_owned(),
        Some("invalid_tool_input") => "invalid tool input".to_owned(),
        Some("client_build_failure") => "tool client build failure".to_owned(),
        Some("transport_failure") => "transport failure".to_owned(),
        Some("response_error") => "response error".to_owned(),
        Some("graphql_error") => "graphql error".to_owned(),
        Some("codex_not_found") => "codex executable not found".to_owned(),
        Some("invalid_workspace_cwd") => "invalid workspace cwd".to_owned(),
        Some("port_exit") => "port process exited".to_owned(),
        Some("turn_timeout") => "turn timed out".to_owned(),
        Some("turn_failed") => "turn failed".to_owned(),
        Some(
            "read_timeout" | "startup_timeout" | "handshake_timeout" | "response_timeout"
            | "response_timed_out",
        ) => "response timeout".to_owned(),
        Some(other) => other.replace('_', " "),
        None => inline_text(code),
    }
}

fn contains_truthy_flag(value: &Value, keys: &[&str]) -> bool {
    let normalized_keys = keys
        .iter()
        .filter_map(|key| normalize_key(Some(key)))
        .collect::<Vec<_>>();
    contains_truthy_flag_with_keys(value, &normalized_keys)
}

fn contains_truthy_flag_with_keys(value: &Value, keys: &[String]) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, nested)| {
            let normalized_key = normalize_key(Some(key)).unwrap_or_default();
            (keys.contains(&normalized_key) && nested.as_bool() == Some(true))
                || contains_truthy_flag_with_keys(nested, keys)
        }),
        Value::Array(values) => values
            .iter()
            .any(|nested| contains_truthy_flag_with_keys(nested, keys)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn contains_string_marker(value: &Value, markers: &[&str]) -> bool {
    let normalized_markers = markers
        .iter()
        .filter_map(|marker| normalize_key(Some(marker)))
        .collect::<Vec<_>>();
    contains_string_marker_with_markers(value, &normalized_markers)
}

fn contains_string_marker_with_markers(value: &Value, markers: &[String]) -> bool {
    match value {
        Value::Object(map) => map
            .values()
            .any(|nested| contains_string_marker_with_markers(nested, markers)),
        Value::Array(values) => values
            .iter()
            .any(|nested| contains_string_marker_with_markers(nested, markers)),
        Value::String(raw) => {
            normalize_key(Some(raw)).is_some_and(|normalized| markers.contains(&normalized))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn map_first_array_object_string(
    value: Option<&Value>,
    path: &[&str],
    field: &str,
) -> Option<String> {
    let mut current = value?;
    for segment in path {
        current = current.get(*segment)?;
    }

    current.as_array()?.iter().find_map(|item| {
        item.get(field).and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(boolean) => Some(boolean.to_string()),
            _ => None,
        })
    })
}

fn parse_summary_value(message: &str) -> Option<Value> {
    if !matches!(message.chars().next(), Some('{' | '[')) {
        return None;
    }

    serde_json::from_str::<Value>(message)
        .ok()
        .map(|value| sanitize_json_value(&value))
}

fn sanitize_single_issue(issue: &IssueSnapshot) -> IssueSnapshot {
    StateSnapshot {
        runtime: RuntimeSnapshot::default(),
        issues: vec![issue.clone()],
    }
    .sanitized()
    .issues
    .into_iter()
    .next()
    .expect("single issue sanitization should preserve one issue")
}

fn format_reason_prefix(prefix: &str, message: Option<&Value>) -> String {
    match extract_reason(message) {
        Some(reason) => format!("{prefix}: {}", inline_text(&reason)),
        None => prefix.to_owned(),
    }
}

fn extract_reason(message: Option<&Value>) -> Option<String> {
    map_string(message, &["reason"])
        .or_else(|| map_string(message, &["message"]))
        .or_else(|| map_nested_string(message, &["error", "message"]))
        .or_else(|| map_nested_string(message, &["error", "code"]))
}

fn map_string(value: Option<&Value>, path: &[&str]) -> Option<String> {
    let mut current = value?;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn map_nested_string(value: Option<&Value>, path: &[&str]) -> Option<String> {
    map_string(value, path)
}

fn normalize_key(value: Option<&str>) -> Option<String> {
    let value = value?;
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('_');
        }
    }

    while normalized.contains("__") {
        normalized = normalized.replace("__", "_");
    }

    let normalized = normalized.trim_matches('_').to_owned();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_message(message: &str, max_chars: usize) -> String {
    let mut truncated = String::new();
    let mut chars = message.chars();
    for _ in 0..max_chars {
        let Some(ch) = chars.next() else {
            return message.to_owned();
        };
        truncated.push(ch);
    }

    if chars.next().is_some() {
        truncated.push_str("...");
    }

    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        CodexTotalsView, ThroughputView, normalize_operator_float, summarize_codex_message,
    };
    use symphony_observability::{RuntimeActivitySnapshot, RuntimeSnapshot, ThroughputSnapshot};

    #[test]
    fn summarizes_session_started_messages() {
        let message = summarize_codex_message(
            Some("session_started"),
            Some(r#"{"session_id":"session-123"}"#),
        );

        assert_eq!(message.as_deref(), Some("session started (session-123)"));
    }

    #[test]
    fn summarizes_input_required_and_failure_events() {
        let input_required = summarize_codex_message(
            Some("turn_input_required"),
            Some(r#"{"payload":{"method":"item/tool/requestUserInput"}}"#),
        );
        let startup_failed = summarize_codex_message(
            Some("startup_failed"),
            Some(r#"{"reason":"response_timeout"}"#),
        );

        assert_eq!(
            input_required.as_deref(),
            Some("turn blocked: waiting for user input")
        );
        assert_eq!(
            startup_failed.as_deref(),
            Some("startup failed: response_timeout")
        );
    }

    #[test]
    fn summarizes_auto_approved_tool_input_requests() {
        let message = summarize_codex_message(
            Some("approval_auto_approved"),
            Some(r#"{"method":"item/tool/requestUserInput","decision":"allow"}"#),
        );

        assert_eq!(
            message.as_deref(),
            Some("tool input request (auto-approved): allow")
        );
    }

    #[test]
    fn humanizes_event_only_updates_without_raw_json_payloads() {
        assert_eq!(
            summarize_codex_message(Some("worker/started"), None).as_deref(),
            Some("worker started")
        );
        assert_eq!(
            summarize_codex_message(Some("turn/completed"), None).as_deref(),
            Some("turn completed")
        );
    }

    #[test]
    fn preserves_plain_text_turn_updates_when_present() {
        let message = summarize_codex_message(Some("turn/start"), Some("turn 2 started"));
        assert_eq!(message.as_deref(), Some("turn 2 started"));
    }

    #[test]
    fn humanizes_approval_required_commands() {
        let message = summarize_codex_message(
            Some("approval_required"),
            Some(r#"{"command":"gh pr view"}"#),
        );

        assert_eq!(message.as_deref(), Some("approval required: gh pr view"));
    }

    #[test]
    fn humanizes_protocol_method_aliases_and_session_variants() {
        let session_new = summarize_codex_message(
            None,
            Some(r#"{"method":"session/new","thread":{"id":"thread-1"},"turn":{"id":"turn-2"}}"#),
        );
        let approval = summarize_codex_message(
            None,
            Some(r#"{"method":"turn/approval_required","command":"git push origin"}"#),
        );
        let input_required =
            summarize_codex_message(None, Some(r#"{"method":"turn/input_required"}"#));

        assert_eq!(
            session_new.as_deref(),
            Some("session started (thread-1/turn-2)")
        );
        assert_eq!(
            approval.as_deref(),
            Some("approval required: git push origin")
        );
        assert_eq!(
            input_required.as_deref(),
            Some("turn blocked: waiting for user input")
        );
    }

    #[test]
    fn humanizes_notification_flags_and_structured_errors() {
        let approval = summarize_codex_message(
            Some("notification"),
            Some(r#"{"event":{"approval_required":true}}"#),
        );
        let graphql_error = summarize_codex_message(
            Some("notification"),
            Some(
                r#"{"error":"graphql_error","output":{"errors":[{"message":"permission denied"}]}}"#,
            ),
        );

        assert_eq!(approval.as_deref(), Some("approval required"));
        assert_eq!(
            graphql_error.as_deref(),
            Some("graphql error: permission denied")
        );
    }

    #[test]
    fn humanizes_dynamic_tool_failures_with_error_detail() {
        let message = summarize_codex_message(
            Some("tool_call_failed"),
            Some(
                r#"{"name":"linear_graphql","error":"transport_failure","output":{"message":"connection refused"}}"#,
            ),
        );

        assert_eq!(
            message.as_deref(),
            Some(
                "dynamic tool call failed: linear_graphql (transport failure: connection refused)"
            )
        );
    }

    #[test]
    fn falls_back_to_inline_text_for_plain_messages() {
        let message = summarize_codex_message(None, Some("plain\ntext"));
        assert_eq!(message.as_deref(), Some("plain text"));
    }

    #[test]
    fn falls_back_to_sorted_redacted_json_for_structured_messages() {
        let message = summarize_codex_message(
            Some("notification"),
            Some(r#"{"zeta":1,"authorization":"Bearer abc","alpha":{"token":"xyz","ok":"yes"}}"#),
        );

        assert_eq!(
            message.as_deref(),
            Some(
                r#"{"alpha":{"ok":"yes","token":"[REDACTED]"},"authorization":"[REDACTED]","zeta":1}"#
            )
        );
    }

    #[test]
    fn humanizes_notification_payload_message_fields() {
        let message = summarize_codex_message(
            Some("notification"),
            Some(r#"{"payload":{"message":"syncing tracker state"}}"#),
        );

        assert_eq!(message.as_deref(), Some("syncing tracker state"));
    }

    #[test]
    fn normalizes_negative_zero_operator_floats() {
        let totals = CodexTotalsView::from(&RuntimeSnapshot {
            running: 0,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: -0.0,
            rate_limits: None,
            activity: RuntimeActivitySnapshot {
                throughput: ThroughputSnapshot {
                    window_seconds: -0.0,
                    input_tokens_per_second: -0.0,
                    output_tokens_per_second: -0.0,
                    total_tokens_per_second: -0.0,
                },
                ..RuntimeActivitySnapshot::default()
            },
        });

        assert_eq!(totals.seconds_running, 0.0);
        assert_eq!(normalize_operator_float(-0.0), 0.0);

        let throughput = ThroughputView {
            window_seconds: normalize_operator_float(-0.0),
            input_tokens_per_second: normalize_operator_float(-0.0),
            output_tokens_per_second: normalize_operator_float(-0.0),
            total_tokens_per_second: normalize_operator_float(-0.0),
        };
        assert_eq!(throughput.window_seconds, 0.0);
        assert_eq!(throughput.total_tokens_per_second, 0.0);
    }
}
