use serde::{Deserialize, Serialize};

use crate::{IssueSnapshot, RuntimeSnapshot, RuntimeSpecView};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub runtime: RuntimeSnapshot,
    #[serde(default)]
    pub issues: Vec<IssueSnapshot>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueStatusTotalsSnapshot {
    pub total: usize,
    pub running: usize,
    pub retrying: usize,
    pub completed: usize,
    pub failed: usize,
    pub canceled: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateSpecView {
    pub runtime: RuntimeSpecView,
    pub issue_totals: IssueStatusTotalsSnapshot,
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
    pub fn issue_totals(&self) -> IssueStatusTotalsSnapshot {
        self.issues
            .iter()
            .fold(IssueStatusTotalsSnapshot::default(), |mut totals, issue| {
                totals.total += 1;

                match issue.state.trim().to_ascii_lowercase().as_str() {
                    "running" | "in progress" => totals.running += 1,
                    "retrying" | "retry" => totals.retrying += 1,
                    "completed" | "complete" | "done" | "succeeded" => totals.completed += 1,
                    "failed" | "error" => totals.failed += 1,
                    "canceled" | "cancelled" => totals.canceled += 1,
                    _ => totals.unknown += 1,
                }

                totals
            })
    }

    #[must_use]
    pub fn spec_view(&self) -> StateSpecView {
        StateSpecView {
            runtime: self.runtime.spec_view(),
            issue_totals: self.issue_totals(),
        }
    }
}

#[cfg(test)]
mod tests {
    use symphony_domain::IssueId;

    use crate::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

    #[test]
    fn issue_totals_group_known_states() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 2,
                retrying: 1,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            issues: vec![
                IssueSnapshot {
                    id: IssueId("SYM-1".to_owned()),
                    identifier: "SYM-1".to_owned(),
                    state: "Running".to_owned(),
                    retry_attempts: 0,
                },
                IssueSnapshot {
                    id: IssueId("SYM-2".to_owned()),
                    identifier: "SYM-2".to_owned(),
                    state: "retry".to_owned(),
                    retry_attempts: 2,
                },
                IssueSnapshot {
                    id: IssueId("SYM-3".to_owned()),
                    identifier: "SYM-3".to_owned(),
                    state: "Done".to_owned(),
                    retry_attempts: 1,
                },
                IssueSnapshot {
                    id: IssueId("SYM-4".to_owned()),
                    identifier: "SYM-4".to_owned(),
                    state: "Failed".to_owned(),
                    retry_attempts: 3,
                },
                IssueSnapshot {
                    id: IssueId("SYM-5".to_owned()),
                    identifier: "SYM-5".to_owned(),
                    state: "unknown-state".to_owned(),
                    retry_attempts: 0,
                },
            ],
        };

        let totals = snapshot.issue_totals();
        assert_eq!(totals.total, 5);
        assert_eq!(totals.running, 1);
        assert_eq!(totals.retrying, 1);
        assert_eq!(totals.completed, 1);
        assert_eq!(totals.failed, 1);
        assert_eq!(totals.canceled, 0);
        assert_eq!(totals.unknown, 1);
    }

    #[test]
    fn state_spec_view_uses_runtime_and_issue_totals() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 1,
                retrying: 0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            issues: vec![],
        };

        let spec_view = snapshot.spec_view();
        assert_eq!(spec_view.runtime.counts.running, 1);
        assert_eq!(spec_view.runtime.counts.retrying, 0);
        assert_eq!(spec_view.issue_totals.total, 0);
    }
}
