use symphony_domain::IssueId;

use crate::TrackerState;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackerBlockerRef {
    pub id: Option<String>,
    pub identifier: Option<String>,
    pub state: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerIssue {
    pub id: IssueId,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub state: TrackerState,
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<TrackerBlockerRef>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl TrackerIssue {
    pub fn new(id: IssueId, identifier: impl Into<String>, state: TrackerState) -> Self {
        let identifier = identifier.into();
        Self {
            id,
            title: identifier.clone(),
            identifier,
            description: None,
            priority: None,
            state,
            branch_name: None,
            url: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_branch_name(mut self, branch_name: impl Into<String>) -> Self {
        self.branch_name = Some(branch_name.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    pub fn with_blocked_by(mut self, blocked_by: Vec<TrackerBlockerRef>) -> Self {
        self.blocked_by = blocked_by;
        self
    }

    pub fn with_created_at(mut self, created_at: u64) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn with_updated_at(mut self, updated_at: u64) -> Self {
        self.updated_at = Some(updated_at);
        self
    }
}
