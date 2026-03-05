use symphony_domain::IssueId;

use crate::TrackerState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerIssue {
    pub id: IssueId,
    pub identifier: String,
    pub state: TrackerState,
    pub priority: Option<i32>,
    pub created_at: Option<u64>,
}

impl TrackerIssue {
    pub fn new(id: IssueId, identifier: impl Into<String>, state: TrackerState) -> Self {
        Self {
            id,
            identifier: identifier.into(),
            state,
            priority: None,
            created_at: None,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_created_at(mut self, created_at: u64) -> Self {
        self.created_at = Some(created_at);
        self
    }
}
