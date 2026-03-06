use serde::{Deserialize, Serialize};
use symphony_domain::IssueId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSnapshot {
    pub id: IssueId,
    pub identifier: String,
    pub state: String,
    #[serde(default)]
    pub retry_attempts: u32,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub turn_count: u32,
    #[serde(default)]
    pub last_event: Option<String>,
    #[serde(default)]
    pub last_message: Option<String>,
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub last_event_at: Option<u64>,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub retry_due_at: Option<u64>,
    #[serde(default)]
    pub retry_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueTaskMapKind {
    Running,
    Retrying,
    Completed,
    Failed,
    Canceled,
    Pending,
    Unknown,
}

impl IssueTaskMapKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Retrying => "retrying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::Pending => "pending",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Retrying)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

impl IssueSnapshot {
    #[must_use]
    pub fn normalized_state_key(&self) -> Option<String> {
        normalize_state_key(&self.state)
    }

    #[must_use]
    pub fn task_map_kind(&self) -> IssueTaskMapKind {
        if self.retry_attempts > 0 {
            return IssueTaskMapKind::Retrying;
        }

        match self.normalized_state_key().as_deref() {
            Some("running" | "in progress" | "active" | "started" | "working") => {
                IssueTaskMapKind::Running
            }
            Some("retrying" | "retry" | "waiting to retry") => IssueTaskMapKind::Retrying,
            Some("completed" | "complete" | "done" | "succeeded" | "success") => {
                IssueTaskMapKind::Completed
            }
            Some("failed" | "error" | "errored") => IssueTaskMapKind::Failed,
            Some("canceled" | "cancelled") => IssueTaskMapKind::Canceled,
            Some("todo" | "queued" | "backlog" | "blocked" | "planned" | "ready") => {
                IssueTaskMapKind::Pending
            }
            Some(_) => IssueTaskMapKind::Unknown,
            None => IssueTaskMapKind::Unknown,
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.task_map_kind() == IssueTaskMapKind::Running
    }

    #[must_use]
    pub fn is_retrying(&self) -> bool {
        self.task_map_kind() == IssueTaskMapKind::Retrying
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.task_map_kind().is_terminal()
    }
}

fn normalize_state_key(state: &str) -> Option<String> {
    let mut normalized = String::new();
    for (index, token) in state.split_whitespace().enumerate() {
        if index > 0 {
            normalized.push(' ');
        }
        normalized.push_str(&token.to_ascii_lowercase());
    }

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::{IssueSnapshot, IssueTaskMapKind};
    use symphony_domain::IssueId;

    #[test]
    fn classifies_retry_attempts_as_retrying_even_when_tracker_state_is_running() {
        let issue = IssueSnapshot {
            id: IssueId("SYM-1".to_owned()),
            identifier: "SYM-1".to_owned(),
            state: "In Progress".to_owned(),
            retry_attempts: 2,
            workspace_path: None,
            session_id: None,
            turn_count: 0,
            last_event: None,
            last_message: None,
            started_at: None,
            last_event_at: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            retry_due_at: None,
            retry_error: None,
        };

        assert_eq!(issue.task_map_kind(), IssueTaskMapKind::Retrying);
        assert!(issue.is_retrying());
        assert_eq!(issue.normalized_state_key().as_deref(), Some("in progress"));
    }

    #[test]
    fn classifies_terminal_and_pending_states() {
        let completed = IssueSnapshot {
            id: IssueId("SYM-2".to_owned()),
            identifier: "SYM-2".to_owned(),
            state: "Done".to_owned(),
            retry_attempts: 0,
            workspace_path: None,
            session_id: None,
            turn_count: 0,
            last_event: None,
            last_message: None,
            started_at: None,
            last_event_at: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            retry_due_at: None,
            retry_error: None,
        };
        let pending = IssueSnapshot {
            id: IssueId("SYM-3".to_owned()),
            identifier: "SYM-3".to_owned(),
            state: "  Backlog  ".to_owned(),
            retry_attempts: 0,
            workspace_path: None,
            session_id: None,
            turn_count: 0,
            last_event: None,
            last_message: None,
            started_at: None,
            last_event_at: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            retry_due_at: None,
            retry_error: None,
        };

        assert_eq!(completed.task_map_kind(), IssueTaskMapKind::Completed);
        assert!(completed.is_terminal());
        assert_eq!(pending.task_map_kind(), IssueTaskMapKind::Pending);
        assert_eq!(pending.normalized_state_key().as_deref(), Some("backlog"));
    }
}
