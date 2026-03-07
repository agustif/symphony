#![forbid(unsafe_code)]

//! Pure domain model for Symphony orchestration state.
//!
//! This crate owns the reducer, its executable invariant catalog, and the
//! serialization-safe diagnostics used by runtime and operator surfaces.

mod agent_update;
mod invariants;

#[cfg(test)]
mod property_tests;

use im::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::invariants::{InvariantDescriptor, invariant_catalog, invariant_descriptor};
use crate::{agent_update::apply_agent_update, invariants::validate_all};

/// Stable issue identifier used as the primary key across reducer state.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct IssueId(pub String);

/// Absolute Codex token counters reported by a worker event.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Live worker/session metadata tracked while an issue is running.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningEntry {
    pub identifier: Option<String>,
    pub tracker_state: Option<String>,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub turn_count: u32,
    pub started_at: Option<u64>,
    pub codex_app_server_pid: Option<u32>,
    pub last_codex_event: Option<String>,
    pub last_codex_timestamp: Option<u64>,
    pub last_codex_message: Option<String>,
    pub usage: Usage,
}

/// Workspace-wide aggregate Codex counters derived from absolute worker usage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Pending retry metadata for an issue that remains claimed but is not running.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryEntry {
    pub attempt: u32,
    pub identifier: Option<String>,
    pub error: Option<String>,
    pub due_at: Option<u64>,
}

/// Single authoritative reducer state for orchestration topology and counters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestratorState {
    pub claimed: HashSet<IssueId>,
    pub running: HashMap<IssueId, RunningEntry>,
    pub retry_attempts: HashMap<IssueId, RetryEntry>,
    /// Set of issue IDs that have completed processing (terminal state reached).
    /// Populated when an issue transitions to a terminal tracker state.
    /// Used for bookkeeping and observability only; not considered during dispatch eligibility.
    pub completed: HashSet<IssueId>,
    pub codex_totals: CodexTotals,
    pub codex_rate_limits: Option<serde_json::Value>,
}

/// Incremental live-session update emitted by the worker/protocol layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub issue_id: IssueId,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub pid: Option<u32>,
    pub event: Option<String>,
    pub timestamp: Option<u64>,
    pub message: Option<String>,
    pub usage: Option<Usage>,
    pub rate_limits: Option<serde_json::Value>,
}

/// Reducer input events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Claim(IssueId),
    MarkRunning(IssueId),
    UpdateAgent(Box<AgentUpdate>),
    QueueRetry { issue_id: IssueId, attempt: u32 },
    Release(IssueId),
}

/// Reducer side-effect intents emitted by successful or rejected transitions.
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

/// Typed reasons for state-preserving transition rejection.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransitionRejection {
    #[error("issue is already claimed")]
    AlreadyClaimed,
    #[error("issue is not claimed")]
    MissingClaim,
    #[error("issue is already running")]
    AlreadyRunning,
    #[error("issue is running without being claimed (invariant violation)")]
    RunningWithoutClaim,
    #[error("invalid retry attempt")]
    InvalidRetryAttempt,
    #[error("retry attempt regression")]
    RetryAttemptRegression,
}

/// Serialization-safe operator payload for rejected transitions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRejectionPayload {
    pub code: String,
    pub message: String,
}

impl TransitionRejection {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadyClaimed => "already_claimed",
            Self::MissingClaim => "missing_claim",
            Self::AlreadyRunning => "already_running",
            Self::RunningWithoutClaim => "running_without_claim",
            Self::InvalidRetryAttempt => "invalid_retry_attempt",
            Self::RetryAttemptRegression => "retry_attempt_regression",
        }
    }

    #[must_use]
    pub fn to_payload(&self) -> TransitionRejectionPayload {
        TransitionRejectionPayload {
            code: self.code().to_owned(),
            message: self.to_string(),
        }
    }
}

/// Executable invariant violations for the orchestrator reducer state.
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

/// Serialization-safe operator payload for invariant failures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantViolation {
    pub code: String,
    pub message: String,
}

impl InvariantError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::RunningWithoutClaim => "running_without_claim",
            Self::RetryWithoutClaim => "retry_without_claim",
            Self::RunningAndRetrying => "running_and_retrying",
            Self::RetryAttemptMustBePositive => "retry_attempt_must_be_positive",
            Self::TransitionRejected(rejection) => rejection.code(),
        }
    }

    #[must_use]
    pub fn to_violation(&self) -> InvariantViolation {
        InvariantViolation {
            code: self.code().to_owned(),
            message: self.to_string(),
        }
    }
}

impl From<TransitionRejection> for InvariantError {
    fn from(rejection: TransitionRejection) -> Self {
        InvariantError::TransitionRejected(rejection)
    }
}

