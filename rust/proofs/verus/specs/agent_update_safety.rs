// Verus proof suite for agent-update accounting and topology preservation.
// Model aligns with rust/crates/symphony-domain/src/agent_update.rs.

#![allow(unused_imports)]

use vstd::{map::*, prelude::*, seq::*, set::*};

verus! {

pub type IssueId = int;

pub struct RunningEntry {
    pub session_epoch: nat,
    pub last_usage_total: nat,
    pub turn_count: nat,
    pub started_at_seen: bool,
}

pub struct OrchestratorState {
    pub claimed: Set<IssueId>,
    pub running: Map<IssueId, RunningEntry>,
    pub retrying: Set<IssueId>,
    pub total_usage: nat,
    pub has_rate_limits: bool,
}

pub struct AgentUpdate {
    pub issue_id: IssueId,
    pub session_epoch: nat,
    pub usage_total: nat,
    pub starts_new_turn: bool,
    pub has_timestamp: bool,
    pub has_rate_limits: bool,
}

pub enum TransitionRejection {
    MissingClaim,
}

pub enum Command {
    TransitionRejected { issue_id: IssueId, reason: TransitionRejection },
}

pub open spec fn orchestrator_invariants(state: OrchestratorState) -> bool {
    &&& forall|id: IssueId| #![auto] state.running.dom().contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId| #![auto] state.retrying.contains(id) ==> state.claimed.contains(id)
    &&& forall|id: IssueId| #![auto] !(state.running.dom().contains(id) && state.retrying.contains(id))
}

pub open spec fn effective_usage_baseline(state: OrchestratorState, update: AgentUpdate) -> nat {
    if state.running.dom().contains(update.issue_id)
        && state.running[update.issue_id].session_epoch == update.session_epoch
    {
        state.running[update.issue_id].last_usage_total
    } else {
        0
    }
}

pub open spec fn usage_delta(state: OrchestratorState, update: AgentUpdate) -> nat {
    let baseline = effective_usage_baseline(state, update);
    if baseline <= update.usage_total {
        (update.usage_total - baseline) as nat
    } else {
        0nat
    }
}

pub open spec fn apply_agent_update(
    state: OrchestratorState,
    update: AgentUpdate,
) -> OrchestratorState {
    if !state.running.dom().contains(update.issue_id) {
        state
    } else {
        OrchestratorState {
            claimed: state.claimed,
            running: state.running.insert(
                update.issue_id,
                RunningEntry {
                    session_epoch: update.session_epoch,
                    last_usage_total: update.usage_total,
                    turn_count: state.running[update.issue_id].turn_count
                        + if update.starts_new_turn { 1nat } else { 0nat },
                    started_at_seen: state.running[update.issue_id].started_at_seen || update.has_timestamp,
                },
            ),
            retrying: state.retrying,
            total_usage: state.total_usage + usage_delta(state, update),
            has_rate_limits: state.has_rate_limits || update.has_rate_limits,
        }
    }
}

pub open spec fn emitted_commands(state: OrchestratorState, update: AgentUpdate) -> Seq<Command> {
    if state.running.dom().contains(update.issue_id) {
        Seq::<Command>::empty()
    } else {
        seq![Command::TransitionRejected {
            issue_id: update.issue_id,
            reason: TransitionRejection::MissingClaim,
        }]
    }
}

pub proof fn lemma_update_without_running_issue_is_noop_and_rejected(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        !state.running.dom().contains(update.issue_id),
    ensures
        apply_agent_update(state, update) == state,
        emitted_commands(state, update)
            == seq![Command::TransitionRejected {
                issue_id: update.issue_id,
                reason: TransitionRejection::MissingClaim,
            }],
{
}

pub proof fn lemma_update_preserves_topology_invariants(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
    ensures
        orchestrator_invariants(apply_agent_update(state, update)),
{
}

pub proof fn lemma_update_preserves_claim_and_retry_membership(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
    ensures
        apply_agent_update(state, update).claimed == state.claimed,
        apply_agent_update(state, update).retrying == state.retrying,
        apply_agent_update(state, update).running.dom() == state.running.dom(),
{
}

pub proof fn lemma_update_totals_are_monotonic(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
    ensures
        apply_agent_update(state, update).total_usage >= state.total_usage,
{
}

pub proof fn lemma_same_session_uses_previous_usage_baseline(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
        state.running[update.issue_id].session_epoch == update.session_epoch,
        state.running[update.issue_id].last_usage_total <= update.usage_total,
    ensures
        usage_delta(state, update)
            == update.usage_total - state.running[update.issue_id].last_usage_total,
        apply_agent_update(state, update).total_usage
            == state.total_usage
                + update.usage_total
                - state.running[update.issue_id].last_usage_total,
{
}

pub proof fn lemma_session_change_rebases_usage_delta(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
        state.running[update.issue_id].session_epoch != update.session_epoch,
    ensures
        effective_usage_baseline(state, update) == 0,
        usage_delta(state, update) == update.usage_total,
        apply_agent_update(state, update).running[update.issue_id].last_usage_total
            == update.usage_total,
{
}

pub proof fn lemma_timestamp_updates_only_raise_started_flag(
    state: OrchestratorState,
    update: AgentUpdate,
)
    requires
        orchestrator_invariants(state),
        state.running.dom().contains(update.issue_id),
    ensures
        apply_agent_update(state, update).running[update.issue_id].started_at_seen
            == (state.running[update.issue_id].started_at_seen || update.has_timestamp),
{
}

fn main() {}

} // verus!
