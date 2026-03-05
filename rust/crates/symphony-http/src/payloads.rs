use std::time::{SystemTime, UNIX_EPOCH};

use symphony_observability::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

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

#[derive(Clone, Debug, PartialEq)]
pub struct TokenTotalsView {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

impl TokenTotalsView {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "total_tokens": self.total_tokens,
            "seconds_running": self.seconds_running,
        })
    }
}

impl Default for TokenTotalsView {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: 0.0,
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

#[derive(Clone, Debug, PartialEq)]
pub struct RunningIssueView {
    pub issue_id: String,
    pub issue_identifier: String,
    pub state: String,
    pub session_id: Option<String>,
    pub turn_count: u32,
    pub last_event: Option<String>,
    pub last_message: String,
    pub started_at: Option<String>,
    pub last_event_at: Option<String>,
    pub tokens: TokenTotalsView,
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
            session_id: None,
            turn_count: 0,
            last_event: None,
            last_message: String::new(),
            started_at: None,
            last_event_at: None,
            tokens: TokenTotalsView::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryIssueView {
    pub issue_id: String,
    pub issue_identifier: String,
    pub attempt: u32,
    pub due_at: Option<String>,
    pub error: String,
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
            due_at: None,
            error: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateApiView {
    pub generated_at: String,
    pub counts: StateCountsView,
    pub running: Vec<RunningIssueView>,
    pub retrying: Vec<RetryIssueView>,
    pub codex_totals: TokenTotalsView,
    pub rate_limits: Option<serde_json::Value>,
    pub runtime: RuntimeLegacyView,
    pub issues: Vec<IssueLegacyView>,
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
        let issues = self
            .issues
            .iter()
            .map(IssueLegacyView::to_json)
            .collect::<Vec<_>>();

        serde_json::json!({
            "generated_at": self.generated_at,
            "counts": self.counts.to_json(),
            "running": running,
            "retrying": retrying,
            "codex_totals": self.codex_totals.to_json(),
            "rate_limits": self.rate_limits,
            "runtime": self.runtime.to_json(),
            "issues": issues,
        })
    }
}

impl From<&StateSnapshot> for StateApiView {
    fn from(snapshot: &StateSnapshot) -> Self {
        let legacy_issues = snapshot
            .issues
            .iter()
            .map(IssueLegacyView::from)
            .collect::<Vec<_>>();
        let running = snapshot
            .issues
            .iter()
            .filter(|issue| issue.retry_attempts == 0)
            .map(RunningIssueView::from)
            .collect::<Vec<_>>();
        let retrying = snapshot
            .issues
            .iter()
            .filter(|issue| issue.retry_attempts > 0)
            .map(RetryIssueView::from)
            .collect::<Vec<_>>();

        Self {
            generated_at: now_rfc3339_utc(),
            counts: StateCountsView {
                running: snapshot.runtime.running,
                retrying: snapshot.runtime.retrying,
            },
            running,
            retrying,
            codex_totals: TokenTotalsView {
                input_tokens: snapshot.runtime.input_tokens,
                output_tokens: snapshot.runtime.output_tokens,
                total_tokens: snapshot.runtime.total_tokens,
                seconds_running: 0.0,
            },
            rate_limits: None,
            runtime: RuntimeLegacyView::from(&snapshot.runtime),
            issues: legacy_issues,
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
    pub at: Option<String>,
    pub event: Option<String>,
    pub message: String,
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

#[derive(Clone, Debug, PartialEq)]
pub struct IssueApiView {
    pub issue_identifier: String,
    pub issue_id: String,
    pub status: String,
    pub workspace: WorkspaceView,
    pub attempts: AttemptsView,
    pub running: Option<RunningIssueView>,
    pub retry: Option<RetryIssueView>,
    pub logs: IssueLogsView,
    pub recent_events: Vec<RecentEventView>,
    pub last_error: Option<String>,
    pub tracked: serde_json::Value,
    pub issue: IssueLegacyView,
}

impl IssueApiView {
    pub fn to_json(&self) -> serde_json::Value {
        let running = self.running.as_ref().map(RunningIssueView::to_json);
        let retry = self.retry.as_ref().map(RetryIssueView::to_json);
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
            "issue": self.issue.to_json(),
        })
    }
}

impl From<&IssueSnapshot> for IssueApiView {
    fn from(issue: &IssueSnapshot) -> Self {
        let status = if issue.retry_attempts > 0 {
            "retrying"
        } else {
            "running"
        };

        Self {
            issue_identifier: issue.identifier.clone(),
            issue_id: issue.id.0.clone(),
            status: status.to_owned(),
            workspace: WorkspaceView { path: None },
            attempts: AttemptsView {
                restart_count: 0,
                current_retry_attempt: issue.retry_attempts,
            },
            running: Some(RunningIssueView::from(issue)),
            retry: (issue.retry_attempts > 0).then(|| RetryIssueView::from(issue)),
            logs: IssueLogsView {
                codex_session_logs: Vec::new(),
            },
            recent_events: Vec::new(),
            last_error: None,
            tracked: serde_json::json!({}),
            issue: IssueLegacyView::from(issue),
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
            requested_at: now_rfc3339_utc(),
            operations: vec!["poll", "reconcile"],
        }
    }
}

fn now_rfc3339_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}
