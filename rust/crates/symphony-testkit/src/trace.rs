//! Trace helpers for replaying reducer events and validating the executable
//! reducer contract.
//!
//! These helpers sit between unit tests and full conformance suites: they make
//! it cheap to prove that a generated event trace preserves domain invariants
//! and that emitted commands match the reducer's transition semantics.

use symphony_domain::{
    Command, Event, InvariantError, IssueId, OrchestratorState, RetryEntry, RunningEntry,
    TransitionRejection, reduce, validate_invariants,
};
use thiserror::Error;

use crate::StateSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStep {
    pub event: Event,
    pub commands: Vec<Command>,
    pub state: OrchestratorState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraceResult {
    pub initial_state: OrchestratorState,
    pub steps: Vec<TraceStep>,
    pub final_state: OrchestratorState,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TraceValidationError {
    #[error("initial_state violated a reducer invariant: {source}")]
    InitialInvariant { source: InvariantError },
    #[error("step {step_index} violated a reducer invariant: {source}")]
    Invariant {
        step_index: usize,
        source: InvariantError,
    },
    #[error(
        "step {step_index} emitted {command_count} commands; reducer steps must emit at most one"
    )]
    TooManyCommands {
        step_index: usize,
        command_count: usize,
    },
    #[error("step {step_index} mutated state after a rejected transition")]
    RejectionMutatedState { step_index: usize },
    #[error(
        "step {step_index} emitted Dispatch for {issue_id}, but the issue did not enter running state"
    )]
    DispatchWithoutRunning { step_index: usize, issue_id: String },
    #[error(
        "step {step_index} emitted ScheduleRetry for {issue_id}@{attempt}, but retry state does not match"
    )]
    RetryCommandMismatch {
        step_index: usize,
        issue_id: String,
        attempt: u32,
    },
    #[error("step {step_index} emitted ReleaseClaim for {issue_id}, but tracking still exists")]
    ReleaseCommandMismatch { step_index: usize, issue_id: String },
    #[error("step {step_index} violated the {event_kind} transition contract: {detail}")]
    TransitionContract {
        step_index: usize,
        event_kind: &'static str,
        detail: String,
    },
    #[error("final_state does not match the last recorded trace step")]
    FinalStateMismatch,
}

/// Replays reducer events from the default empty state.
///
/// # Examples
///
/// ```
/// use symphony_domain::{Event, IssueId};
/// use symphony_testkit::{run_trace, validate_trace};
///
/// let issue_id = IssueId("SYM-500".to_owned());
/// let trace = run_trace(vec![Event::Claim(issue_id.clone()), Event::MarkRunning(issue_id)]);
///
/// assert_eq!(validate_trace(&trace), Ok(()));
/// ```
pub fn run_trace(events: impl IntoIterator<Item = Event>) -> TraceResult {
    run_trace_from_state(OrchestratorState::default(), events)
}

/// Replays reducer events from an explicit starting state.
pub fn run_trace_from_state(
    mut state: OrchestratorState,
    events: impl IntoIterator<Item = Event>,
) -> TraceResult {
    let initial_state = state.clone();
    let mut steps = Vec::new();
    for event in events {
        let (next_state, commands) = reduce(state, event.clone());
        steps.push(TraceStep {
            event,
            commands,
            state: next_state.clone(),
        });
        state = next_state;
    }
    TraceResult {
        initial_state,
        steps,
        final_state: state,
    }
}

/// Validates the full reducer contract over a recorded trace.
///
/// Validation includes:
/// - initial-state and per-step invariant checks
/// - command cardinality and command/state coherence
/// - event-specific transition semantics against the previous state
/// - final-state consistency with the recorded steps
pub fn validate_trace(result: &TraceResult) -> Result<(), TraceValidationError> {
    validate_invariants(&result.initial_state)
        .map_err(|source| TraceValidationError::InitialInvariant { source })?;

    let mut previous_state = result.initial_state.clone();

    for (step_index, step) in result.steps.iter().enumerate() {
        validate_invariants(&step.state)
            .map_err(|source| TraceValidationError::Invariant { step_index, source })?;

        if step.commands.len() > 1 {
            return Err(TraceValidationError::TooManyCommands {
                step_index,
                command_count: step.commands.len(),
            });
        }

        if let Some(command) = step.commands.first() {
            match command {
                Command::TransitionRejected { .. } => {
                    if step.state != previous_state {
                        return Err(TraceValidationError::RejectionMutatedState { step_index });
                    }
                }
                Command::Dispatch(issue_id) => {
                    if !step.state.claimed.contains(issue_id)
                        || !step.state.running.contains_key(issue_id)
                    {
                        return Err(TraceValidationError::DispatchWithoutRunning {
                            step_index,
                            issue_id: issue_id.0.clone(),
                        });
                    }
                }
                Command::ScheduleRetry { issue_id, attempt } => {
                    let retry_matches = step
                        .state
                        .retry_attempts
                        .get(issue_id)
                        .is_some_and(|entry| entry.attempt == *attempt);
                    if !step.state.claimed.contains(issue_id)
                        || step.state.running.contains_key(issue_id)
                        || !retry_matches
                    {
                        return Err(TraceValidationError::RetryCommandMismatch {
                            step_index,
                            issue_id: issue_id.0.clone(),
                            attempt: *attempt,
                        });
                    }
                }
                Command::ReleaseClaim(issue_id) => {
                    if step.state.claimed.contains(issue_id)
                        || step.state.running.contains_key(issue_id)
                        || step.state.retry_attempts.contains_key(issue_id)
                    {
                        return Err(TraceValidationError::ReleaseCommandMismatch {
                            step_index,
                            issue_id: issue_id.0.clone(),
                        });
                    }
                }
            }
        }

        validate_step_contract(step_index, &previous_state, step)?;
        previous_state = step.state.clone();
    }

    let expected_final_state = result
        .steps
        .last()
        .map(|step| &step.state)
        .unwrap_or(&result.initial_state);

    if expected_final_state != &result.final_state {
        return Err(TraceValidationError::FinalStateMismatch);
    }

    Ok(())
}

