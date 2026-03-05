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
}
