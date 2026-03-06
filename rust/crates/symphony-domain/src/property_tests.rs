#[cfg(test)]
mod tests {
    use crate::*;
    use std::collections::HashSet;

    fn issue_id(raw: &str) -> IssueId {
        IssueId(raw.to_owned())
    }

    /// Property: Reducer is deterministic
    /// Running reduce with the same state and event always produces the same result
    #[test]
    fn property_reduce_is_deterministic() {
        for _ in 0..100 {
            let state = generate_random_state();
            let event = generate_random_event();

            let (state1, cmds1) = reduce(state.clone(), event.clone());
            let (state2, cmds2) = reduce(state, event);

            assert_eq!(state1, state2, "State must be deterministic");
            assert_eq!(cmds1, cmds2, "Commands must be deterministic");
        }
    }

    /// Property: Running implies claimed invariant holds after any reduce
    #[test]
    fn property_running_implies_claimed_maintained() {
        for _ in 0..100 {
            let state = generate_valid_state();
            let event = generate_random_event();

            let (new_state, _) = reduce(state, event);

            // Check invariant
            for issue_id in new_state.running.keys() {
                assert!(
                    new_state.claimed.contains(issue_id),
                    "Running issue {:?} must be claimed",
                    issue_id
                );
            }
        }
    }

    /// Property: No running and retrying simultaneously
    #[test]
    fn property_no_running_and_retrying() {
        for _ in 0..100 {
            let state = generate_valid_state();
            let event = generate_random_event();

            let (new_state, _) = reduce(state, event);

            let running_set: HashSet<_> = new_state.running.keys().collect();
            let retry_set: HashSet<_> = new_state.retry_attempts.keys().collect();

            let intersection: Vec<_> = running_set.intersection(&retry_set).collect();
            assert!(
                intersection.is_empty(),
                "No issue should be both running and retrying: {:?}",
                intersection
            );
        }
    }

    /// Property: Retry attempts are positive
    #[test]
    fn property_retry_attempts_positive() {
        for _ in 0..100 {
            let state = generate_valid_state();
            let event = generate_random_event();

            let (new_state, _) = reduce(state, event);

            for (issue_id, retry_entry) in &new_state.retry_attempts {
                assert!(
                    retry_entry.attempt >= 1,
                    "Retry attempt for {:?} must be >= 1, got {}",
                    issue_id,
                    retry_entry.attempt
                );
            }
        }
    }

    /// Property: Release clears all tracking
    #[test]
    fn property_release_clears_all() {
        for _ in 0..100 {
            let issue_id = issue_id(&format!("SYM-{}", simple_random()));
            let mut state = generate_valid_state();

            // Add issue to all tracking structures
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), RunningEntry::default());
            state.retry_attempts.insert(
                issue_id.clone(),
                RetryEntry {
                    attempt: 1,
                    ..RetryEntry::default()
                },
            );

            let (new_state, _) = reduce(state, Event::Release(issue_id.clone()));

