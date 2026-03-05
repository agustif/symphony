// Verus full proof suite for reducer transition chains.

#![allow(unused_imports)]

use vstd::prelude::*;

mod runtime_quick;
use runtime_quick::*;

verus! {

pub open spec fn claim_then_mark_running(
    state: OrchestratorState,
    issue_id: IssueId,
) -> OrchestratorState {
    apply_event(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id))
}

pub open spec fn queue_retry_from_state(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
) -> OrchestratorState {
    apply_event(state, Event::QueueRetry { issue_id, attempt })
}

pub proof fn lemma_claim_then_mark_running_establishes_running(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
    ensures
        orchestrator_invariants(claim_then_mark_running(state, issue_id)),
        claim_then_mark_running(state, issue_id).claimed.contains(issue_id),
        claim_then_mark_running(state, issue_id).running.dom().contains(issue_id),
        !claim_then_mark_running(state, issue_id).retry_attempts.dom().contains(issue_id),
{
    lemma_apply_event_preserves_invariants(state, Event::Claim(issue_id));
    lemma_apply_event_preserves_invariants(
        apply_event(state, Event::Claim(issue_id)),
        Event::MarkRunning(issue_id),
    );
}

pub proof fn lemma_retry_regression_is_noop(
    state: OrchestratorState,
    issue_id: IssueId,
    next_attempt: nat,
)
    requires
        orchestrator_invariants(state),
        state.retry_attempts.dom().contains(issue_id),
        next_attempt <= state.retry_attempts[issue_id].attempt,
    ensures
        queue_retry_from_state(state, issue_id, next_attempt) == state,
{
}

pub proof fn lemma_retry_with_higher_attempt_replaces_entry(
    state: OrchestratorState,
    issue_id: IssueId,
    next_attempt: nat,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        next_attempt > 0,
        state.retry_attempts.dom().contains(issue_id) ==> state.retry_attempts[issue_id].attempt < next_attempt,
    ensures
        orchestrator_invariants(queue_retry_from_state(state, issue_id, next_attempt)),
        queue_retry_from_state(state, issue_id, next_attempt).retry_attempts.dom().contains(issue_id),
        queue_retry_from_state(state, issue_id, next_attempt).retry_attempts[issue_id].attempt == next_attempt,
        !queue_retry_from_state(state, issue_id, next_attempt).running.dom().contains(issue_id),
{
    lemma_apply_event_preserves_invariants(
        state,
        Event::QueueRetry {
            issue_id,
            attempt: next_attempt,
        },
    );
}

pub open spec fn release_after_retry(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
) -> OrchestratorState {
    apply_event(queue_retry_from_state(state, issue_id, attempt), Event::Release(issue_id))
}

pub proof fn lemma_release_after_retry_clears_all(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        attempt > 0,
        state.retry_attempts.dom().contains(issue_id) ==> state.retry_attempts[issue_id].attempt < attempt,
    ensures
        orchestrator_invariants(release_after_retry(state, issue_id, attempt)),
        !release_after_retry(state, issue_id, attempt).claimed.contains(issue_id),
        !release_after_retry(state, issue_id, attempt).running.dom().contains(issue_id),
        !release_after_retry(state, issue_id, attempt).retry_attempts.dom().contains(issue_id),
{
    lemma_retry_with_higher_attempt_replaces_entry(state, issue_id, attempt);
    lemma_apply_event_preserves_invariants(
        queue_retry_from_state(state, issue_id, attempt),
        Event::Release(issue_id),
    );
}

pub proof fn lemma_release_on_unclaimed_issue_is_noop(
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

pub proof fn lemma_mark_running_clears_retry_entry(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
        state.retry_attempts.dom().contains(issue_id),
    ensures
        apply_event(state, Event::MarkRunning(issue_id)).running.dom().contains(issue_id),
        !apply_event(state, Event::MarkRunning(issue_id)).retry_attempts.dom().contains(issue_id),
{
}

fn main() {}

} // verus!
