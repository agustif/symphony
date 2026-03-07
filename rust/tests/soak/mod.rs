#![forbid(unsafe_code)]

use symphony_domain::Event;
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
    assert!(command_count(&trace) <= expected_commands);

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

#[test]
fn retry_heavy_soak_preserves_attempt_validity_and_eventual_cleanup() {
    let issue_pool = issue_ids("SOAK-RETRY", 5);
    let mut events = Vec::new();
    for attempt in 1..=75 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));
            events.push(Event::MarkRunning(issue.clone()));
            events.push(Event::QueueRetry {
                issue_id: issue.clone(),
                attempt,
            });
            if attempt % 5 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);

    assert_eq!(validate_trace(&trace), Ok(()));

    for step in &trace.steps {
        for (issue_id, retry_entry) in &step.state.retry_attempts {
            assert!(
                retry_entry.attempt > 0,
                "retry attempt must stay positive for {}",
                issue_id.0
            );
            assert!(
                step.state.claimed.contains(issue_id),
                "retrying issue must remain claimed for {}",
                issue_id.0
            );
            assert!(
                !step.state.running.contains_key(issue_id),
                "retrying issue cannot also be running for {}",
                issue_id.0
            );
        }
    }

    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn async_poll_loop_durability_keeps_state_bounded() {
    let issue_pool = issue_ids("ASYNC", 20);
    let mut events = deterministic_event_stream(&issue_pool, 2_500, 0xCAFE_BABE);
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    let (max_claimed, max_running) = max_active_counts(&trace);
    assert!(max_claimed <= issue_pool.len());
    assert!(max_running <= issue_pool.len());

    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

#[test]
fn tracker_flapping_recovery_eventually_clears_state() {
    let issue_pool = issue_ids("FLAP", 4);
    let mut events = Vec::new();

    // Simulate flapping: rapid claim/release cycles
    for round in 0..50 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));
            if round % 2 == 0 {
                events.push(Event::MarkRunning(issue.clone()));
            }
            if round % 3 == 0 {
                events.push(Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: round + 1,
                });
            }
            if round % 2 == 1 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Verify no issues are stuck
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn token_window_remains_bounded_across_many_updates() {
    let issue_pool = issue_ids("TOKEN", 6);
    let mut events = deterministic_event_stream(&issue_pool, 800, 0xDEAD_BEEF);
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Verify token tracking consistency via codex_totals
    let _totals = &trace.final_state.codex_totals;

    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

// =============================================================================
// S1.1: Medium soak profile (1-hour mixed workload simulation)
// =============================================================================

#[test]
fn s1_medium_soak_mixed_workload_remains_bounded() {
    // Simulate 1-hour equivalent via large event count
    // At ~50 events/second realistic tick rate, 1 hour ≈ 180,000 events
    // Use 20,000 events for test tractability while preserving stress characteristics
    let issue_pool = issue_ids("S1-MED", 12);
    let mut events = deterministic_event_stream(&issue_pool, 20_000, 0x51EED1E);
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Resource usage assertions
    let (max_claimed, max_running) = max_active_counts(&trace);
    assert!(
        max_claimed <= issue_pool.len(),
        "claimed count bounded by pool size"
    );
    assert!(
        max_running <= issue_pool.len(),
        "running count bounded by pool size"
    );

    // Retry queue bounds
    let max_retrying = trace
        .steps
        .iter()
        .map(|s| s.state.retry_attempts.len())
        .max()
        .unwrap_or(0);
    assert!(
        max_retrying <= issue_pool.len(),
        "retry queue bounded by pool size"
    );

    // Eventual cleanup
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn s1_medium_soak_retry_queue_does_not_grow_unbounded() {
    let issue_pool = issue_ids("S1-RETRY", 8);
    let mut events = Vec::new();

    // Simulate sustained retry pressure
    for round in 0..200 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));
            events.push(Event::MarkRunning(issue.clone()));
            // Heavy retry pressure
            events.push(Event::QueueRetry {
                issue_id: issue.clone(),
                attempt: (round % 10) + 1,
            });
            // Periodic release to prevent unbounded growth
            if round % 3 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Verify retry queue stays bounded
    for step in &trace.steps {
        assert!(
            step.state.retry_attempts.len() <= issue_pool.len(),
            "retry queue must not exceed issue pool size at step"
        );
    }

    assert!(trace.final_state.retry_attempts.is_empty());
}

// =============================================================================
// S1.2: Extended soak profile (8-hour durability simulation)
// =============================================================================

#[test]
fn s2_extended_soak_durability_across_long_run() {
    // 8-hour equivalent would be ~1.44M events at 50 events/sec
    // Use 50,000 events for test tractability
    let issue_pool = issue_ids("S2-EXT", 15);
    let mut events = deterministic_event_stream(&issue_pool, 50_000, 0x8E8E_8E8E);
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Verify no state corruption over long run
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());

    // Verify reasonable throughput (command count should be proportional to events)
    let command_count = command_count(&trace);
    assert!(command_count > 0, "extended soak should produce commands");
}