            assert!(
                !new_state.claimed.contains(&issue_id),
                "Claim must be released"
            );
            assert!(
                !new_state.running.contains_key(&issue_id),
                "Running must be cleared"
            );
            assert!(
                !new_state.retry_attempts.contains_key(&issue_id),
                "Retry must be cleared"
            );
        }
    }

    /// Property: Mark running requires claim
    #[test]
    fn property_mark_running_requires_claim() {
        for _ in 0..100 {
            let state = OrchestratorState::default();
            let issue_id = issue_id(&format!("SYM-{}", simple_random()));

            let (new_state, commands) = reduce(state, Event::MarkRunning(issue_id.clone()));

            // Should reject transition
            assert_eq!(commands.len(), 1);
            match &commands[0] {
                Command::TransitionRejected {
                    issue_id: id,
                    reason,
                } => {
                    assert_eq!(id, &issue_id);
                    assert!(matches!(reason, TransitionRejection::MissingClaim));
                }
                _ => panic!("Expected TransitionRejected command"),
            }

            // State should be unchanged
            assert_eq!(new_state, OrchestratorState::default());
        }
    }

    /// Property: Retry attempts increase monotonically
    #[test]
    fn property_retry_attempts_monotonic() {
        for _ in 0..50 {
            let issue_id = issue_id(&format!("SYM-{}", simple_random()));
            let mut state = OrchestratorState::default();
            state.claimed.insert(issue_id.clone());

            let mut last_attempt = 0;

            for attempt in 1..=5 {
                let (new_state, commands) = reduce(
                    state.clone(),
                    Event::QueueRetry {
                        issue_id: issue_id.clone(),
                        attempt,
                    },
                );

                if attempt > last_attempt {
                    // Increasing attempt should succeed
                    assert!(matches!(&commands[0], Command::ScheduleRetry { .. }));
                    state = new_state;
                    if let Some(e) = state.retry_attempts.get(&issue_id) {
                        last_attempt = e.attempt;
                    }
                } else {
                    // Non-increasing attempt should be rejected
                    assert!(matches!(&commands[0], Command::TransitionRejected { .. }));
                }
            }
        }
    }

    /// Property: No duplicate claims
    #[test]
    fn property_no_duplicate_claims() {
        let issue_id = issue_id("SYM-DUP");
        let state = OrchestratorState::default();

        // First claim succeeds
        let (state1, cmds1) = reduce(state.clone(), Event::Claim(issue_id.clone()));
        assert!(state1.claimed.contains(&issue_id));
        assert!(cmds1.is_empty());

        // Second claim on same issue is rejected
        let (state2, cmds2) = reduce(state1.clone(), Event::Claim(issue_id.clone()));
        assert_eq!(cmds2.len(), 1);
        match &cmds2[0] {
            Command::TransitionRejected {
                issue_id: id,
                reason,
            } => {
                assert_eq!(id, &issue_id);
                assert!(matches!(reason, TransitionRejection::AlreadyClaimed));
            }
            _ => panic!("Expected TransitionRejected command"),
        }
        assert_eq!(state1, state2); // State unchanged
    }

    // Helper functions for generating random test data
    fn generate_random_state() -> OrchestratorState {
        let mut state = OrchestratorState::default();

        // Add some random claims
        for i in 0..simple_random() % 5 {
            state.claimed.insert(issue_id(&format!("SYM-{}", i)));
        }

        // Add some random running entries
        for i in 0..simple_random() % 3 {
            let id = issue_id(&format!("SYM-{}", i + 10));
            state.claimed.insert(id.clone());
            state.running.insert(id, RunningEntry::default());
        }

        // Add some random retry entries
        for i in 0..simple_random() % 2 {
            let id = issue_id(&format!("SYM-{}", i + 20));
            state.claimed.insert(id.clone());
            state.retry_attempts.insert(
                id,
                RetryEntry {
                    attempt: i + 1,
                    ..RetryEntry::default()
                },
            );
        }

        state
    }

    fn generate_valid_state() -> OrchestratorState {
        let state = generate_random_state();
        // Ensure invariants hold
        validate_invariants(&state).expect("Generated state should be valid");
        state
    }

    fn generate_random_event() -> Event {
        let issue_id = issue_id(&format!("SYM-{}", simple_random()));

        match simple_random() % 5 {
            0 => Event::Claim(issue_id),
            1 => Event::MarkRunning(issue_id),
            2 => Event::Release(issue_id),
            3 => Event::QueueRetry {
                issue_id,
                attempt: simple_random() % 5 + 1,
            },
            _ => Event::UpdateAgent(Box::new(AgentUpdate {
                issue_id,
                session_id: Some("session".to_string()),
                thread_id: Some("thread".to_string()),
                turn_id: Some("turn".to_string()),
                pid: Some(12345),
                event: Some("event".to_string()),
                timestamp: Some(1234567890),
                message: Some("message".to_string()),
                usage: Some(Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    total_tokens: 150,
                }),
                rate_limits: None,
            })),
        }
    }

    fn simple_random() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};

        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }
}
