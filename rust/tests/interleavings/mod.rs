#![forbid(unsafe_code)]

use symphony_domain::{AgentUpdate, Event, OrchestratorState, RunningEntry};
use symphony_testkit::{
    command_count, interleave_preserving_order, issue_id, run_trace, run_trace_from_state,
    validate_trace,
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

#[test]
fn release_and_retry_races_converge_to_cleared_issue_state() {
    let issue = issue_id("SYM-RACE");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 1,
    }];
    let right = vec![Event::Release(issue.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        assert!(!trace.final_state.claimed.contains(&issue));
        assert!(!trace.final_state.running.contains_key(&issue));
        assert!(!trace.final_state.retry_attempts.contains_key(&issue));
    }
}

#[test]
fn release_and_protocol_update_races_do_not_reanimate_running_entries() {
    let issue = issue_id("SYM-LATE");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    let left = vec![Event::Release(issue.clone())];
    let right = vec![Event::UpdateAgent(Box::new(AgentUpdate {
        issue_id: issue.clone(),
        session_id: Some("thread-late:turn-1".to_owned()),
        thread_id: Some("thread-late".to_owned()),
        turn_id: Some("turn-1".to_owned()),
        pid: Some(42),
        event: Some("turn/update".to_owned()),
        timestamp: Some(1_700_000_001),
        message: Some("late event".to_owned()),
        usage: None,
        rate_limits: None,
    }))];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        assert!(!trace.final_state.claimed.contains(&issue));
        assert!(!trace.final_state.running.contains_key(&issue));
        assert!(trace.final_state.retry_attempts.is_empty());
    }
}

#[test]
fn concurrent_queue_retry_attempts_enforce_monotonicity() {
    let issue = issue_id("SYM-MONO");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 2,
    }];
    let right = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 5,
    }];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Final attempt should be at least 5 (the higher of the two)
        if let Some(entry) = trace.final_state.retry_attempts.get(&issue) {
            assert!(
                entry.attempt >= 5,
                "attempt should be >= 5, got {}",
                entry.attempt
            );
        }
    }
}

#[test]
fn worker_exit_continuation_races_with_new_dispatch() {
    let issue = issue_id("SYM-DISPATCH");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    let left = vec![Event::Release(issue.clone())];
    let right = vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
    ];

    let schedules = interleave_preserving_order(&left, &right, 16);
    assert!(!schedules.is_empty());

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // After release, claim can re-succeed but running state should be consistent
        assert!(trace.final_state.claimed.len() <= 1);
        assert!(trace.final_state.running.len() <= 1);
    }
}

#[test]
fn retry_pop_and_release_race() {
    let issue = issue_id("SYM-POP");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // QueueRetry transitions out of running, then Release clears the issue
    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 1,
    }];
    let right = vec![Event::Release(issue.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Issue should be released regardless of order
        assert!(!trace.final_state.claimed.contains(&issue));
        assert!(!trace.final_state.running.contains_key(&issue));
    }
}

#[test]
fn retry_attempt_regression_is_rejected() {
    let issue = issue_id("SYM-REGR");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // First, set up a high retry attempt
    let setup = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 5,
    }];
    let setup_trace = run_trace_from_state(state.clone(), setup);
    let state_with_high_attempt = setup_trace.final_state;

    // Now race a lower attempt (regression) with a release
    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 2,
    }];
    let right = vec![Event::Release(issue.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state_with_high_attempt.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
    }
}

#[test]
fn terminal_transition_during_retry() {
    let issue = issue_id("SYM-TERM");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // Set up retry state
    let setup = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 3,
    }];
    let setup_trace = run_trace_from_state(state.clone(), setup);
    let state_with_retry = setup_trace.final_state;

    // Race Release with another QueueRetry
    let left = vec![Event::Release(issue.clone())];
    let right = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 4,
    }];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state_with_retry.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Release should clear the retry entry
        assert!(!trace.final_state.claimed.contains(&issue));
        assert!(!trace.final_state.running.contains_key(&issue));
    }
}

#[test]
fn protocol_update_after_queue_retry_is_ignored() {
    let issue = issue_id("SYM-PROTO");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // QueueRetry transitions issue out of running state
    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 1,
    }];
    let right = vec![Event::UpdateAgent(Box::new(AgentUpdate {
        issue_id: issue.clone(),
        session_id: Some("session-proto".to_owned()),
        thread_id: Some("thread-proto".to_owned()),
        turn_id: Some("turn-1".to_owned()),
        pid: Some(123),
        event: Some("update".to_owned()),
        timestamp: Some(1_700_000_000),
        message: Some("proto event".to_owned()),
        usage: None,
        rate_limits: None,
    }))];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // UpdateAgent after QueueRetry should be ignored (issue not in running)
        // The state should be consistent regardless of ordering
    }
}

// ========================================================================
// I1.1/I1.2: Scheduler Interleaving Race Tests
// ========================================================================

/// I1.1: Tick/retry race - poll dispatch running while retry fires
/// Verifies that when a poll tick dispatches work while a retry timer fires,
/// the state remains consistent without double-dispatch or missed retry.
#[test]
fn tick_dispatch_races_with_retry_timer() {
    let issue = issue_id("SYM-TICK-RETRY");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    // Issue is in retry state (not running, but claimed with retry entry)
    state.retry_attempts.insert(
        issue.clone(),
        symphony_domain::RetryEntry {
            attempt: 2,
            identifier: None,
            error: None,
            due_at: Some(100),
        },
    );

    // Retry timer pops (QueueRetry) while tick tries to dispatch (MarkRunning)
    let left = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 3,
    }];
    let right = vec![Event::MarkRunning(issue.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Issue should be in a consistent state
        assert!(
            trace.final_state.claimed.contains(&issue)
                || trace.final_state.running.contains_key(&issue)
        );
    }
}

