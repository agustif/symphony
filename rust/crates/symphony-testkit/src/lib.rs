#![forbid(unsafe_code)]

mod issue_builders;
mod observability_builders;
mod path_builders;
mod protocol_builders;

use symphony_domain::{
    Command, Event, InvariantError, IssueId, OrchestratorState, reduce, validate_invariants,
};

#[derive(Clone, Debug)]
pub struct TraceStep {
    pub event: Event,
    pub commands: Vec<Command>,
    pub state: OrchestratorState,
}

#[derive(Clone, Debug, Default)]
pub struct TraceResult {
    pub steps: Vec<TraceStep>,
    pub final_state: OrchestratorState,
}

pub use issue_builders::issue_id;
pub use observability_builders::{issue_snapshot, runtime_snapshot, state_snapshot};
pub use path_builders::test_cwd;
pub use protocol_builders::{protocol_stderr_line, protocol_stdout_line};

pub fn issue_ids(prefix: &str, count: usize) -> Vec<IssueId> {
    (1..=count)
        .map(|index| issue_id(&format!("{prefix}-{index}")))
        .collect()
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

pub fn interleave_preserving_order(
    left: &[Event],
    right: &[Event],
    limit: usize,
) -> Vec<Vec<Event>> {
    if limit == 0 {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut prefix = Vec::with_capacity(left.len() + right.len());
    collect_interleavings(left, right, 0, 0, &mut prefix, &mut results, limit);
    results
}

fn collect_interleavings(
    left: &[Event],
    right: &[Event],
    left_index: usize,
    right_index: usize,
    prefix: &mut Vec<Event>,
    results: &mut Vec<Vec<Event>>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    if left_index == left.len() && right_index == right.len() {
        results.push(prefix.clone());
        return;
    }
    if left_index < left.len() {
        prefix.push(left[left_index].clone());
        collect_interleavings(
            left,
            right,
            left_index + 1,
            right_index,
            prefix,
            results,
            limit,
        );
        prefix.pop();
    }
    if right_index < right.len() {
        prefix.push(right[right_index].clone());
        collect_interleavings(
            left,
            right,
            left_index,
            right_index + 1,
            prefix,
            results,
            limit,
        );
        prefix.pop();
    }
}

pub fn deterministic_event_stream(issue_ids: &[IssueId], steps: usize, seed: u64) -> Vec<Event> {
    if issue_ids.is_empty() || steps == 0 {
        return Vec::new();
    }

    let mut generator_state = seed;
    let mut events = Vec::with_capacity(steps);
    for _ in 0..steps {
        generator_state = next_lcg(generator_state);
        let issue_index = (generator_state as usize) % issue_ids.len();
        let issue = issue_ids[issue_index].clone();

        generator_state = next_lcg(generator_state);
        let event = match generator_state % 3 {
            0 => Event::Claim(issue),
            1 => Event::MarkRunning(issue),
            _ => Event::Release(issue),
        };
        events.push(event);
    }
    events
}

fn next_lcg(value: u64) -> u64 {
    value
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

pub fn release_events(issue_ids: &[IssueId]) -> Vec<Event> {
    issue_ids.iter().cloned().map(Event::Release).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_observability::StateSnapshot;

    #[test]
    fn helper_builds_issue_id() {
        let id = issue_id("SYM-7");
        assert_eq!(id.0, "SYM-7");
    }

    #[test]
    fn helper_builds_state_snapshot() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-8", "SYM-8", "Running", 0)],
        );
        assert_eq!(snapshot.runtime.running, 1);
        assert_eq!(snapshot.issues.len(), 1);
    }

    #[test]
    fn helper_builds_protocol_stdout_line() {
        let line = protocol_stdout_line("turn.start");
        assert!(line.contains("\"method\":\"turn.start\""));
    }

    #[test]
    fn helper_builds_test_cwd() {
        let cwd = test_cwd("/tmp/symphony");
        assert_eq!(cwd, std::path::PathBuf::from("/tmp/symphony"));
    }

    #[test]
    fn state_snapshot_default_roundtrip() {
        let snapshot = StateSnapshot::default();
        assert_eq!(snapshot.runtime.running, 0);
    }

    #[test]
    fn interleaving_helper_respects_limit() {
        let issue = issue_id("SYM-1");
        let left = vec![Event::Claim(issue.clone()), Event::MarkRunning(issue.clone())];
        let right = vec![Event::Release(issue)];

        let schedules = interleave_preserving_order(&left, &right, 2);
        assert_eq!(schedules.len(), 2);
    }

    #[test]
    fn deterministic_stream_is_stable() {
        let issue_pool = issue_ids("SYM", 3);
        let stream_a = deterministic_event_stream(&issue_pool, 12, 11);
        let stream_b = deterministic_event_stream(&issue_pool, 12, 11);
        assert_eq!(stream_a, stream_b);
    }
}