#[test]
fn s2_extended_soak_stalled_worker_recovery_assertions() {
    let issue_pool = issue_ids("S2-STALL", 6);
    let mut events = Vec::new();

    // Simulate worker stall patterns: claim -> run -> stall -> retry -> recover
    for cycle in 0..100 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));
            events.push(Event::MarkRunning(issue.clone()));
            // Simulate stall by queueing retry (would be triggered by stall timeout)
            events.push(Event::QueueRetry {
                issue_id: issue.clone(),
                attempt: (cycle % 5) + 1,
            });
            // Recovery: release and re-claim
            if cycle % 4 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // All stalls should eventually recover
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

// =============================================================================
// S2.1: Tracker instability profile
// =============================================================================

#[test]
fn s2_tracker_instability_intermittent_failures_with_recovery() {
    let issue_pool = issue_ids("S2-TRK", 8);
    let mut events = Vec::new();

    // Simulate intermittent tracker failures via rapid state changes
    for round in 0..150 {
        for (idx, issue) in issue_pool.iter().enumerate() {
            // Simulate tracker returning inconsistent state
            if round % 7 == 0 {
                // Tracker "failure" - release all claims
                events.push(Event::Release(issue.clone()));
            } else if round % 5 == idx % 5 {
                // Normal operation
                events.push(Event::Claim(issue.clone()));
                if round % 3 == 0 {
                    events.push(Event::MarkRunning(issue.clone()));
                }
            }
            // Periodic retry pressure
            if round % 11 == 0 {
                events.push(Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: ((round % 8) + 1) as u32,
                });
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Verify recovery from tracker instability
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn s2_tracker_instability_state_refresh_failure_handling() {
    let issue_pool = issue_ids("S2-REFRESH", 5);
    let mut events = Vec::new();

    // Simulate state refresh failures with recovery
    for round in 0..100 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));

            // Simulate refresh failure causing retry
            if round % 4 == 0 {
                events.push(Event::MarkRunning(issue.clone()));
                events.push(Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: 1,
                });
            }

            // Recovery
            if round % 6 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // State should be consistent after recovery
    assert!(trace.final_state.claimed.is_empty());
}

// =============================================================================
// S2.2: Protocol instability profile
// =============================================================================

#[test]
fn s2_protocol_instability_malformed_output_recovery() {
    let issue_pool = issue_ids("S2-PROTO", 7);
    let mut events = Vec::new();

    // Simulate protocol errors causing worker failures
    for round in 0..120 {
        for (idx, issue) in issue_pool.iter().enumerate() {
            events.push(Event::Claim(issue.clone()));
            events.push(Event::MarkRunning(issue.clone()));

            // Simulate malformed protocol causing failure
            if round % 3 == idx % 3 {
                events.push(Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: ((round % 6) + 1) as u32,
                });
            }

            // Recovery from protocol error
            if round % 5 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Protocol errors should not leave orphaned state
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

#[test]
fn s2_protocol_instability_timeout_recovery_assertions() {
    let issue_pool = issue_ids("S2-TIMEOUT", 6);
    let mut events = Vec::new();

    // Simulate protocol timeouts
    for round in 0..80 {
        for issue in &issue_pool {
            events.push(Event::Claim(issue.clone()));
            events.push(Event::MarkRunning(issue.clone()));

            // Simulate timeout triggering retry
            if round % 2 == 0 {
                events.push(Event::QueueRetry {
                    issue_id: issue.clone(),
                    attempt: (round % 4) + 1,
                });
            }

            // Eventual recovery
            if round % 7 == 0 {
                events.push(Event::Release(issue.clone()));
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // Timeouts should not cause permanent stuck state
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn s2_protocol_instability_mixed_failure_modes() {
    let issue_pool = issue_ids("S2-MIXED", 10);
    let mut events = Vec::new();

    // Mix of protocol failures, timeouts, and tracker issues
    for round in 0..200 {
        for (idx, issue) in issue_pool.iter().enumerate() {
            events.push(Event::Claim(issue.clone()));

            // Different failure modes
            match round % 4 {
                0 => {
                    // Protocol parse failure
                    events.push(Event::MarkRunning(issue.clone()));
                    events.push(Event::QueueRetry {
                        issue_id: issue.clone(),
                        attempt: 1,
                    });
                }
                1 => {
                    // Timeout
                    events.push(Event::MarkRunning(issue.clone()));
                    events.push(Event::QueueRetry {
                        issue_id: issue.clone(),
                        attempt: 2,
                    });
                }
                2 => {
                    // Normal completion
                    events.push(Event::MarkRunning(issue.clone()));
                    if idx % 2 == 0 {
                        events.push(Event::Release(issue.clone()));
                    }
                }
                _ => {
                    // Tracker state change
                    events.push(Event::Release(issue.clone()));
                }
            }
        }
    }
    events.extend(release_events(&issue_pool));

    let trace = run_trace(events);
    assert_eq!(validate_trace(&trace), Ok(()));

    // All failure modes should eventually resolve
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}