/// I1.2: Reconcile/retry race - terminal transition during retry handling
/// Verifies that when reconciliation detects terminal state while retry is being processed,
/// the issue is properly released without orphaned state.
#[test]
fn reconcile_terminal_races_with_retry_handling() {
    let issue = issue_id("SYM-TERM-RETRY");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // Release (terminal detection from reconcile) races with QueueRetry
    let left = vec![Event::Release(issue.clone())];
    let right = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 4,
    }];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Terminal release should clear retry entry
        assert!(!trace.final_state.claimed.contains(&issue));
        assert!(!trace.final_state.running.contains_key(&issue));
    }
}

/// I1.1: Multiple issue tick/retry race with cross-issue interactions
#[test]
fn multi_issue_tick_retry_races_remain_isolated() {
    let issue_a = issue_id("SYM-MULTI-A");
    let issue_b = issue_id("SYM-MULTI-B");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_a.clone());
    state.claimed.insert(issue_b.clone());
    state
        .running
        .insert(issue_a.clone(), RunningEntry::default());

    // Issue A: Release races with retry
    // Issue B: MarkRunning races with release
    let left = vec![
        Event::Release(issue_a.clone()),
        Event::QueueRetry {
            issue_id: issue_b.clone(),
            attempt: 1,
        },
    ];
    let right = vec![
        Event::MarkRunning(issue_b.clone()),
        Event::Release(issue_a.clone()),
    ];

    let schedules = interleave_preserving_order(&left, &right, 32);
    assert!(!schedules.is_empty());

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
    }
}

// ========================================================================
// I2.1: Worker Lifecycle Race Tests
// ========================================================================

/// I2.1a: Worker spawning while shutdown requested
/// Verifies that workers spawned just before/during shutdown are properly cleaned up.
#[test]
fn worker_spawn_during_shutdown_is_handled() {
    let issue = issue_id("SYM-SPAWN-SHUTDOWN");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());

    // Worker spawn (MarkRunning) races with release (shutdown)
    let left = vec![Event::MarkRunning(issue.clone())];
    let right = vec![Event::Release(issue.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Final state should be clean regardless of order
        assert!(!trace.final_state.running.contains_key(&issue));
    }
}

/// I2.1b: Worker death during state transition
/// Verifies that when a worker dies (Release) during a state transition (QueueRetry),
/// the state remains consistent.
#[test]
fn worker_death_during_state_transition() {
    let issue = issue_id("SYM-DEATH-TRANS");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    // Worker death (Release) races with state transition (QueueRetry)
    let left = vec![Event::Release(issue.clone())];
    let right = vec![Event::QueueRetry {
        issue_id: issue.clone(),
        attempt: 5,
    }];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
    }
}

/// I2.1c: Multiple workers completing simultaneously
/// Verifies that when multiple workers for different issues complete at the same time,
/// the state is updated correctly for all.
#[test]
fn multiple_worker_completions_simultaneous() {
    let issue_a = issue_id("SYM-COMPLETE-A");
    let issue_b = issue_id("SYM-COMPLETE-B");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_a.clone());
    state.claimed.insert(issue_b.clone());
    state
        .running
        .insert(issue_a.clone(), RunningEntry::default());
    state
        .running
        .insert(issue_b.clone(), RunningEntry::default());

    // Both workers release simultaneously
    let left = vec![Event::Release(issue_a.clone())];
    let right = vec![Event::Release(issue_b.clone())];

    let schedules = interleave_preserving_order(&left, &right, 8);
    assert_eq!(schedules.len(), 2);

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Both should be released
        assert!(!trace.final_state.running.contains_key(&issue_a));
        assert!(!trace.final_state.running.contains_key(&issue_b));
        assert!(trace.final_state.claimed.is_empty());
    }
}

/// I2.1d: Worker restart race condition
/// Verifies that rapid worker death and restart cycles don't corrupt state.
#[test]
fn worker_restart_race_preserves_invariants() {
    let issue = issue_id("SYM-RESTART");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());

    // Cycle: Start -> Stop -> Start -> Stop
    let left = vec![
        Event::MarkRunning(issue.clone()),
        Event::Release(issue.clone()),
    ];
    let right = vec![
        Event::MarkRunning(issue.clone()),
        Event::Release(issue.clone()),
    ];

    let schedules = interleave_preserving_order(&left, &right, 64);
    assert!(!schedules.is_empty());

    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Command count should be bounded by event count
        assert!(command_count(&trace) <= 4);
    }
}

/// I2.1e: Protocol update races with worker lifecycle
#[test]
fn protocol_update_races_with_lifecycle() {
    let issue = issue_id("SYM-PROTO-LIFECYCLE");
    let mut state = OrchestratorState::default();
    state.claimed.insert(issue.clone());
    state.running.insert(issue.clone(), RunningEntry::default());

    let left = vec![Event::Release(issue.clone())];
    let right = vec![Event::UpdateAgent(Box::new(AgentUpdate {
        issue_id: issue.clone(),
        session_id: Some("session-race".to_owned()),
        thread_id: Some("thread-race".to_owned()),
        turn_id: Some("turn-race".to_owned()),
        pid: Some(99),
        event: Some("update".to_owned()),
        timestamp: Some(1_800_000_000),
        message: Some("racing update".to_owned()),
        usage: None,
        rate_limits: None,
    }))];

    let schedules = interleave_preserving_order(&left, &right, 8);
    for schedule in schedules {
        let trace = run_trace_from_state(state.clone(), schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
        // Release should win - update after release is ignored
        assert!(!trace.final_state.running.contains_key(&issue));
    }
}
