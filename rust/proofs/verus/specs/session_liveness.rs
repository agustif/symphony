// Verus progress obligations for the Symphony reducer.
// These lemmas capture one-step liveness obligations used by the runtime loop.

#![allow(unused_imports)]

use vstd::prelude::*;

mod runtime_quick;
use runtime_quick::*;

verus! {

pub open spec fn dispatch_candidate(state: OrchestratorState, issue_id: IssueId) -> bool {
    &&& orchestrator_invariants(state)
    &&& !state.claimed.contains(issue_id)
    &&& !state.running.dom().contains(issue_id)
}

pub open spec fn dispatch_progress(state: OrchestratorState, issue_id: IssueId) -> OrchestratorState {
    apply_event(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id))
}

pub open spec fn dispatch_progress_commands(
    state: OrchestratorState,
    issue_id: IssueId,
) -> Seq<Command> {
    emitted_commands(state, Event::Claim(issue_id))
        + emitted_commands(apply_event(state, Event::Claim(issue_id)), Event::MarkRunning(issue_id))
}

pub open spec fn fair_scheduler_dispatch_assumption(
    state: OrchestratorState,
    issue_id: IssueId,
) -> bool {
    dispatch_candidate(state, issue_id)
}

pub proof fn lemma_dispatch_candidate_reaches_running(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        dispatch_candidate(state, issue_id),
    ensures
        orchestrator_invariants(dispatch_progress(state, issue_id)),
        dispatch_progress(state, issue_id).claimed.contains(issue_id),
        dispatch_progress(state, issue_id).running.dom().contains(issue_id),
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

pub proof fn lemma_fair_scheduler_dispatch_step_emits_dispatch(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        fair_scheduler_dispatch_assumption(state, issue_id),
    ensures
        dispatch_progress_commands(state, issue_id) == seq![Command::Dispatch(issue_id)],
        dispatch_progress(state, issue_id).running.dom().contains(issue_id),
{
    lemma_dispatch_candidate_reaches_running(state, issue_id);
    lemma_claim_emits_no_commands_on_valid_transition(state, issue_id);
    lemma_mark_running_emits_dispatch_on_valid_transition(
        apply_event(state, Event::Claim(issue_id)),
        issue_id,
    );
}

pub open spec fn normal_exit_progress(
    state: OrchestratorState,
    issue_id: IssueId,
) -> OrchestratorState {
    apply_event(state, Event::QueueRetry { issue_id, attempt: 1 })
}

pub proof fn lemma_normal_exit_moves_running_to_retry(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        state.running.dom().contains(issue_id),
    ensures
        orchestrator_invariants(normal_exit_progress(state, issue_id)),
        !normal_exit_progress(state, issue_id).running.dom().contains(issue_id),
        normal_exit_progress(state, issue_id).retry_attempts.dom().contains(issue_id),
        normal_exit_progress(state, issue_id).retry_attempts[issue_id].attempt == 1,
{
    lemma_apply_event_preserves_invariants(
        state,
        Event::QueueRetry {
            issue_id,
            attempt: 1,
        },
    );
}

pub open spec fn failure_exit_progress(
    state: OrchestratorState,
    issue_id: IssueId,
    next_attempt: nat,
) -> OrchestratorState {
    apply_event(state, Event::QueueRetry { issue_id, attempt: next_attempt })
}

pub open spec fn fair_failure_retry_assumption(
    state: OrchestratorState,
    issue_id: IssueId,
    next_attempt: nat,
) -> bool {
    &&& orchestrator_invariants(state)
    &&& state.claimed.contains(issue_id)
    &&& next_attempt > 0
    &&& state.retry_attempts.dom().contains(issue_id) ==> state.retry_attempts[issue_id].attempt < next_attempt
}

pub proof fn lemma_failure_exit_increments_retry_attempt(
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
        orchestrator_invariants(failure_exit_progress(state, issue_id, next_attempt)),
        failure_exit_progress(state, issue_id, next_attempt).retry_attempts.dom().contains(issue_id),
        failure_exit_progress(state, issue_id, next_attempt).retry_attempts[issue_id].attempt == next_attempt,
{
    lemma_apply_event_preserves_invariants(
        state,
        Event::QueueRetry {
            issue_id,
            attempt: next_attempt,
        },
    );
    lemma_queue_retry_emits_schedule_retry_on_valid_transition(state, issue_id, next_attempt);
}

pub proof fn lemma_fair_failure_exit_emits_schedule_retry(
    state: OrchestratorState,
    issue_id: IssueId,
    next_attempt: nat,
)
    requires
        fair_failure_retry_assumption(state, issue_id, next_attempt),
    ensures
        emitted_commands(state, Event::QueueRetry { issue_id, attempt: next_attempt })
            == seq![Command::ScheduleRetry { issue_id, attempt: next_attempt }],
        failure_exit_progress(state, issue_id, next_attempt).retry_attempts.dom().contains(issue_id),
{
    lemma_failure_exit_increments_retry_attempt(state, issue_id, next_attempt);
    lemma_queue_retry_emits_schedule_retry_on_valid_transition(state, issue_id, next_attempt);
}

pub open spec fn retry_timer_progress(state: OrchestratorState, issue_id: IssueId) -> OrchestratorState {
    apply_event(state, Event::MarkRunning(issue_id))
}

pub open spec fn fair_retry_service_assumption(
    state: OrchestratorState,
    issue_id: IssueId,
) -> bool {
    &&& orchestrator_invariants(state)
    &&& state.claimed.contains(issue_id)
    &&& !state.running.dom().contains(issue_id)
    &&& state.retry_attempts.dom().contains(issue_id)
    &&& state.retry_attempts[issue_id].attempt > 0
}

pub proof fn lemma_retry_timer_dispatches_claimed_issue(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
        state.retry_attempts.dom().contains(issue_id),
        state.retry_attempts[issue_id].attempt > 0,
    ensures
        orchestrator_invariants(retry_timer_progress(state, issue_id)),
        retry_timer_progress(state, issue_id).running.dom().contains(issue_id),
        !retry_timer_progress(state, issue_id).retry_attempts.dom().contains(issue_id),
{
    lemma_apply_event_preserves_invariants(state, Event::MarkRunning(issue_id));
    lemma_mark_running_emits_dispatch_on_valid_transition(state, issue_id);
}

pub proof fn lemma_fair_retry_service_step_emits_dispatch(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        fair_retry_service_assumption(state, issue_id),
    ensures
        emitted_commands(state, Event::MarkRunning(issue_id))
            == seq![Command::Dispatch(issue_id)],
        retry_timer_progress(state, issue_id).running.dom().contains(issue_id),
{
    lemma_retry_timer_dispatches_claimed_issue(state, issue_id);
    lemma_mark_running_emits_dispatch_on_valid_transition(state, issue_id);
}

pub open spec fn terminal_release_progress(
    state: OrchestratorState,
    issue_id: IssueId,
) -> OrchestratorState {
    apply_event(state, Event::Release(issue_id))
}

pub open spec fn fair_terminal_reconciliation_assumption(
    state: OrchestratorState,
    issue_id: IssueId,
) -> bool {
    &&& orchestrator_invariants(state)
    &&& state.claimed.contains(issue_id)
}

pub proof fn lemma_terminal_release_clears_claim(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
    ensures
        orchestrator_invariants(terminal_release_progress(state, issue_id)),
        !terminal_release_progress(state, issue_id).claimed.contains(issue_id),
        !terminal_release_progress(state, issue_id).running.dom().contains(issue_id),
        !terminal_release_progress(state, issue_id).retry_attempts.dom().contains(issue_id),
{
    lemma_apply_event_preserves_invariants(state, Event::Release(issue_id));
    lemma_release_emits_release_claim_on_valid_transition(state, issue_id);
}

pub proof fn lemma_fair_terminal_reconciliation_emits_release_claim(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        fair_terminal_reconciliation_assumption(state, issue_id),
    ensures
        emitted_commands(state, Event::Release(issue_id))
            == seq![Command::ReleaseClaim(issue_id)],
        !terminal_release_progress(state, issue_id).claimed.contains(issue_id),
{
    lemma_terminal_release_clears_claim(state, issue_id);
    lemma_release_emits_release_claim_on_valid_transition(state, issue_id);
}

fn main() {}

} // verus!