fn validate_step_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
) -> Result<(), TraceValidationError> {
    match &step.event {
        Event::Claim(issue_id) => {
            validate_claim_contract(step_index, previous_state, step, issue_id)
        }
        Event::MarkRunning(issue_id) => {
            validate_mark_running_contract(step_index, previous_state, step, issue_id)
        }
        Event::UpdateAgent(update) => {
            validate_update_agent_contract(step_index, previous_state, step, &update.issue_id)
        }
        Event::QueueRetry { issue_id, attempt } => {
            validate_queue_retry_contract(step_index, previous_state, step, issue_id, *attempt)
        }
        Event::Release(issue_id) => {
            validate_release_contract(step_index, previous_state, step, issue_id)
        }
    }
}

fn validate_claim_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
) -> Result<(), TraceValidationError> {
    if previous_state.claimed.contains(issue_id) {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::AlreadyClaimed,
        );
    }

    if !step.commands.is_empty() {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful Claim must not emit commands",
        ));
    }

    let mut expected_claimed = previous_state.claimed.clone();
    expected_claimed.insert(issue_id.clone());
    if step.state.claimed != expected_claimed
        || step.state.running != previous_state.running
        || step.state.retry_attempts != previous_state.retry_attempts
        || step.state.codex_totals != previous_state.codex_totals
        || step.state.codex_rate_limits != previous_state.codex_rate_limits
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful Claim must add exactly one claim and leave all other state unchanged",
        ));
    }

    Ok(())
}

fn validate_mark_running_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
) -> Result<(), TraceValidationError> {
    if !previous_state.claimed.contains(issue_id) {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::MissingClaim,
        );
    }

    if previous_state.running.contains_key(issue_id) {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::AlreadyRunning,
        );
    }

    let mut expected_running = previous_state.running.clone();
    expected_running.insert(issue_id.clone(), RunningEntry::default());

    let mut expected_retry_attempts = previous_state.retry_attempts.clone();
    expected_retry_attempts.remove(issue_id);

    if step.commands != vec![Command::Dispatch(issue_id.clone())]
        || step.state.claimed != previous_state.claimed
        || step.state.running != expected_running
        || step.state.retry_attempts != expected_retry_attempts
        || step.state.codex_totals != previous_state.codex_totals
        || step.state.codex_rate_limits != previous_state.codex_rate_limits
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful MarkRunning must dispatch once, add a default running entry, and clear any retry entry for the same issue",
        ));
    }

    Ok(())
}

fn validate_update_agent_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
) -> Result<(), TraceValidationError> {
    if !previous_state.running.contains_key(issue_id) {
        // UpdateAgent requires the issue to be running. If not claimed at all, it's MissingClaim.
        // If claimed but not running (e.g., in retry queue), it's RunningWithoutClaim.
        let expected_rejection = if previous_state.claimed.contains(issue_id) {
            TransitionRejection::RunningWithoutClaim
        } else {
            TransitionRejection::MissingClaim
        };
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            expected_rejection,
        );
    }

    let totals_are_monotonic = step.state.codex_totals.input_tokens
        >= previous_state.codex_totals.input_tokens
        && step.state.codex_totals.output_tokens >= previous_state.codex_totals.output_tokens
        && step.state.codex_totals.total_tokens >= previous_state.codex_totals.total_tokens;
    let running_topology_is_stable = step.state.running.len() == previous_state.running.len()
        && previous_state
            .running
            .keys()
            .all(|running_issue_id| step.state.running.contains_key(running_issue_id));

    if !step.commands.is_empty()
        || step.state.claimed != previous_state.claimed
        || step.state.retry_attempts != previous_state.retry_attempts
        || !running_topology_is_stable
        || !step.state.running.contains_key(issue_id)
        || !totals_are_monotonic
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful UpdateAgent must preserve topology and keep aggregate token counters monotonic",
        ));
    }

    Ok(())
}

