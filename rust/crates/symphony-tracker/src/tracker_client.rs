use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use symphony_domain::IssueId;

use crate::{TrackerError, TrackerIssue, TrackerState};

#[cfg(test)]
mockall::mock! {
    pub TrackerClient {}

    #[async_trait]
    impl TrackerClient for TrackerClient {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError>;
        async fn fetch_candidates_by_states(
            &self,
            states: &[TrackerState],
        ) -> Result<Vec<TrackerIssue>, TrackerError>;
        async fn fetch_states_by_ids(
            &self,
            ids: &[IssueId],
        ) -> Result<HashMap<IssueId, TrackerState>, TrackerError>;
        async fn fetch_terminal_candidates(
            &self,
            terminal_states: &[TrackerState],
        ) -> Result<Vec<TrackerIssue>, TrackerError>;
        async fn fetch_terminal_states_by_ids(
            &self,
            ids: &[IssueId],
            terminal_states: &[TrackerState],
        ) -> Result<HashMap<IssueId, TrackerState>, TrackerError>;
    }
}

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
            .filter_map(TrackerState::normalized_key)
            .collect::<HashSet<_>>();
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let issues = self.fetch_candidates().await?;
        Ok(issues
            .into_iter()
            .filter(|issue| {
                issue
                    .state
                    .normalized_key()
                    .is_some_and(|state| states.contains(&state))
            })
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

    async fn fetch_terminal_candidates(
        &self,
        terminal_states: &[TrackerState],
    ) -> Result<Vec<TrackerIssue>, TrackerError> {
        if terminal_states.is_empty() {
            return Ok(Vec::new());
        }
        self.fetch_candidates_by_states(terminal_states).await
    }

    async fn fetch_terminal_states_by_ids(
        &self,
        ids: &[IssueId],
        terminal_states: &[TrackerState],
    ) -> Result<HashMap<IssueId, TrackerState>, TrackerError> {
        if ids.is_empty() || terminal_states.is_empty() {
            return Ok(HashMap::new());
        }
        let states = self.fetch_states_by_ids(ids).await?;
        Ok(states
            .into_iter()
            .filter(|(_, state)| state.is_terminal(terminal_states))
            .collect())
    }
}
