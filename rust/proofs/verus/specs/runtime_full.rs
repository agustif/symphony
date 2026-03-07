// Verus full proof suite for reducer transition chains.

#![allow(unused_imports)]

use vstd::prelude::*;

mod runtime_quick;
use runtime_quick::*;

verus! {

pub open spec fn available_dispatch_slots(max_concurrent: nat, running_count: nat) -> nat {
    if running_count < max_concurrent {
        (max_concurrent - running_count) as nat
    } else {
        0
    }
}

pub proof fn lemma_available_dispatch_slots_are_bounded(
    max_concurrent: nat,
    running_count: nat,
)
    ensures
        0 <= available_dispatch_slots(max_concurrent, running_count),
        available_dispatch_slots(max_concurrent, running_count) <= max_concurrent,
{
}

pub proof fn lemma_available_dispatch_slots_zero_when_saturated(
    max_concurrent: nat,
    running_count: nat,
)
    requires
        running_count >= max_concurrent,
    ensures
        available_dispatch_slots(max_concurrent, running_count) == 0,
{
}

pub proof fn lemma_available_dispatch_slots_reconstruct_capacity_when_available(
    max_concurrent: nat,
    running_count: nat,
)
    requires
        running_count < max_concurrent,
    ensures
        available_dispatch_slots(max_concurrent, running_count) + running_count
            == max_concurrent,
{
}

pub open spec fn claim_then_mark_running(
    state: OrchestratorState,
    issue_id: IssueId,
) -> OrchestratorState {
    apply_event(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id))
}

pub open spec fn claim_then_mark_running_commands(
    state: OrchestratorState,
    issue_id: IssueId,
) -> Seq<Command> {
    emitted_commands(state, Event::Claim(issue_id))
        + emitted_commands(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id))
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
    lemma_claim_emits_no_commands_on_valid_transition(state, issue_id);
    lemma_mark_running_emits_dispatch_on_valid_transition(
        apply_event(state, Event::Claim(issue_id)),
        issue_id,
    );
}

pub proof fn lemma_claim_then_mark_running_emits_single_dispatch(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
    ensures
        claim_then_mark_running_commands(state, issue_id)
            == seq![Command::Dispatch(issue_id)],
{
    lemma_claim_emits_no_commands_on_valid_transition(state, issue_id);
    lemma_mark_running_emits_dispatch_on_valid_transition(
        apply_event(state, Event::Claim(issue_id)),
        issue_id,
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
        orchestrator_invariants(queue_retry_from_state(state, issue_id, next_attempt)),
{
    lemma_queue_retry_regression_is_noop_base(state, issue_id, next_attempt);
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

pub open spec fn release_after_retry_commands(
    state: OrchestratorState,
    issue_id: IssueId,
    attempt: nat,
) -> Seq<Command> {
    emitted_commands(state, Event::QueueRetry { issue_id, attempt })
        + emitted_commands(queue_retry_from_state(state, issue_id, attempt), Event::Release(issue_id))
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
    lemma_queue_retry_emits_schedule_retry_on_valid_transition(state, issue_id, attempt);
    lemma_release_emits_release_claim_on_valid_transition(
        queue_retry_from_state(state, issue_id, attempt),
        issue_id,
    );
}

pub proof fn lemma_release_after_retry_emits_retry_then_release_commands(
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
        release_after_retry_commands(state, issue_id, attempt)
            == seq![
                Command::ScheduleRetry { issue_id, attempt },
                Command::ReleaseClaim(issue_id),
            ],
{
    lemma_queue_retry_emits_schedule_retry_on_valid_transition(state, issue_id, attempt);
    lemma_release_emits_release_claim_on_valid_transition(
        queue_retry_from_state(state, issue_id, attempt),
        issue_id,
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
        orchestrator_invariants(apply_event(state, Event::Release(issue_id))),
{
    lemma_release_without_claim_is_noop_base(state, issue_id);
}

pub proof fn lemma_rejection_cases_are_state_preserving(
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
    lemma_invalid_transition_preserves_invariants(state, event);
}

pub proof fn lemma_valid_claim_then_mark_running_never_hits_rejection_case(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
    ensures
        !is_rejection_case(state, Event::Claim(issue_id)),
        !is_rejection_case(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id)),
{
}

pub proof fn lemma_valid_release_after_retry_never_hits_rejection_case(
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
        !is_rejection_case(state, Event::QueueRetry { issue_id, attempt }),
        !is_rejection_case(queue_retry_from_state(state, issue_id, attempt), Event::Release(issue_id)),
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
        emitted_commands(state, Event::MarkRunning(issue_id))
            == seq![Command::Dispatch(issue_id)],
{
    lemma_mark_running_emits_dispatch_on_valid_transition(state, issue_id);
}

fn main() {}

} // verus!
