// Verus quick proof suite for core Symphony reducer invariants.
// Model aligns with rust/crates/symphony-domain/src/lib.rs reducer semantics.

#![allow(unused_imports)]

use vstd::{map::*, prelude::*, set::*};

verus! {

pub type IssueId = int;

pub struct RunningEntry {}

pub struct RetryEntry {
    pub attempt: nat,
}

pub struct OrchestratorState {
    pub claimed: Set<IssueId>,
    pub running: Map<IssueId, RunningEntry>,
    pub retry_attempts: Map<IssueId, RetryEntry>,
}

pub enum Event {
    Claim(IssueId),
    MarkRunning(IssueId),
    QueueRetry { issue_id: IssueId, attempt: nat },
    Release(IssueId),
}

pub open spec fn orchestrator_invariants(state: OrchestratorState) -> bool {
    &&& forall|id: IssueId| state.running.dom().contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId| state.retry_attempts.dom().contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId|
            !(state.running.dom().contains(id) && state.retry_attempts.dom().contains(id))
    &&& forall|id: IssueId|
            state.retry_attempts.dom().contains(id) ==> state.retry_attempts[id].attempt > 0
}

pub open spec fn apply_event(state: OrchestratorState, event: Event) -> OrchestratorState {
    match event {
        Event::Claim(issue_id) => {
            if state.claimed.contains(issue_id) {
                state
            } else {
                OrchestratorState {
                    claimed: state.claimed.insert(issue_id),
                    running: state.running,
                    retry_attempts: state.retry_attempts,
                }
            }
        },
        Event::MarkRunning(issue_id) => {
            if !state.claimed.contains(issue_id) || state.running.dom().contains(issue_id) {
                state
            } else {
                OrchestratorState {
                    claimed: state.claimed,
                    running: state.running.insert(issue_id, RunningEntry {}),
                    retry_attempts: state.retry_attempts.remove(issue_id),
                }
            }
        },
        Event::QueueRetry { issue_id, attempt } => {
            if !state.claimed.contains(issue_id)
                || attempt == 0
                || (state.retry_attempts.dom().contains(issue_id)
                    && state.retry_attempts[issue_id].attempt >= attempt)
            {
                state
            } else {
                OrchestratorState {
                    claimed: state.claimed,
                    running: state.running.remove(issue_id),
                    retry_attempts: state.retry_attempts.insert(issue_id, RetryEntry { attempt }),
                }
            }
        },
        Event::Release(issue_id) => {
            if !state.claimed.contains(issue_id) {
                state
            } else {
                OrchestratorState {
                    claimed: state.claimed.remove(issue_id),
                    running: state.running.remove(issue_id),
                    retry_attempts: state.retry_attempts.remove(issue_id),
                }
            }
        },
    }
}

pub open spec fn is_rejection_case(state: OrchestratorState, event: Event) -> bool {
    match event {
        Event::Claim(issue_id) => state.claimed.contains(issue_id),
        Event::MarkRunning(issue_id) => {
            !state.claimed.contains(issue_id) || state.running.dom().contains(issue_id)
        },
        Event::QueueRetry { issue_id, attempt } => {
            !state.claimed.contains(issue_id)
                || attempt == 0
                || (state.retry_attempts.dom().contains(issue_id)
                    && state.retry_attempts[issue_id].attempt >= attempt)
        },
        Event::Release(issue_id) => !state.claimed.contains(issue_id),
    }
}

pub proof fn lemma_claim_preserves_invariants(state: OrchestratorState, issue_id: IssueId)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, Event::Claim(issue_id))),
{
}

pub proof fn lemma_mark_running_preserves_invariants(state: OrchestratorState, issue_id: IssueId)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, Event::MarkRunning(issue_id))),
{
}

pub proof fn lemma_queue_retry_preserves_invariants(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, Event::QueueRetry { issue_id, attempt })),
{
}

pub proof fn lemma_release_preserves_invariants(state: OrchestratorState, issue_id: IssueId)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, Event::Release(issue_id))),
{
}

pub proof fn lemma_apply_event_preserves_invariants(state: OrchestratorState, event: Event)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, event)),
{
    match event {
        Event::Claim(issue_id) => {
            lemma_claim_preserves_invariants(state, issue_id);
        },
        Event::MarkRunning(issue_id) => {
            lemma_mark_running_preserves_invariants(state, issue_id);
        },
        Event::QueueRetry { issue_id, attempt } => {
            lemma_queue_retry_preserves_invariants(state, issue_id, attempt);
        },
        Event::Release(issue_id) => {
            lemma_release_preserves_invariants(state, issue_id);
        },
    }
}

pub proof fn lemma_mark_running_without_claim_is_noop(state: OrchestratorState, issue_id: IssueId)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
    ensures
        apply_event(state, Event::MarkRunning(issue_id)) == state,
{
}

pub proof fn lemma_claim_on_claimed_issue_is_noop(state: OrchestratorState, issue_id: IssueId)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
    ensures
        apply_event(state, Event::Claim(issue_id)) == state,
{
}

pub proof fn lemma_mark_running_on_running_issue_is_noop(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(issue_id),
    ensures
        apply_event(state, Event::MarkRunning(issue_id)) == state,
{
}

pub proof fn lemma_release_clears_tracking_for_claimed_issue(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
    ensures
        !apply_event(state, Event::Release(issue_id)).claimed.contains(issue_id),
        !apply_event(state, Event::Release(issue_id)).running.dom().contains(issue_id),
        !apply_event(state, Event::Release(issue_id)).retry_attempts.dom().contains(issue_id),
{
}

pub proof fn lemma_queue_retry_requires_positive_attempt(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
    ensures
        apply_event(state, Event::QueueRetry { issue_id, attempt: 0 }) == state,
{
}

pub proof fn lemma_queue_retry_without_claim_is_noop(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
    ensures
        apply_event(state, Event::QueueRetry { issue_id, attempt }) == state,
{
}

pub proof fn lemma_queue_retry_regression_is_noop_base(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
)
    requires
        orchestrator_invariants(state),
        state.retry_attempts.dom().contains(issue_id),
        attempt <= state.retry_attempts[issue_id].attempt,
    ensures
        apply_event(state, Event::QueueRetry { issue_id, attempt }) == state,
{
}

pub proof fn lemma_release_without_claim_is_noop_base(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
    ensures
        apply_event(state, Event::Release(issue_id)) == state,
{
}

pub proof fn lemma_invalid_transition_preserves_invariants(
    state: OrchestratorState,
    event: Event,
)
    requires
        orchestrator_invariants(state),
        is_rejection_case(state, event),
    ensures
        apply_event(state, event) == state,
        orchestrator_invariants(apply_event(state, event)),
{
    match event {
        Event::Claim(issue_id) => {
            lemma_claim_on_claimed_issue_is_noop(state, issue_id);
        },
        Event::MarkRunning(issue_id) => {
            if !state.claimed.contains(issue_id) {
                lemma_mark_running_without_claim_is_noop(state, issue_id);
            } else {
                lemma_mark_running_on_running_issue_is_noop(state, issue_id);
            }
        },
        Event::QueueRetry { issue_id, attempt } => {
            if !state.claimed.contains(issue_id) {
                lemma_queue_retry_without_claim_is_noop(state, issue_id, attempt);
            } else if attempt == 0 {
                lemma_queue_retry_requires_positive_attempt(state, issue_id);
            } else {
                lemma_queue_retry_regression_is_noop_base(state, issue_id, attempt);
            }
        },
        Event::Release(issue_id) => {
            lemma_release_without_claim_is_noop_base(state, issue_id);
        },
    }
}

fn main() {}

} // verus!
