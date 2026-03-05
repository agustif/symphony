use serde::{Deserialize, Serialize};

use crate::{IssueSnapshot, RuntimeSnapshot};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub runtime: RuntimeSnapshot,
    #[serde(default)]
    pub issues: Vec<IssueSnapshot>,
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
}
