use serde::{Deserialize, Serialize};

use crate::{IssueSnapshot, IssueTaskMapKind, RuntimeSnapshot, RuntimeSpecView};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    #[default]
    Live,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotErrorCode {
    Timeout,
    Unavailable,
}

impl SnapshotErrorCode {
    #[must_use]
    pub const fn api_code(self) -> &'static str {
        match self {
            Self::Timeout => "snapshot_timeout",
            Self::Unavailable => "snapshot_unavailable",
        }
    }

    #[must_use]
    pub const fn default_message(self) -> &'static str {
        match self {
            Self::Timeout => "Snapshot timed out",
            Self::Unavailable => "Snapshot unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotErrorView {
    pub code: SnapshotErrorCode,
    pub message: String,
}

impl SnapshotErrorView {
    #[must_use]
    pub fn new(code: SnapshotErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn timeout() -> Self {
        Self::new(
            SnapshotErrorCode::Timeout,
            SnapshotErrorCode::Timeout.default_message(),
        )
    }

    #[must_use]
    pub fn unavailable() -> Self {
        Self::new(
            SnapshotErrorCode::Unavailable,
            SnapshotErrorCode::Unavailable.default_message(),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub runtime: RuntimeSnapshot,
    #[serde(default)]
    pub issues: Vec<IssueSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshotEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<StateSnapshot>,
    #[serde(default)]
    pub status: SnapshotStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SnapshotErrorView>,
}

impl StateSnapshotEnvelope {
    #[must_use]
    pub fn ready(snapshot: StateSnapshot) -> Self {
        Self {
            snapshot: Some(snapshot),
            status: SnapshotStatus::Live,
            error: None,
        }
    }

    #[must_use]
    pub fn stale(snapshot: StateSnapshot, error: SnapshotErrorView) -> Self {
        Self {
            snapshot: Some(snapshot),
            status: SnapshotStatus::Stale,
            error: Some(error),
        }
    }

    #[must_use]
    pub fn timeout() -> Self {
        Self {
            snapshot: None,
            status: SnapshotStatus::Live,
            error: Some(SnapshotErrorView::timeout()),
        }
    }

    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            snapshot: None,
            status: SnapshotStatus::Live,
            error: Some(SnapshotErrorView::unavailable()),
        }
    }

    #[must_use]
    pub fn from_error(error: SnapshotErrorView) -> Self {
        Self {
            snapshot: None,
            status: SnapshotStatus::Live,
            error: Some(error),
        }
    }

    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self.status, SnapshotStatus::Live) && self.error.is_none()
    }

    #[must_use]
    pub const fn is_stale(&self) -> bool {
        matches!(self.status, SnapshotStatus::Stale)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueStatusTotalsSnapshot {
    pub total: usize,
    pub active: usize,
    pub terminal: usize,
    pub running: usize,
    pub retrying: usize,
    pub completed: usize,
    pub failed: usize,
    pub canceled: usize,
    pub pending: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMapSnapshot {
    pub running_rows: usize,
    pub retrying_rows: usize,
    pub inactive_tracked: usize,
    pub runtime_running_gap: isize,
    pub runtime_retrying_gap: isize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateSpecView {
    pub runtime: RuntimeSpecView,
    pub issue_totals: IssueStatusTotalsSnapshot,
    pub task_maps: TaskMapSnapshot,
}

impl StateSnapshot {
    pub fn issue(&self, issue_id: &str) -> Option<&IssueSnapshot> {
        self.issues.iter().find(|issue| issue.id.0 == issue_id)
    }

    pub fn issue_by_identifier(&self, issue_identifier: &str) -> Option<&IssueSnapshot> {
        self.issues
            .iter()
            .find(|issue| issue.identifier == issue_identifier)
    }

    pub fn issue_by_id_or_identifier(&self, issue_key: &str) -> Option<&IssueSnapshot> {
        self.issue(issue_key)
            .or_else(|| self.issue_by_identifier(issue_key))
    }

    #[must_use]
    pub fn running_issues(&self) -> Vec<&IssueSnapshot> {
        self.issues
            .iter()
            .filter(|issue| issue.is_running())
            .collect()
    }

    #[must_use]
    pub fn retrying_issues(&self) -> Vec<&IssueSnapshot> {
        self.issues
            .iter()
            .filter(|issue| issue.is_retrying())
            .collect()
    }

    #[must_use]
    pub fn issue_totals(&self) -> IssueStatusTotalsSnapshot {
        self.issues
            .iter()
            .fold(IssueStatusTotalsSnapshot::default(), |mut totals, issue| {
                totals.total += 1;

                match issue.task_map_kind() {
                    IssueTaskMapKind::Running => totals.running += 1,
                    IssueTaskMapKind::Retrying => totals.retrying += 1,
                    IssueTaskMapKind::Completed => totals.completed += 1,
                    IssueTaskMapKind::Failed => totals.failed += 1,
                    IssueTaskMapKind::Canceled => totals.canceled += 1,
                    IssueTaskMapKind::Pending => totals.pending += 1,
                    IssueTaskMapKind::Unknown => totals.unknown += 1,
                }

                totals.active = totals.running + totals.retrying;
                totals.terminal = totals.completed + totals.failed + totals.canceled;
                totals
            })
    }

    #[must_use]
    pub fn task_maps(&self) -> TaskMapSnapshot {
        let running_rows = self.running_issues().len();
        let retrying_rows = self.retrying_issues().len();
        let inactive_tracked = self
            .issues
            .len()
            .saturating_sub(running_rows.saturating_add(retrying_rows));

        TaskMapSnapshot {
            running_rows,
            retrying_rows,
            inactive_tracked,
            runtime_running_gap: self.runtime.running as isize - running_rows as isize,
            runtime_retrying_gap: self.runtime.retrying as isize - retrying_rows as isize,
        }
    }

    #[must_use]
    pub fn spec_view(&self) -> StateSpecView {
        StateSpecView {
            runtime: self.runtime.spec_view(),
            issue_totals: self.issue_totals(),
            task_maps: self.task_maps(),
        }
    }
}

#[cfg(test)]
mod tests {
    use symphony_domain::IssueId;

    use crate::{
        IssueSnapshot, RuntimeSnapshot, SnapshotErrorCode, SnapshotErrorView, SnapshotStatus,
        StateSnapshot, StateSnapshotEnvelope,
    };

    #[test]
    fn issue_totals_group_known_states() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 2,
                retrying: 1,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                seconds_running: 0.0,
                rate_limits: None,
                activity: crate::RuntimeActivitySnapshot::default(),
            },
            issues: vec![
                IssueSnapshot {
                    id: IssueId("SYM-1".to_owned()),
                    identifier: "SYM-1".to_owned(),
                    state: "Running".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-2".to_owned()),
                    identifier: "SYM-2".to_owned(),
                    state: "retry".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-3".to_owned()),
                    identifier: "SYM-3".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-4".to_owned()),
                    identifier: "SYM-4".to_owned(),
                    state: "Failed".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-5".to_owned()),
                    identifier: "SYM-5".to_owned(),
                    state: "unknown-state".to_owned(),
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
                },
            ],
        };

        let totals = snapshot.issue_totals();
        assert_eq!(totals.total, 5);
        assert_eq!(totals.active, 2);
        assert_eq!(totals.terminal, 2);
        assert_eq!(totals.running, 1);
        assert_eq!(totals.retrying, 1);
        assert_eq!(totals.completed, 1);
        assert_eq!(totals.failed, 1);
        assert_eq!(totals.canceled, 0);
        assert_eq!(totals.pending, 0);
        assert_eq!(totals.unknown, 1);
    }

    #[test]
    fn state_spec_view_uses_runtime_and_issue_totals() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 1,
                retrying: 2,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                seconds_running: 0.0,
                rate_limits: None,
                activity: crate::RuntimeActivitySnapshot::default(),
            },
            issues: vec![
                IssueSnapshot {
                    id: IssueId("SYM-1".to_owned()),
                    identifier: "SYM-1".to_owned(),
                    state: "Running".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-2".to_owned()),
                    identifier: "SYM-2".to_owned(),
                    state: "Retrying".to_owned(),
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
                },
                IssueSnapshot {
                    id: IssueId("SYM-3".to_owned()),
                    identifier: "SYM-3".to_owned(),
                    state: "Backlog".to_owned(),
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
                },
            ],
        };

        let spec_view = snapshot.spec_view();
        assert_eq!(spec_view.runtime.counts.running, 1);
        assert_eq!(spec_view.runtime.counts.retrying, 2);
        assert_eq!(spec_view.issue_totals.total, 3);
        assert_eq!(spec_view.issue_totals.pending, 1);
        assert_eq!(spec_view.task_maps.running_rows, 1);
        assert_eq!(spec_view.task_maps.retrying_rows, 1);
        assert_eq!(spec_view.task_maps.inactive_tracked, 1);
        assert_eq!(spec_view.task_maps.runtime_running_gap, 0);
        assert_eq!(spec_view.task_maps.runtime_retrying_gap, 1);
    }

    #[test]
    fn snapshot_error_codes_map_to_api_contract() {
        assert_eq!(SnapshotErrorCode::Timeout.api_code(), "snapshot_timeout");
        assert_eq!(
            SnapshotErrorCode::Unavailable.api_code(),
            "snapshot_unavailable"
        );
        assert_eq!(
            SnapshotErrorCode::Timeout.default_message(),
            "Snapshot timed out"
        );
        assert_eq!(
            SnapshotErrorCode::Unavailable.default_message(),
            "Snapshot unavailable"
        );
    }

    #[test]
    fn snapshot_envelope_represents_ready_stale_and_error_states() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot::default(),
            issues: Vec::new(),
        };

        let ready = StateSnapshotEnvelope::ready(snapshot.clone());
        assert!(ready.is_live());
        assert_eq!(ready.snapshot, Some(snapshot.clone()));
        assert!(ready.error.is_none());

        let stale = StateSnapshotEnvelope::stale(snapshot.clone(), SnapshotErrorView::timeout());
        assert!(stale.is_stale());
        assert_eq!(stale.status, SnapshotStatus::Stale);
        assert_eq!(
            stale.error,
            Some(SnapshotErrorView::new(
                SnapshotErrorCode::Timeout,
                "Snapshot timed out"
            ))
        );

        let timeout = StateSnapshotEnvelope::timeout();
        assert!(timeout.snapshot.is_none());
        assert_eq!(
            timeout.error,
            Some(SnapshotErrorView::new(
                SnapshotErrorCode::Timeout,
                "Snapshot timed out"
            ))
        );
    }
}
