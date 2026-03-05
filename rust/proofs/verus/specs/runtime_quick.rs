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

fn main() {}

} // verus!