fn validate_queue_retry_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
    attempt: u32,
) -> Result<(), TraceValidationError> {
    if !previous_state.claimed.contains(issue_id) {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::MissingClaim,
        );
    }

    if attempt == 0 {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::InvalidRetryAttempt,
        );
    }

    if previous_state
        .retry_attempts
        .get(issue_id)
        .is_some_and(|entry| entry.attempt >= attempt)
    {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::RetryAttemptRegression,
        );
    }

    let mut expected_running = previous_state.running.clone();
    expected_running.remove(issue_id);

    let mut expected_retry_attempts = previous_state.retry_attempts.clone();
    expected_retry_attempts.insert(
        issue_id.clone(),
        RetryEntry {
            attempt,
            ..RetryEntry::default()
        },
    );

    if step.commands
        != vec![Command::ScheduleRetry {
            issue_id: issue_id.clone(),
            attempt,
        }]
        || step.state.claimed != previous_state.claimed
        || step.state.running != expected_running
        || step.state.retry_attempts != expected_retry_attempts
        || step.state.codex_totals != previous_state.codex_totals
        || step.state.codex_rate_limits != previous_state.codex_rate_limits
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful QueueRetry must preserve claims, drop any running entry, and store the new retry attempt",
        ));
    }

    Ok(())
}

fn validate_release_contract(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
) -> Result<(), TraceValidationError> {
    if !previous_state.claimed.contains(issue_id) {
        return expect_rejection(
            step_index,
            previous_state,
            step,
            issue_id,
            TransitionRejection::MissingClaim,
        );
    }

    let mut expected_claimed = previous_state.claimed.clone();
    expected_claimed.remove(issue_id);

    let mut expected_running = previous_state.running.clone();
    expected_running.remove(issue_id);

    let mut expected_retry_attempts = previous_state.retry_attempts.clone();
    expected_retry_attempts.remove(issue_id);

    if step.commands != vec![Command::ReleaseClaim(issue_id.clone())]
        || step.state.claimed != expected_claimed
        || step.state.running != expected_running
        || step.state.retry_attempts != expected_retry_attempts
        || step.state.codex_totals != previous_state.codex_totals
        || step.state.codex_rate_limits != previous_state.codex_rate_limits
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "successful Release must clear claim, running, and retry state for the issue only",
        ));
    }

    Ok(())
}

fn expect_rejection(
    step_index: usize,
    previous_state: &OrchestratorState,
    step: &TraceStep,
    issue_id: &IssueId,
    reason: TransitionRejection,
) -> Result<(), TraceValidationError> {
    if step.commands
        != vec![Command::TransitionRejected {
            issue_id: issue_id.clone(),
            reason,
        }]
    {
        return Err(transition_contract_error(
            step_index,
            &step.event,
            "invalid transition must emit the expected rejection command",
        ));
    }

    if step.state != *previous_state {
        return Err(TraceValidationError::RejectionMutatedState { step_index });
    }

    Ok(())
}

fn transition_contract_error(
    step_index: usize,
    event: &Event,
    detail: impl Into<String>,
) -> TraceValidationError {
    TraceValidationError::TransitionContract {
        step_index,
        event_kind: event_kind(event),
        detail: detail.into(),
    }
}

fn event_kind(event: &Event) -> &'static str {
    match event {
        Event::Claim(_) => "Claim",
        Event::MarkRunning(_) => "MarkRunning",
        Event::UpdateAgent(_) => "UpdateAgent",
        Event::QueueRetry { .. } => "QueueRetry",
        Event::Release(_) => "Release",
    }
}

pub fn command_count(result: &TraceResult) -> usize {
    result.steps.iter().map(|step| step.commands.len()).sum()
}

pub fn max_active_counts(result: &TraceResult) -> (usize, usize) {
    let mut max_claimed = 0;
    let mut max_running = 0;

    for step in &result.steps {
        max_claimed = max_claimed.max(step.state.claimed.len());
        max_running = max_running.max(step.state.running.len());
    }

    (max_claimed, max_running)
}

