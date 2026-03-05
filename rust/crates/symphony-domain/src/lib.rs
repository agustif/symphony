#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IssueId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryEntry {
    pub attempt: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestratorState {
    pub claimed: HashSet<IssueId>,
    pub running: HashSet<IssueId>,
    pub retry_attempts: HashMap<IssueId, RetryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Claim(IssueId),
    MarkRunning(IssueId),
    Release(IssueId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    None,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InvariantError {
    #[error("issue cannot run without claim")]
    RunningWithoutClaim,
}

pub fn reduce(mut state: OrchestratorState, event: Event) -> (OrchestratorState, Vec<Command>) {
    match event {
        Event::Claim(issue_id) => {
            state.claimed.insert(issue_id);
        }
        Event::MarkRunning(issue_id) => {
            state.claimed.insert(issue_id.clone());
            state.running.insert(issue_id);
        }
        Event::Release(issue_id) => {
            state.running.remove(&issue_id);
            state.claimed.remove(&issue_id);
            state.retry_attempts.remove(&issue_id);
        }
    }
    (state, vec![Command::None])
}

pub fn validate_invariants(state: &OrchestratorState) -> Result<(), InvariantError> {
    if state.running.iter().all(|id| state.claimed.contains(id)) {
        Ok(())
    } else {
        Err(InvariantError::RunningWithoutClaim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_clears_all_tracking_for_issue() {
        let issue_id = IssueId("SYM-1".to_owned());
        let (state, _) = reduce(
            OrchestratorState::default(),
            Event::MarkRunning(issue_id.clone()),
        );
        let (state, _) = reduce(state, Event::Release(issue_id));
        assert!(state.running.is_empty());
        assert!(state.claimed.is_empty());
        assert!(state.retry_attempts.is_empty());
        assert_eq!(validate_invariants(&state), Ok(()));
    }
}
