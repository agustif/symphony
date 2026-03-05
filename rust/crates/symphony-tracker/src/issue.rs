use symphony_domain::IssueId;

use crate::TrackerState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerIssue {
    pub id: IssueId,
    pub identifier: String,
    pub state: TrackerState,
}

impl TrackerIssue {
    pub fn new(id: IssueId, identifier: impl Into<String>, state: TrackerState) -> Self {
        Self {
            id,
            identifier: identifier.into(),
            state,
        }
    }
}