pub fn snapshot_state(state: &OrchestratorState) -> StateSnapshot {
    let mut claimed = state
        .claimed
        .iter()
        .map(|id| id.0.clone())
        .collect::<Vec<_>>();
    claimed.sort();

    let mut running = state
        .running
        .keys()
        .map(|id| id.0.clone())
        .collect::<Vec<_>>();
    running.sort();

    let mut retry_attempts = state
        .retry_attempts
        .iter()
        .map(|(id, entry)| (id.0.clone(), entry.attempt))
        .collect::<Vec<_>>();
    retry_attempts.sort_by(|left, right| left.0.cmp(&right.0));

    StateSnapshot {
        claimed,
        running,
        retry_attempts,
    }
}

pub fn snapshot_trace(result: &TraceResult) -> Vec<StateSnapshot> {
    result
        .steps
        .iter()
        .map(|step| snapshot_state(&step.state))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{issue_id, orchestrator_state_from_labels};

    #[test]
    fn trace_runner_reports_command_count() {
        let issue = issue_id("SYM-1");
        let trace = run_trace(vec![Event::Claim(issue.clone()), Event::Release(issue)]);
        assert_eq!(command_count(&trace), 1);
        assert_eq!(validate_trace(&trace), Ok(()));
    }

    #[test]
    fn snapshot_normalizes_set_order() {
        let state =
            orchestrator_state_from_labels(&["SYM-2", "SYM-1"], &["SYM-2"], &[("SYM-3", 2)]);
        let snapshot = snapshot_state(&state);
        assert_eq!(
            snapshot.claimed,
            vec!["SYM-1".to_owned(), "SYM-2".to_owned(), "SYM-3".to_owned()]
        );
    }

    #[test]
    fn validate_trace_rejects_invalid_initial_state() {
        let mut invalid_state = OrchestratorState::default();
        invalid_state
            .running
            .insert(issue_id("SYM-INIT"), RunningEntry::default());

        let trace = TraceResult {
            initial_state: invalid_state,
            steps: Vec::new(),
            final_state: OrchestratorState::default(),
        };

        assert_eq!(
            validate_trace(&trace),
            Err(TraceValidationError::InitialInvariant {
                source: InvariantError::RunningWithoutClaim,
            }),
        );
    }

    #[test]
    fn validate_trace_rejects_empty_trace_with_mismatched_final_state() {
        let mut final_state = OrchestratorState::default();
        final_state.claimed.insert(issue_id("SYM-FINAL"));

        let trace = TraceResult {
            initial_state: OrchestratorState::default(),
            steps: Vec::new(),
            final_state,
        };

        assert_eq!(
            validate_trace(&trace),
            Err(TraceValidationError::FinalStateMismatch),
        );
    }

    #[test]
    fn validate_trace_rejects_state_mutation_after_rejection() {
        let issue = issue_id("SYM-BAD");
        let mut mutated = OrchestratorState::default();
        mutated.claimed.insert(issue.clone());

        let trace = TraceResult {
            initial_state: OrchestratorState::default(),
            steps: vec![TraceStep {
                event: Event::MarkRunning(issue.clone()),
                commands: vec![Command::TransitionRejected {
                    issue_id: issue,
                    reason: TransitionRejection::MissingClaim,
                }],
                state: mutated.clone(),
            }],
            final_state: mutated,
        };

        assert_eq!(
            validate_trace(&trace),
            Err(TraceValidationError::RejectionMutatedState { step_index: 0 }),
        );
    }

    #[test]
    fn validate_trace_rejects_retry_command_without_retry_state() {
        let issue = issue_id("SYM-RETRY");
        let trace = TraceResult {
            initial_state: orchestrator_state_from_labels(&["SYM-RETRY"], &["SYM-RETRY"], &[]),
            steps: vec![TraceStep {
                event: Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: 2,
                },
                commands: vec![Command::ScheduleRetry {
                    issue_id: issue.clone(),
                    attempt: 2,
                }],
                state: orchestrator_state_from_labels(&["SYM-RETRY"], &["SYM-RETRY"], &[]),
            }],
            final_state: orchestrator_state_from_labels(&["SYM-RETRY"], &["SYM-RETRY"], &[]),
        };

        assert_eq!(
            validate_trace(&trace),
            Err(TraceValidationError::RetryCommandMismatch {
                step_index: 0,
                issue_id: issue.0,
                attempt: 2,
            }),
        );
    }

    #[test]
    fn validate_trace_rejects_claim_transition_contract_violation() {
        let issue = issue_id("SYM-CLAIM");
        let trace = TraceResult {
            initial_state: OrchestratorState::default(),
            steps: vec![TraceStep {
                event: Event::Claim(issue.clone()),
                commands: Vec::new(),
                state: OrchestratorState::default(),
            }],
            final_state: OrchestratorState::default(),
        };

        assert_eq!(
            validate_trace(&trace),
            Err(TraceValidationError::TransitionContract {
                step_index: 0,
                event_kind: "Claim",
                detail: "successful Claim must add exactly one claim and leave all other state unchanged".to_owned(),
            }),
        );
    }
}
