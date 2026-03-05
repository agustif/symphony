#![forbid(unsafe_code)]

mod cli_cases;
mod orchestrator_cases;
mod protocol_cases;
mod workflow_cases;

use symphony_domain::{Event, OrchestratorState, RetryEntry};
use symphony_testkit::{command_count, issue_id, run_trace, run_trace_from_state, validate_trace};

#[test]
fn lifecycle_claim_run_release_is_conformant() {
    let issue = issue_id("SYM-9001");
    let events = vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Release(issue),
    ];

    let trace = run_trace(events);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn mark_running_conformance_always_claims_issue() {
    let issue = issue_id("SYM-42");
    let trace = run_trace(vec![Event::MarkRunning(issue.clone())]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(!trace.final_state.running.contains_key(&issue));
    assert!(!trace.final_state.claimed.contains(&issue));
}

#[test]
fn release_conformance_clears_seeded_retry_tracking() {
    let issue = issue_id("SYM-77");
    let mut state = OrchestratorState::default();
    state
        .retry_attempts
        .insert(issue.clone(), RetryEntry { attempt: 3 });
    state.claimed.insert(issue.clone());
    state
        .running
        .insert(issue.clone(), symphony_domain::RunningEntry::default());

    let trace = run_trace_from_state(state, vec![Event::Release(issue)]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(trace.final_state.retry_attempts.is_empty());
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

#[test]
fn command_stream_conformance_matches_event_count() {
    let issue = issue_id("SYM-11");
    let trace = run_trace(vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Claim(issue.clone()),
        Event::Release(issue),
    ]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert_eq!(command_count(&trace), 3);
}
