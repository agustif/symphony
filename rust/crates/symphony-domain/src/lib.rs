#![forbid(unsafe_code)]

#[cfg(test)]
mod property_tests;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct IssueId(pub String);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningEntry {
    pub identifier: Option<String>,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub codex_app_server_pid: Option<u32>,
    pub last_codex_event: Option<String>,
    pub last_codex_timestamp: Option<u64>,
    pub last_codex_message: Option<String>,
    pub usage: Usage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryEntry {
    pub attempt: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestratorState {
    pub claimed: HashSet<IssueId>,
    pub running: HashMap<IssueId, RunningEntry>,
    pub retry_attempts: HashMap<IssueId, RetryEntry>,
    pub codex_totals: CodexTotals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Claim(IssueId),
    MarkRunning(IssueId),
    UpdateAgent {
        issue_id: IssueId,
        session_id: Option<String>,
        thread_id: Option<String>,
        turn_id: Option<String>,
        pid: Option<u32>,
        event: Option<String>,
        timestamp: Option<u64>,
        message: Option<String>,
        usage: Option<Usage>,
    },
    QueueRetry {
        issue_id: IssueId,
        attempt: u32,
    },
    Release(IssueId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Dispatch(IssueId),
    ScheduleRetry {
        issue_id: IssueId,
        attempt: u32,
    },
    ReleaseClaim(IssueId),
    TransitionRejected {
        issue_id: IssueId,
        reason: TransitionRejection,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransitionRejection {
    #[error("issue is already claimed")]
    AlreadyClaimed,
    #[error("issue is not claimed")]
    MissingClaim,
    #[error("issue is already running")]
    AlreadyRunning,
    #[error("invalid retry attempt")]
    InvalidRetryAttempt,
    #[error("retry attempt regression")]
    RetryAttemptRegression,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InvariantError {
    #[error("issue cannot run without claim")]
    RunningWithoutClaim,
    #[error("issue cannot be queued for retry without claim")]
    RetryWithoutClaim,
    #[error("issue cannot be running and retrying at the same time")]
    RunningAndRetrying,
    #[error("retry attempts must always be positive")]
    RetryAttemptMustBePositive,
    #[error("transition rejected: {0}")]
    TransitionRejected(TransitionRejection),
}

impl From<TransitionRejection> for InvariantError {
    fn from(rejection: TransitionRejection) -> Self {
        InvariantError::TransitionRejected(rejection)
    }
}

pub fn reduce(mut state: OrchestratorState, event: Event) -> (OrchestratorState, Vec<Command>) {
    match event {
        Event::Claim(issue_id) => {
            if state.claimed.contains(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::AlreadyClaimed);
            }
            state.claimed.insert(issue_id);
        }
        Event::MarkRunning(issue_id) => {
            if !state.claimed.contains(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::MissingClaim);
            }
            if state.running.contains_key(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::AlreadyRunning);
            }
            let dispatch_issue_id = issue_id.clone();
            state.retry_attempts.remove(&issue_id);
            state.running.insert(issue_id, RunningEntry::default());
            return (state, vec![Command::Dispatch(dispatch_issue_id)]);
        }
        Event::UpdateAgent {
            issue_id,
            session_id,
            thread_id,
            turn_id,
            pid,
            event,
            timestamp,
            message,
            usage,
        } => {
            if !state.running.contains_key(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::MissingClaim);
            }
            if let Some(entry) = state.running.get_mut(&issue_id) {
                if let Some(sid) = session_id {
                    entry.session_id = Some(sid);
                }
                if let Some(tid) = thread_id {
                    entry.thread_id = Some(tid);
                }
                if let Some(tuid) = turn_id {
                    entry.turn_id = Some(tuid);
                }
                if let Some(p) = pid {
                    entry.codex_app_server_pid = Some(p);
                }
                if let Some(e) = event {
                    entry.last_codex_event = Some(e);
                }
                if let Some(ts) = timestamp {
                    entry.last_codex_timestamp = Some(ts);
                }
                if let Some(msg) = message {
                    entry.last_codex_message = Some(msg);
                }
                if let Some(u) = &usage {
                    entry.usage = u.clone();
                    state.codex_totals.input_tokens += u.input_tokens;
                    state.codex_totals.output_tokens += u.output_tokens;
                    state.codex_totals.total_tokens += u.total_tokens;
                }
            }
            return (state, Vec::new());
        }
        Event::QueueRetry { issue_id, attempt } => {
            if !state.claimed.contains(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::MissingClaim);
            }
            if attempt == 0 {
                return reject_transition(
                    state,
                    issue_id,
                    TransitionRejection::InvalidRetryAttempt,
                );
            }
            if state
                .retry_attempts
                .get(&issue_id)
                .is_some_and(|retry_entry| retry_entry.attempt >= attempt)
            {
                return reject_transition(
                    state,
                    issue_id,
                    TransitionRejection::RetryAttemptRegression,
                );
            }
            state.running.remove(&issue_id);
            state
                .retry_attempts
                .insert(issue_id.clone(), RetryEntry { attempt });
            return (state, vec![Command::ScheduleRetry { issue_id, attempt }]);
        }
        Event::Release(issue_id) => {
            if !state.claimed.contains(&issue_id) {
                return reject_transition(state, issue_id, TransitionRejection::MissingClaim);
            }
            state.running.remove(&issue_id);
            state.claimed.remove(&issue_id);
            state.retry_attempts.remove(&issue_id);
            return (state, vec![Command::ReleaseClaim(issue_id)]);
        }
    }
    (state, Vec::new())
}

pub fn validate_invariants(state: &OrchestratorState) -> Result<(), InvariantError> {
    if state.running.keys().any(|id| !state.claimed.contains(id)) {
        return Err(InvariantError::RunningWithoutClaim);
    }
    if state
        .retry_attempts
        .keys()
        .any(|issue_id| !state.claimed.contains(issue_id))
    {
        return Err(InvariantError::RetryWithoutClaim);
    }
    if state
        .running
        .keys()
        .any(|issue_id| state.retry_attempts.contains_key(issue_id))
    {
        return Err(InvariantError::RunningAndRetrying);
    }
    if state
        .retry_attempts
        .values()
        .any(|retry_entry| retry_entry.attempt == 0)
    {
        return Err(InvariantError::RetryAttemptMustBePositive);
    }
    Ok(())
}

fn reject_transition(
    state: OrchestratorState,
    issue_id: IssueId,
    reason: TransitionRejection,
) -> (OrchestratorState, Vec<Command>) {
    (
        state,
        vec![Command::TransitionRejected { issue_id, reason }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue_id(raw: &str) -> IssueId {
        IssueId(raw.to_owned())
    }

    fn assert_deterministic(state: OrchestratorState, event: Event) {
        let (first_state, first_commands) = reduce(state.clone(), event.clone());
        let (second_state, second_commands) = reduce(state, event);
        assert_eq!(first_state, second_state);
        assert_eq!(first_commands, second_commands);
    }

    #[test]
    fn mark_running_requires_existing_claim() {
        let issue_id = issue_id("SYM-1");
        let (state, commands) = reduce(
            OrchestratorState::default(),
            Event::MarkRunning(issue_id.clone()),
        );
        assert_eq!(state, OrchestratorState::default());
        assert_eq!(
            commands,
            vec![Command::TransitionRejected {
                issue_id,
                reason: TransitionRejection::MissingClaim,
            }],
        );
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn claim_then_mark_running_dispatches_once() {
        let issue_id = issue_id("SYM-9");
        let (state, claim_commands) =
            reduce(OrchestratorState::default(), Event::Claim(issue_id.clone()));
        let (state, run_commands) = reduce(state, Event::MarkRunning(issue_id.clone()));

        assert!(claim_commands.is_empty());
        assert_eq!(run_commands, vec![Command::Dispatch(issue_id.clone())]);
        assert!(state.claimed.contains(&issue_id));
        assert!(state.running.contains_key(&issue_id));
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn queue_retry_requires_existing_claim() {
        let issue_id = issue_id("SYM-3");
        let (state, commands) = reduce(
            OrchestratorState::default(),
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 1,
            },
        );
        assert_eq!(state, OrchestratorState::default());
        assert_eq!(
            commands,
            vec![Command::TransitionRejected {
                issue_id,
                reason: TransitionRejection::MissingClaim,
            }],
        );
    }

    #[test]
    fn queue_retry_moves_issue_from_running_to_retry() {
        let issue_id = issue_id("SYM-17");
        let (state, _) = reduce(OrchestratorState::default(), Event::Claim(issue_id.clone()));
        let (state, _) = reduce(state, Event::MarkRunning(issue_id.clone()));
        let (state, commands) = reduce(
            state,
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 2,
            },
        );

        assert!(!state.running.contains_key(&issue_id));
        assert!(state.claimed.contains(&issue_id));
        assert_eq!(
            state.retry_attempts.get(&issue_id),
            Some(&RetryEntry { attempt: 2 }),
        );
        assert_eq!(
            commands,
            vec![Command::ScheduleRetry {
                issue_id,
                attempt: 2,
            }],
        );
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn release_clears_all_tracking_for_issue() {
        let issue_id = issue_id("SYM-1");
        let (state, _) = reduce(OrchestratorState::default(), Event::Claim(issue_id.clone()));
        let (state, _) = reduce(state, Event::MarkRunning(issue_id.clone()));
        let (state, _) = reduce(
            state,
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 3,
            },
        );
        let (state, commands) = reduce(state, Event::Release(issue_id.clone()));
        assert!(state.running.is_empty());
        assert!(state.claimed.is_empty());
        assert!(state.retry_attempts.is_empty());
        assert_eq!(commands, vec![Command::ReleaseClaim(issue_id)]);
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn release_requires_existing_claim() {
        let issue_id = issue_id("SYM-99");
        let base_state = OrchestratorState::default();
        let (state, commands) = reduce(base_state.clone(), Event::Release(issue_id.clone()));
        assert_eq!(state, base_state);
        assert_eq!(
            commands,
            vec![Command::TransitionRejected {
                issue_id,
                reason: TransitionRejection::MissingClaim,
            }],
        );
    }

    #[test]
    fn queue_retry_rejects_non_increasing_attempts() {
        let issue_id = issue_id("SYM-77");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .retry_attempts
            .insert(issue_id.clone(), RetryEntry { attempt: 2 });

        let (state, commands) = reduce(
            state.clone(),
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 2,
            },
        );

        assert_eq!(
            state.retry_attempts.get(&issue_id),
            Some(&RetryEntry { attempt: 2 })
        );
        assert_eq!(
            commands,
            vec![Command::TransitionRejected {
                issue_id,
                reason: TransitionRejection::RetryAttemptRegression,
            }],
        );
    }

    #[test]
    fn validate_invariants_rejects_running_without_claim() {
        let issue_id = issue_id("SYM-42");
        let mut state = OrchestratorState::default();
        state.running.insert(issue_id, RunningEntry::default());
        assert_eq!(
            validate_invariants(&state),
            Err(InvariantError::RunningWithoutClaim),
        );
    }

    #[test]
    fn validate_invariants_rejects_zero_retry_attempts() {
        let issue_id = issue_id("SYM-314");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .retry_attempts
            .insert(issue_id, RetryEntry { attempt: 0 });
        assert_eq!(
            validate_invariants(&state),
            Err(InvariantError::RetryAttemptMustBePositive),
        );
    }

    #[test]
    fn reduce_is_deterministic_across_lifecycle_events() {
        let issue_id = issue_id("SYM-615");
        let mut running_state = OrchestratorState::default();
        running_state.claimed.insert(issue_id.clone());

        let mut retrying_state = OrchestratorState::default();
        retrying_state.claimed.insert(issue_id.clone());
        retrying_state
            .retry_attempts
            .insert(issue_id.clone(), RetryEntry { attempt: 1 });

        assert_deterministic(OrchestratorState::default(), Event::Claim(issue_id.clone()));
        assert_deterministic(running_state.clone(), Event::MarkRunning(issue_id.clone()));
        assert_deterministic(
            running_state,
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 1,
            },
        );
        assert_deterministic(retrying_state, Event::Release(issue_id));
    }
}
