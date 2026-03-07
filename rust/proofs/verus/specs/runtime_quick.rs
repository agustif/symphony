// Verus quick proof suite for core Symphony reducer invariants.
// Model aligns with rust/crates/symphony-domain/src/lib.rs reducer semantics.

#![allow(unused_imports)]

use vstd::{map::*, prelude::*, seq::*, set::*};

verus! {

pub type IssueId = int;

pub struct Usage {
    pub input_tokens: nat,
    pub output_tokens: nat,
    pub total_tokens: nat,
}

pub struct RunningEntry {
    pub turn_count: nat,
    pub started_at: Option<nat>,
    pub usage: Usage,
}

pub struct CodexTotals {
    pub input_tokens: nat,
    pub output_tokens: nat,
    pub total_tokens: nat,
}

pub struct RetryEntry {
    pub attempt: nat,
}

pub struct AgentUpdate {
    pub issue_id: IssueId,
    pub reset_usage: bool,
    pub new_turn_id: bool,
    pub timestamp: Option<nat>,
    pub usage: Option<Usage>,
    pub has_rate_limits: bool,
}

pub struct OrchestratorState {
    pub claimed: Set<IssueId>,
    pub running: Map<IssueId, RunningEntry>,
    pub retry_attempts: Map<IssueId, RetryEntry>,
    pub codex_totals: CodexTotals,
    pub has_rate_limits: bool,
}

pub enum Event {
    Claim(IssueId),
    MarkRunning(IssueId),
    UpdateAgent(AgentUpdate),
    QueueRetry { issue_id: IssueId, attempt: nat },
    Release(IssueId),
}

pub enum TransitionRejection {
    AlreadyClaimed,
    MissingClaim,
    AlreadyRunning,
    InvalidRetryAttempt,
    RetryAttemptRegression,
}

pub enum Command {
    Dispatch(IssueId),
    ScheduleRetry { issue_id: IssueId, attempt: nat },
    ReleaseClaim(IssueId),
    TransitionRejected { issue_id: IssueId, reason: TransitionRejection },
}

pub open spec fn zero_usage() -> Usage {
    Usage {
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
    }
}

pub open spec fn zero_running_entry() -> RunningEntry {
    RunningEntry {
        turn_count: 0,
        started_at: None,
        usage: zero_usage(),
    }
}

pub open spec fn saturating_sub(next: nat, previous: nat) -> nat {
    if next >= previous {
        (next - previous) as nat
    } else {
        0
    }
}

pub open spec fn usage_delta(previous: Usage, next: Usage) -> Usage {
    Usage {
        input_tokens: saturating_sub(next.input_tokens, previous.input_tokens),
        output_tokens: saturating_sub(next.output_tokens, previous.output_tokens),
        total_tokens: saturating_sub(next.total_tokens, previous.total_tokens),
    }
}

pub open spec fn add_usage_to_totals(totals: CodexTotals, delta: Usage) -> CodexTotals {
    CodexTotals {
        input_tokens: totals.input_tokens + delta.input_tokens,
        output_tokens: totals.output_tokens + delta.output_tokens,
        total_tokens: totals.total_tokens + delta.total_tokens,
    }
}

pub open spec fn apply_agent_update_to_running_entry(entry: RunningEntry, update: AgentUpdate) -> RunningEntry {
    let base_usage = if update.reset_usage {
        zero_usage()
    } else {
        entry.usage
    };
    let next_usage = match update.usage {
        Some(usage) => usage,
        None => base_usage,
    };
    RunningEntry {
        turn_count: if update.new_turn_id {
            entry.turn_count + 1
        } else {
            entry.turn_count
        },
        started_at: match update.timestamp {
            Some(timestamp) => match entry.started_at {
                Some(existing) => Some(existing),
                None => Some(timestamp),
            },
            None => entry.started_at,
        },
        usage: next_usage,
    }
}

pub open spec fn orchestrator_invariants(state: OrchestratorState) -> bool {
    &&& forall|id: IssueId| #![auto] state.running.dom().contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId| #![auto] state.retry_attempts.dom().contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId|
            !(state.running.dom().contains(id) && state.retry_attempts.dom().contains(id))
    &&& forall|id: IssueId|
            #![auto]
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
                    codex_totals: state.codex_totals,
                    has_rate_limits: state.has_rate_limits,
                }
            }
        },
        Event::MarkRunning(issue_id) => {
            if !state.claimed.contains(issue_id) || state.running.dom().contains(issue_id) {
                state
            } else {
                OrchestratorState {
                    claimed: state.claimed,
                    running: state.running.insert(issue_id, zero_running_entry()),
                    retry_attempts: state.retry_attempts.remove(issue_id),
                    codex_totals: state.codex_totals,
                    has_rate_limits: state.has_rate_limits,
                }
            }
        },
        Event::UpdateAgent(update) => {
            if !state.running.dom().contains(update.issue_id) {
                state
            } else {
                let issue_id = update.issue_id;
                let current_entry = state.running[issue_id];
                let base_usage = if update.reset_usage {
                    zero_usage()
                } else {
                    current_entry.usage
                };
                let usage_delta = match update.usage {
                    Some(usage) => usage_delta(base_usage, usage),
                    None => zero_usage(),
                };
                OrchestratorState {
                    claimed: state.claimed,
                    running: state
                        .running
                        .insert(issue_id, apply_agent_update_to_running_entry(current_entry, update)),
                    retry_attempts: state.retry_attempts,
                    codex_totals: add_usage_to_totals(state.codex_totals, usage_delta),
                    has_rate_limits: state.has_rate_limits || update.has_rate_limits,
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
                    codex_totals: state.codex_totals,
                    has_rate_limits: state.has_rate_limits,
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
                    codex_totals: state.codex_totals,
                    has_rate_limits: state.has_rate_limits,
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
        Event::UpdateAgent(update) => !state.running.dom().contains(update.issue_id),
        Event::QueueRetry { issue_id, attempt } => {
            !state.claimed.contains(issue_id)
                || attempt == 0
                || (state.retry_attempts.dom().contains(issue_id)
                    && state.retry_attempts[issue_id].attempt >= attempt)
        },
        Event::Release(issue_id) => !state.claimed.contains(issue_id),
    }
}

pub open spec fn event_issue_id(event: Event) -> IssueId {
    match event {
        Event::Claim(issue_id) => issue_id,
        Event::MarkRunning(issue_id) => issue_id,
        Event::UpdateAgent(update) => update.issue_id,
        Event::QueueRetry { issue_id, attempt: _ } => issue_id,
        Event::Release(issue_id) => issue_id,
    }
}

pub open spec fn rejection_reason(state: OrchestratorState, event: Event) -> Option<TransitionRejection> {
    match event {
        Event::Claim(issue_id) => {
            if state.claimed.contains(issue_id) {
                Some(TransitionRejection::AlreadyClaimed)
            } else {
                None
            }
        },
        Event::MarkRunning(issue_id) => {
            if !state.claimed.contains(issue_id) {
                Some(TransitionRejection::MissingClaim)
            } else if state.running.dom().contains(issue_id) {
                Some(TransitionRejection::AlreadyRunning)
            } else {
                None
            }
        },
        Event::UpdateAgent(update) => {
            if !state.running.dom().contains(update.issue_id) {
                Some(TransitionRejection::MissingClaim)
            } else {
                None
            }
        },
        Event::QueueRetry { issue_id, attempt } => {
            if !state.claimed.contains(issue_id) {
                Some(TransitionRejection::MissingClaim)
            } else if attempt == 0 {
                Some(TransitionRejection::InvalidRetryAttempt)
            } else if state.retry_attempts.dom().contains(issue_id)
                && state.retry_attempts[issue_id].attempt >= attempt
            {
                Some(TransitionRejection::RetryAttemptRegression)
            } else {
                None
            }
        },
        Event::Release(issue_id) => {
            if !state.claimed.contains(issue_id) {
                Some(TransitionRejection::MissingClaim)
            } else {
                None
            }
        },
    }
}

pub open spec fn emitted_commands(state: OrchestratorState, event: Event) -> Seq<Command> {
    match rejection_reason(state, event) {
        Some(reason) => seq![Command::TransitionRejected {
            issue_id: event_issue_id(event),
            reason,
        }],
        None => match event {
            Event::Claim(issue_id) => seq![],
            Event::MarkRunning(issue_id) => seq![Command::Dispatch(issue_id)],
            Event::UpdateAgent(update) => seq![],
            Event::QueueRetry { issue_id, attempt } => seq![Command::ScheduleRetry {
                issue_id,
                attempt,
            }],
            Event::Release(issue_id) => seq![Command::ReleaseClaim(issue_id)],
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

pub proof fn lemma_update_agent_preserves_invariants(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_event(state, Event::UpdateAgent(update))),
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
        Event::UpdateAgent(update) => {
            lemma_update_agent_preserves_invariants(state, update);
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

pub proof fn lemma_update_agent_without_running_is_noop(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        !state.running.dom().contains(update.issue_id),
    ensures
        apply_event(state, Event::UpdateAgent(update)) == state,
{
}

pub proof fn lemma_update_agent_preserves_topology_when_running(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
    ensures
        apply_event(state, Event::UpdateAgent(update)).claimed == state.claimed,
        apply_event(state, Event::UpdateAgent(update)).running.dom() == state.running.dom(),
        apply_event(state, Event::UpdateAgent(update)).retry_attempts == state.retry_attempts,
{
}

pub proof fn lemma_update_agent_totals_are_monotonic(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
    ensures
        apply_event(state, Event::UpdateAgent(update)).codex_totals.input_tokens
            >= state.codex_totals.input_tokens,
        apply_event(state, Event::UpdateAgent(update)).codex_totals.output_tokens
            >= state.codex_totals.output_tokens,
        apply_event(state, Event::UpdateAgent(update)).codex_totals.total_tokens
            >= state.codex_totals.total_tokens,
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
        Event::UpdateAgent(update) => {
            lemma_update_agent_without_running_is_noop(state, update);
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

pub proof fn lemma_emitted_command_count_is_bounded(state: OrchestratorState, event: Event)
    ensures
        emitted_commands(state, event).len() <= 1,
{
}

pub proof fn lemma_claim_emits_no_commands_on_valid_transition(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        !state.claimed.contains(issue_id),
    ensures
        emitted_commands(state, Event::Claim(issue_id)) == Seq::<Command>::empty(),
{
}

pub proof fn lemma_mark_running_emits_dispatch_on_valid_transition(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
        !state.running.dom().contains(issue_id),
    ensures
        emitted_commands(state, Event::MarkRunning(issue_id))
            == seq![Command::Dispatch(issue_id)],
{
}

pub proof fn lemma_update_agent_emits_no_commands_on_valid_transition(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
    ensures
        emitted_commands(state, Event::UpdateAgent(update)) == Seq::<Command>::empty(),
{
}

pub proof fn lemma_queue_retry_emits_schedule_retry_on_valid_transition(
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
        emitted_commands(state, Event::QueueRetry { issue_id, attempt })
            == seq![Command::ScheduleRetry { issue_id, attempt }],
{
}

pub proof fn lemma_release_emits_release_claim_on_valid_transition(
    state: OrchestratorState,
    issue_id: IssueId,
)
    requires
        orchestrator_invariants(state),
        state.claimed.contains(issue_id),
    ensures
        emitted_commands(state, Event::Release(issue_id))
            == seq![Command::ReleaseClaim(issue_id)],
{
}

pub proof fn lemma_rejection_emits_transition_rejected_command(
    state: OrchestratorState,
    event: Event,
)
    requires
        orchestrator_invariants(state),
        is_rejection_case(state, event),
    ensures
        emitted_commands(state, event).len() == 1,
        emitted_commands(state, event)[0]
            == (Command::TransitionRejected {
                issue_id: event_issue_id(event),
                reason: rejection_reason(state, event).unwrap(),
            }),
{
}

fn main() {}

} // verus!
