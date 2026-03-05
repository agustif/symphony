use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use symphony_domain::IssueId;

use crate::{TrackerError, TrackerIssue, TrackerState};

#[async_trait]
pub trait TrackerClient: Send + Sync {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError>;

    async fn fetch_candidates_by_states(
        &self,
        states: &[TrackerState],
    ) -> Result<Vec<TrackerIssue>, TrackerError> {
        if states.is_empty() {
            return self.fetch_candidates().await;
        }
        let states = states
            .iter()
            .map(TrackerState::as_str)
            .collect::<HashSet<_>>();
        let issues = self.fetch_candidates().await?;
        Ok(issues
            .into_iter()
            .filter(|issue| states.contains(issue.state.as_str()))
            .collect())
    }

    async fn fetch_states_by_ids(
        &self,
        ids: &[IssueId],
    ) -> Result<HashMap<IssueId, TrackerState>, TrackerError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let ids = ids.iter().collect::<HashSet<_>>();
        let issues = self.fetch_candidates().await?;
        let mut states_by_id = HashMap::with_capacity(issues.len());
        for issue in issues {
            if ids.contains(&issue.id) {
                states_by_id.insert(issue.id, issue.state);
            }
        }
        Ok(states_by_id)
    }
}
