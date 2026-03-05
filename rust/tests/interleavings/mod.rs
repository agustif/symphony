#![forbid(unsafe_code)]

use symphony_domain::Event;
use symphony_testkit::{
    command_count, interleave_preserving_order, issue_id, run_trace, validate_trace,
};

#[test]
fn duplicate_worker_lifecycle_interleavings_preserve_invariants() {
    let issue = issue_id("SYM-1337");
    let worker_a = vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Release(issue.clone()),
    ];
    let worker_b = vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Release(issue),
    ];

    let schedules = interleave_preserving_order(&worker_a, &worker_b, 64);
    assert_eq!(schedules.len(), 20);

    for schedule in schedules {
        let max_possible_commands = schedule.len();
        let trace = run_trace(schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        assert!(command_count(&trace) <= max_possible_commands);
        assert!(trace.final_state.claimed.is_empty());
        assert!(trace.final_state.running.is_empty());
    }
}

#[test]
fn cross_issue_interleavings_are_bounded_and_isolated() {
    let issue_a = issue_id("SYM-A");
    let issue_b = issue_id("SYM-B");

    let left = vec![
        Event::Claim(issue_a.clone()),
        Event::MarkRunning(issue_a.clone()),
        Event::Release(issue_a),
    ];
    let right = vec![
        Event::Claim(issue_b.clone()),
        Event::MarkRunning(issue_b.clone()),
        Event::Release(issue_b),
    ];

    let schedules = interleave_preserving_order(&left, &right, 64);
    assert_eq!(schedules.len(), 20);

    for schedule in schedules {
        let trace = run_trace(schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        for step in &trace.steps {
            assert!(step.state.claimed.len() <= 2);
            assert!(step.state.running.len() <= 2);
        }
        assert!(trace.final_state.claimed.is_empty());
        assert!(trace.final_state.running.is_empty());
    }
}