/// Applies a reducer event and returns the next state plus emitted commands.
///
/// # Examples
///
/// ```
/// use symphony_domain::{Command, Event, IssueId, OrchestratorState, reduce};
///
/// let issue_id = IssueId("SYM-101".to_owned());
/// let (claimed_state, claim_commands) =
///     reduce(OrchestratorState::default(), Event::Claim(issue_id.clone()));
/// let (running_state, run_commands) = reduce(claimed_state, Event::MarkRunning(issue_id.clone()));
///
/// assert!(claim_commands.is_empty());
/// assert_eq!(run_commands, vec![Command::Dispatch(issue_id.clone())]);
/// assert!(running_state.claimed.contains(&issue_id));
/// assert!(running_state.running.contains_key(&issue_id));
/// ```
#[must_use]
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
        Event::UpdateAgent(update) => {
            let update = *update;
            let issue_id = update.issue_id.clone();

            if let Err(reason) = apply_agent_update(&mut state, update) {
                return reject_transition(state, issue_id, reason);
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
            state.retry_attempts.insert(
                issue_id.clone(),
                RetryEntry {
                    attempt,
                    ..RetryEntry::default()
                },
            );
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

/// Validates the reducer's executable invariant catalog in stable order.
///
/// Use [`invariant_catalog`] to inspect the full set of stable runtime
/// invariants that this function enforces.
///
/// # Examples
///
/// ```
/// use symphony_domain::{OrchestratorState, validate_invariants};
///
/// assert_eq!(validate_invariants(&OrchestratorState::default()), Ok(()));
/// ```
pub fn validate_invariants(state: &OrchestratorState) -> Result<(), InvariantError> {
    validate_all(state)
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
    fn update_agent_requires_running_issue() {
        let issue_id = issue_id("SYM-11");
        let update = AgentUpdate {
            issue_id: issue_id.clone(),
            session_id: Some("session-1".to_owned()),
            thread_id: Some("thread-1".to_owned()),
            turn_id: Some("turn-1".to_owned()),
            pid: Some(1234),
            event: Some("turn.start".to_owned()),
            timestamp: Some(1_700_000_000),
            message: Some("processing".to_owned()),
            usage: Some(Usage {
                input_tokens: 10,
                output_tokens: 5,
                total_tokens: 15,
            }),
            rate_limits: None,
        };

        let (state, commands) = reduce(
            OrchestratorState::default(),
            Event::UpdateAgent(Box::new(update)),
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
    fn update_agent_tracks_absolute_usage_without_double_counting() {
        let issue_id = issue_id("SYM-12");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .running
            .insert(issue_id.clone(), RunningEntry::default());

        let first_usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
        };
        let repeated_usage = first_usage.clone();
        let increased_usage = Usage {
            input_tokens: 140,
            output_tokens: 70,
            total_tokens: 210,
        };

        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-1".to_owned()),
                pid: None,
                event: None,
                timestamp: Some(1_700_000_000),
                message: None,
                usage: Some(first_usage.clone()),
                rate_limits: None,
            })),
        );
        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-1".to_owned()),
                pid: None,
                event: None,
                timestamp: Some(1_700_000_001),
                message: None,
                usage: Some(repeated_usage),
                rate_limits: None,
            })),
        );
        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-2".to_owned()),
                pid: None,
                event: None,
                timestamp: Some(1_700_000_002),
                message: None,
                usage: Some(increased_usage.clone()),
                rate_limits: None,
            })),
        );

        let entry = state
            .running
            .get(&issue_id)
            .expect("issue should remain running");
        assert_eq!(entry.usage, increased_usage);
        assert_eq!(entry.turn_count, 2);
        assert_eq!(state.codex_totals.input_tokens, 140);
        assert_eq!(state.codex_totals.output_tokens, 70);
        assert_eq!(state.codex_totals.total_tokens, 210);
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn update_agent_resets_usage_baseline_when_session_identity_changes() {
        let issue_id = issue_id("SYM-13");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .running
            .insert(issue_id.clone(), RunningEntry::default());

        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-1".to_owned()),
                pid: None,
                event: Some("turn.start".to_owned()),
                timestamp: Some(1_700_000_000),
                message: None,
                usage: Some(Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    total_tokens: 150,
                }),
                rate_limits: None,
            })),
        );

        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-2".to_owned()),
                thread_id: Some("thread-2".to_owned()),
                turn_id: Some("turn-1".to_owned()),
                pid: None,
                event: Some("turn.resume".to_owned()),
                timestamp: Some(1_700_000_005),
                message: Some("session rotated".to_owned()),
                usage: Some(Usage {
                    input_tokens: 30,
                    output_tokens: 10,
                    total_tokens: 40,
                }),
                rate_limits: Some(serde_json::json!({
                    "primary": {
                        "remaining": 9
                    }
                })),
            })),
        );

        let entry = state
            .running
            .get(&issue_id)
            .expect("issue should remain running");
        assert_eq!(entry.session_id.as_deref(), Some("session-2"));
        assert_eq!(entry.thread_id.as_deref(), Some("thread-2"));
        assert_eq!(entry.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(entry.turn_count, 1);
        assert_eq!(entry.started_at, Some(1_700_000_000));
        assert_eq!(entry.last_codex_timestamp, Some(1_700_000_005));
        assert_eq!(entry.last_codex_message.as_deref(), Some("session rotated"));
        assert_eq!(
            entry.usage,
            Usage {
                input_tokens: 30,
                output_tokens: 10,
                total_tokens: 40,
            },
        );
        assert_eq!(state.codex_totals.input_tokens, 130);
        assert_eq!(state.codex_totals.output_tokens, 60);
        assert_eq!(state.codex_totals.total_tokens, 190);
        assert_eq!(
            state.codex_rate_limits,
            Some(serde_json::json!({
                "primary": {
                    "remaining": 9
                }
            })),
        );
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn update_agent_turn_count_is_stable_for_repeat_turn_ids() {
        let issue_id = issue_id("SYM-14");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .running
            .insert(issue_id.clone(), RunningEntry::default());

        let initial_turn = AgentUpdate {
            issue_id: issue_id.clone(),
            session_id: Some("session-1".to_owned()),
            thread_id: Some("thread-1".to_owned()),
            turn_id: Some("turn-1".to_owned()),
            pid: None,
            event: None,
            timestamp: Some(1_700_000_000),
            message: None,
            usage: None,
            rate_limits: None,
        };
        let repeated_turn = AgentUpdate {
            timestamp: Some(1_700_000_001),
            ..initial_turn.clone()
        };
        let new_turn = AgentUpdate {
            turn_id: Some("turn-2".to_owned()),
            timestamp: Some(1_700_000_002),
            ..initial_turn.clone()
        };

        let (state, _) = reduce(state, Event::UpdateAgent(Box::new(initial_turn.clone())));
        let (state, _) = reduce(state, Event::UpdateAgent(Box::new(repeated_turn)));
        let (state, _) = reduce(state, Event::UpdateAgent(Box::new(new_turn)));

        let entry = state
            .running
            .get(&issue_id)
            .expect("issue should remain running");
        assert_eq!(entry.turn_count, 2);
        assert_eq!(entry.turn_id.as_deref(), Some("turn-2"));
        assert_eq!(entry.started_at, Some(1_700_000_000));
        assert_eq!(entry.last_codex_timestamp, Some(1_700_000_002));
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn update_agent_usage_regression_does_not_decrement_totals() {
        let issue_id = issue_id("SYM-15");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .running
            .insert(issue_id.clone(), RunningEntry::default());

        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-1".to_owned()),
                pid: None,
                event: None,
                timestamp: Some(1_700_000_000),
                message: None,
                usage: Some(Usage {
                    input_tokens: 50,
                    output_tokens: 30,
                    total_tokens: 80,
                }),
                rate_limits: None,
            })),
        );
        let (state, _) = reduce(
            state,
            Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id: issue_id.clone(),
                session_id: Some("session-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                turn_id: Some("turn-2".to_owned()),
                pid: None,
                event: None,
                timestamp: Some(1_700_000_005),
                message: None,
                usage: Some(Usage {
                    input_tokens: 40,
                    output_tokens: 20,
                    total_tokens: 60,
                }),
                rate_limits: None,
            })),
        );

        let entry = state
            .running
            .get(&issue_id)
            .expect("issue should remain running");
        assert_eq!(entry.usage.total_tokens, 60);
        assert_eq!(state.codex_totals.input_tokens, 50);
        assert_eq!(state.codex_totals.output_tokens, 30);
        assert_eq!(state.codex_totals.total_tokens, 80);
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
            Some(&RetryEntry {
                attempt: 2,
                ..RetryEntry::default()
            }),
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
        state.retry_attempts.insert(
            issue_id.clone(),
            RetryEntry {
                attempt: 2,
                ..RetryEntry::default()
            },
        );

        let (state, commands) = reduce(
            state.clone(),
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt: 2,
            },
        );

        assert_eq!(
            state.retry_attempts.get(&issue_id),
            Some(&RetryEntry {
                attempt: 2,
                ..RetryEntry::default()
            })
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
        state.retry_attempts.insert(
            issue_id,
            RetryEntry {
                attempt: 0,
                ..RetryEntry::default()
            },
        );
        assert_eq!(
            validate_invariants(&state),
            Err(InvariantError::RetryAttemptMustBePositive),
        );
    }

    #[test]
    fn validate_invariants_rejects_retry_without_claim() {
        let issue_id = issue_id("SYM-271");
        let mut state = OrchestratorState::default();
        state.retry_attempts.insert(
            issue_id,
            RetryEntry {
                attempt: 1,
                ..RetryEntry::default()
            },
        );

        assert_eq!(
            validate_invariants(&state),
            Err(InvariantError::RetryWithoutClaim),
        );
    }

    #[test]
    fn validate_invariants_rejects_running_and_retrying_same_issue() {
        let issue_id = issue_id("SYM-272");
        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_id.clone());
        state
            .running
            .insert(issue_id.clone(), RunningEntry::default());
        state.retry_attempts.insert(
            issue_id,
            RetryEntry {
                attempt: 1,
                ..RetryEntry::default()
            },
        );

        assert_eq!(
            validate_invariants(&state),
            Err(InvariantError::RunningAndRetrying),
        );
    }

    #[test]
    fn invariant_catalog_covers_all_current_validation_errors() {
        let cases = [
            (
                {
                    let issue_id = issue_id("SYM-RUNNING");
                    let mut state = OrchestratorState::default();
                    state.running.insert(issue_id, RunningEntry::default());
                    state
                },
                InvariantError::RunningWithoutClaim,
            ),
            (
                {
                    let issue_id = issue_id("SYM-RETRY");
                    let mut state = OrchestratorState::default();
                    state.retry_attempts.insert(
                        issue_id,
                        RetryEntry {
                            attempt: 1,
                            ..RetryEntry::default()
                        },
                    );
                    state
                },
                InvariantError::RetryWithoutClaim,
            ),
            (
                {
                    let issue_id = issue_id("SYM-BOTH");
                    let mut state = OrchestratorState::default();
                    state.claimed.insert(issue_id.clone());
                    state
                        .running
                        .insert(issue_id.clone(), RunningEntry::default());
                    state.retry_attempts.insert(
                        issue_id,
                        RetryEntry {
                            attempt: 1,
                            ..RetryEntry::default()
                        },
                    );
                    state
                },
                InvariantError::RunningAndRetrying,
            ),
            (
                {
                    let issue_id = issue_id("SYM-ZERO");
                    let mut state = OrchestratorState::default();
                    state.claimed.insert(issue_id.clone());
                    state.retry_attempts.insert(
                        issue_id,
                        RetryEntry {
                            attempt: 0,
                            ..RetryEntry::default()
                        },
                    );
                    state
                },
                InvariantError::RetryAttemptMustBePositive,
            ),
        ];

        for (state, expected_error) in cases {
            let error = validate_invariants(&state).expect_err("state should violate an invariant");
            assert_eq!(error, expected_error);

            let descriptor = invariant_descriptor(error.code())
                .expect("every executable invariant must have catalog metadata");
            assert_eq!(descriptor.code, error.code());
            assert!(!descriptor.spec_sections.is_empty());
            assert!(!descriptor.proof_modules.is_empty());
        }
    }

    #[test]
    fn reduce_is_deterministic_across_lifecycle_events() {
        let issue_id = issue_id("SYM-615");
        let mut running_state = OrchestratorState::default();
        running_state.claimed.insert(issue_id.clone());

        let mut retrying_state = OrchestratorState::default();
        retrying_state.claimed.insert(issue_id.clone());
        retrying_state.retry_attempts.insert(
            issue_id.clone(),
            RetryEntry {
                attempt: 1,
                ..RetryEntry::default()
            },
        );

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

    #[test]
    fn transition_rejection_payload_is_serialization_safe() {
        let payload = TransitionRejection::InvalidRetryAttempt.to_payload();
        assert_eq!(payload.code, "invalid_retry_attempt");
        assert!(payload.message.contains("invalid retry attempt"));
        let encoded = serde_json::to_string(&payload).expect("payload should serialize");
        let decoded: TransitionRejectionPayload =
            serde_json::from_str(&encoded).expect("payload should deserialize");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn invariant_error_violation_payload_is_serialization_safe() {
        let violation = InvariantError::RetryWithoutClaim.to_violation();
        assert_eq!(violation.code, "retry_without_claim");
        assert!(violation.message.contains("retry"));
        let encoded = serde_json::to_string(&violation).expect("violation should serialize");
        let decoded: InvariantViolation =
            serde_json::from_str(&encoded).expect("violation should deserialize");
        assert_eq!(decoded, violation);
    }
}
