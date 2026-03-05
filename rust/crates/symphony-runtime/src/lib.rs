#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::sync::Arc;

use symphony_domain::{
    Command, Event, InvariantError, IssueId, OrchestratorState, reduce, validate_invariants,
};
use symphony_observability::RuntimeSnapshot;
use symphony_tracker::{TrackerClient, TrackerError};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("tracker request failed: {0}")]
    Tracker(#[from] TrackerError),
    #[error("orchestrator invariant violated: {0}")]
    Invariant(#[from] InvariantError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickResult {
    pub state: OrchestratorState,
    pub commands: Vec<Command>,
}

pub struct Runtime<T: TrackerClient> {
    tracker: Arc<T>,
    state: Mutex<OrchestratorState>,
}

impl<T: TrackerClient> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        Self {
            tracker,
            state: Mutex::new(OrchestratorState::default()),
        }
    }

    pub async fn run_tick(&self, max_concurrent_agents: usize) -> Result<TickResult, RuntimeError> {
        let issues = self.tracker.fetch_candidates().await?;
        let candidate_ids: Vec<IssueId> = issues.into_iter().map(|issue| issue.id).collect();
        let candidate_set: HashSet<IssueId> = candidate_ids.iter().cloned().collect();

        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        for issue_id in stale_claims(&state, &candidate_set) {
            state = apply_event(state, Event::Release(issue_id), &mut commands)?;
        }

        let mut available_slots = max_concurrent_agents.saturating_sub(state.running.len());
        for issue_id in candidate_ids {
            if available_slots == 0 {
                break;
            }
            if state.claimed.contains(&issue_id)
                || state.running.contains(&issue_id)
                || state.retry_attempts.contains_key(&issue_id)
            {
                continue;
            }
            state = apply_event(state, Event::Claim(issue_id.clone()), &mut commands)?;
            state = apply_event(state, Event::MarkRunning(issue_id), &mut commands)?;
            available_slots = available_slots.saturating_sub(1);
        }

        validate_invariants(&state)?;
        *state_guard = state.clone();

        Ok(TickResult { state, commands })
    }

    pub async fn snapshot(&self) -> RuntimeSnapshot {
        let state = self.state.lock().await;
        RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        }
    }

    pub async fn state(&self) -> OrchestratorState {
        self.state.lock().await.clone()
    }
}

fn stale_claims(state: &OrchestratorState, candidates: &HashSet<IssueId>) -> Vec<IssueId> {
    let mut stale: Vec<IssueId> = state
        .claimed
        .iter()
        .filter(|issue_id| {
            !state.running.contains(*issue_id)
                && !state.retry_attempts.contains_key(*issue_id)
                && !candidates.contains(*issue_id)
        })
        .cloned()
        .collect();
    stale.sort_by(|left, right| left.0.cmp(&right.0));
    stale
}

fn apply_event(
    state: OrchestratorState,
    event: Event,
    commands: &mut Vec<Command>,
) -> Result<OrchestratorState, InvariantError> {
    let (next_state, mut emitted) = reduce(state, event);
    validate_invariants(&next_state)?;
    commands.append(&mut emitted);
    Ok(next_state)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use symphony_domain::{Command, IssueId, validate_invariants};
    use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};
    use tokio::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct MutableTracker {
        issues: Mutex<Vec<TrackerIssue>>,
    }

    impl MutableTracker {
        async fn set_candidates(&self, issues: Vec<TrackerIssue>) {
            let mut guard = self.issues.lock().await;
            *guard = issues;
        }
    }

    #[async_trait]
    impl TrackerClient for MutableTracker {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(self.issues.lock().await.clone())
        }
    }

    fn issue(id: &str) -> IssueId {
        IssueId(id.to_owned())
    }

    fn tracker_issue(id: &str) -> TrackerIssue {
        TrackerIssue {
            id: issue(id),
            identifier: id.to_owned(),
            state: TrackerState::new("Todo"),
        }
    }

    #[tokio::test]
    async fn runtime_reports_zero_when_no_candidates() {
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(tracker);
        let snapshot = runtime.snapshot().await;
        assert_eq!(snapshot.running, 0);
        assert_eq!(snapshot.retrying, 0);
    }

    #[tokio::test]
    async fn scheduler_never_runs_without_claim() {
        let issue_id = issue("SYM-100");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let first_tick = runtime
            .run_tick(1)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            first_tick.commands,
            vec![Command::Dispatch(issue_id.clone())],
        );

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert!(state.running.contains(&issue_id));
        assert_eq!(validate_invariants(&state), Ok(()));

        let second_tick = runtime
            .run_tick(1)
            .await
            .expect("already running issue should not be re-dispatched");
        assert!(second_tick.commands.is_empty());
    }

    #[tokio::test]
    async fn scheduler_releases_stale_claims_in_sorted_order() {
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(Vec::new()).await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let running_issue = issue("SYM-200");
        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue("SYM-20"));
            state.claimed.insert(issue("SYM-10"));
            state.claimed.insert(running_issue.clone());
            state.running.insert(running_issue.clone());
        }

        let tick = runtime
            .run_tick(3)
            .await
            .expect("release tick should succeed");
        assert_eq!(
            tick.commands,
            vec![
                Command::ReleaseClaim(issue("SYM-10")),
                Command::ReleaseClaim(issue("SYM-20")),
            ],
        );

        let state = runtime.state().await;
        assert!(state.claimed.contains(&running_issue));
        assert!(state.running.contains(&running_issue));
        assert_eq!(validate_invariants(&state), Ok(()));
    }
}
