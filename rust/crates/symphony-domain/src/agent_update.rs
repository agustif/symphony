use crate::{
    AgentUpdate, CodexTotals, OrchestratorState, RunningEntry, TransitionRejection, Usage,
};

pub(crate) fn apply_agent_update(
    state: &mut OrchestratorState,
    update: AgentUpdate,
) -> Result<(), TransitionRejection> {
    // First check if the issue is in the claimed set (invariant: running implies claimed)
    if !state.claimed.contains(&update.issue_id) {
        return Err(TransitionRejection::MissingClaim);
    }

    // Then check if the issue is actually running
    if !state.running.contains_key(&update.issue_id) {
        return Err(TransitionRejection::RunningWithoutClaim);
    }

    let issue_id = update.issue_id.clone();

    let Some(entry) = state.running.get_mut(&issue_id) else {
        return Err(TransitionRejection::RunningWithoutClaim);
    };

    apply_update_to_running_entry(entry, &update, &mut state.codex_totals);

    if let Some(next_rate_limits) = update.rate_limits {
        state.codex_rate_limits = Some(next_rate_limits);
    }

    Ok(())
}

fn apply_update_to_running_entry(
    entry: &mut RunningEntry,
    update: &AgentUpdate,
    totals: &mut CodexTotals,
) {
    if session_identity_changed(entry, update) {
        entry.usage = Usage::default();
    }

    if let Some(session_id) = &update.session_id {
        entry.session_id = Some(session_id.clone());
    }

    if let Some(thread_id) = &update.thread_id {
        entry.thread_id = Some(thread_id.clone());
    }

    if let Some(turn_id) = &update.turn_id {
        if entry.turn_id.as_ref() != Some(turn_id) {
            entry.turn_count = entry.turn_count.saturating_add(1);
        }
        entry.turn_id = Some(turn_id.clone());
    }

    if let Some(pid) = update.pid {
        entry.codex_app_server_pid = Some(pid);
    }

    if let Some(event) = &update.event {
        entry.last_codex_event = Some(event.clone());
    }

    if let Some(timestamp) = update.timestamp {
        if entry.started_at.is_none() {
            entry.started_at = Some(timestamp);
        }
        entry.last_codex_timestamp = Some(timestamp);
    }

    if let Some(message) = &update.message {
        entry.last_codex_message = Some(message.clone());
    }

    if let Some(next_usage) = &update.usage {
        let delta = usage_delta(&entry.usage, next_usage);
        entry.usage = next_usage.clone();
        accumulate_usage_totals(totals, &delta);
    }
}

fn session_identity_changed(entry: &RunningEntry, update: &AgentUpdate) -> bool {
    let session_changed = update
        .session_id
        .as_ref()
        .is_some_and(|next| entry.session_id.as_ref() != Some(next));
    let thread_changed = update
        .thread_id
        .as_ref()
        .is_some_and(|next| entry.thread_id.as_ref() != Some(next));

    session_changed || thread_changed
}

fn usage_delta(previous: &Usage, next: &Usage) -> Usage {
    Usage {
        input_tokens: next.input_tokens.saturating_sub(previous.input_tokens),
        output_tokens: next.output_tokens.saturating_sub(previous.output_tokens),
        total_tokens: next.total_tokens.saturating_sub(previous.total_tokens),
    }
}

fn accumulate_usage_totals(totals: &mut CodexTotals, delta: &Usage) {
    totals.input_tokens += delta.input_tokens;
    totals.output_tokens += delta.output_tokens;
    totals.total_tokens += delta.total_tokens;
}
