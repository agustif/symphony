#![forbid(unsafe_code)]

use symphony_testkit::{
    command_count, deterministic_event_stream, issue_ids, max_active_counts, release_events,
    run_trace, validate_trace,
};

#[test]
fn deterministic_event_soak_keeps_state_bounded() {
    let issue_pool = issue_ids("SOAK", 8);
    let mut events = deterministic_event_stream(&issue_pool, 1_500, 0x5EED_BA5E);
    events.extend(release_events(&issue_pool));

    let expected_commands = events.len();
    let trace = run_trace(events);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert_eq!(command_count(&trace), expected_commands);

    let (max_claimed, max_running) = max_active_counts(&trace);
    assert!(max_claimed <= issue_pool.len());
    assert!(max_running <= issue_pool.len());

    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

#[test]
fn long_run_soak_with_multiple_seeds_remains_valid() {
    let issue_pool = issue_ids("SOAK", 6);
    let first = run_trace(deterministic_event_stream(&issue_pool, 900, 9));
    let second = run_trace(deterministic_event_stream(&issue_pool, 900, 99));

    assert_eq!(validate_trace(&first), Ok(()));
    assert_eq!(validate_trace(&second), Ok(()));

    let (first_claimed, first_running) = max_active_counts(&first);
    let (second_claimed, second_running) = max_active_counts(&second);
    assert!(first_claimed <= issue_pool.len());
    assert!(first_running <= issue_pool.len());
    assert!(second_claimed <= issue_pool.len());
    assert!(second_running <= issue_pool.len());

    assert_ne!(
        first
            .steps
            .iter()
            .map(|step| &step.event)
            .collect::<Vec<_>>(),
        second
            .steps
            .iter()
            .map(|step| &step.event)
            .collect::<Vec<_>>()
    );
}
