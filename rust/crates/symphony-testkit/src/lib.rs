#![forbid(unsafe_code)]

mod clock;
mod config_builders;
mod fake_generators;
mod fakes;
mod fixtures;
mod observability_builders;
mod path_builders;
mod protocol_builders;
mod schedules;
mod timer_queue;
mod trace;

pub use clock::{Clock, ClockHandle, DeterministicClock, RealClock, ThreadSafeClock};
pub use config_builders::{
    RuntimeConfigBuilder, WorkflowContentBuilder, minimal_workflow, runtime_config,
    workflow_content,
};
pub use fake_generators::{
    fake_blocker_ref, fake_issue_id, fake_issue_identifier, fake_tracker_issue,
    fake_tracker_issue_with_state, fake_tracker_issues,
};
pub use fakes::{
    FakeAppServerStream, FakeTracker, FakeTrackerConfig, FakeTrackerResponse, FakeWorkspaceHooks,
    tracker_issue,
};
pub use fixtures::{
    FixtureBuilder, FixtureError, SnapshotComparison, SnapshotDifference, StateSnapshot,
    compare_snapshots, issue_id, issue_ids, load_fixture, load_fixture_with_vars,
    load_json_fixture, load_json_fixture_with_vars, orchestrator_state_from_labels,
    resolve_fixture_path,
};
pub use observability_builders::{issue_snapshot, runtime_snapshot, state_snapshot};
pub use path_builders::test_cwd;
pub use protocol_builders::{protocol_stderr_line, protocol_stdout_line};
pub use schedules::{deterministic_event_stream, interleave_preserving_order, release_events};
pub use timer_queue::{DeterministicTimer, DeterministicTimerQueue};
pub use trace::{
    TraceResult, TraceStep, command_count, max_active_counts, run_trace, run_trace_from_state,
    snapshot_state, snapshot_trace, validate_trace,
};

#[cfg(test)]
mod tests {
    use super::*;

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
    fn interleaving_helper_respects_limit() {
        let issue = issue_id("SYM-1");
        let left = vec![
            symphony_domain::Event::Claim(issue.clone()),
            symphony_domain::Event::MarkRunning(issue.clone()),
        ];
        let right = vec![symphony_domain::Event::Release(issue)];

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
