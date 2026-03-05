use symphony_domain::{
    reduce, validate_invariants, Command, Event, InvariantError, OrchestratorState,
};

// Use StateSnapshot from fixtures module
use crate::StateSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStep {
    pub event: Event,
    pub commands: Vec<Command>,
    pub state: OrchestratorState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraceResult {
    pub steps: Vec<TraceStep>,
    pub final_state: OrchestratorState,
}

pub fn run_trace(events: impl IntoIterator<Item = Event>) -> TraceResult {
    run_trace_from_state(OrchestratorState::default(), events)
}

pub fn run_trace_from_state(
    mut state: OrchestratorState,
    events: impl IntoIterator<Item = Event>,
) -> TraceResult {
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
        steps,
        final_state: state,
    }
}

pub fn validate_trace(result: &TraceResult) -> Result<(), InvariantError> {
    for step in &result.steps {
        validate_invariants(&step.state)?;
    }
    Ok(())
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
}
