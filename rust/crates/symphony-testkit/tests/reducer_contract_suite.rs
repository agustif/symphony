#![forbid(unsafe_code)]

use std::collections::HashSet;

use symphony_domain::{
    AgentUpdate, Event, InvariantError, OrchestratorState, RetryEntry, RunningEntry, Usage,
    invariant_catalog, invariant_descriptor, validate_invariants,
};
use symphony_testkit::{
    deterministic_event_stream, interleave_preserving_order, issue_id, issue_ids, run_trace,
    validate_trace,
};

fn update_event(
    issue_label: &str,
    session_id: &str,
    thread_id: &str,
    turn_id: &str,
    timestamp: u64,
    usage_total: u64,
) -> Event {
    let input_tokens = usage_total / 2;
    let output_tokens = usage_total - input_tokens;

    Event::UpdateAgent(Box::new(AgentUpdate {
        issue_id: issue_id(issue_label),
        session_id: Some(session_id.to_owned()),
        thread_id: Some(thread_id.to_owned()),
        turn_id: Some(turn_id.to_owned()),
        pid: Some(4242),
        event: Some("turn.update".to_owned()),
        timestamp: Some(timestamp),
        message: Some("processing".to_owned()),
        usage: Some(Usage {
            input_tokens,
            output_tokens,
            total_tokens: usage_total,
        }),
        rate_limits: Some(serde_json::json!({
            "primary": {
                "remaining": 9
            }
        })),
    }))
}

#[test]
fn invariant_catalog_is_unique_and_traceable() {
    let mut codes = HashSet::new();

    for descriptor in invariant_catalog() {
        assert!(codes.insert(descriptor.code), "duplicate invariant code");
        assert!(!descriptor.summary.is_empty());
        assert!(!descriptor.description.is_empty());
        assert!(!descriptor.spec_sections.is_empty());
        assert!(!descriptor.proof_modules.is_empty());
        assert_eq!(invariant_descriptor(descriptor.code), Some(descriptor));
    }
}

#[test]
fn public_invariant_api_matches_invalid_state_fixtures() {
    let cases = [
        (
            {
                let mut state = OrchestratorState::default();
                state
                    .running
                    .insert(issue_id("SYM-RUN"), RunningEntry::default());
                state
            },
            InvariantError::RunningWithoutClaim,
        ),
        (
            {
                let mut state = OrchestratorState::default();
                state.retry_attempts.insert(
                    issue_id("SYM-RETRY"),
                    RetryEntry {
                        attempt: 1,
                        ..RetryEntry::default()
                    },
                );
                state
            },
            InvariantError::RetryWithoutClaim,
        ),
        (
            {
                let mut state = OrchestratorState::default();
                let issue = issue_id("SYM-BOTH");
                state.claimed.insert(issue.clone());
                state.running.insert(issue.clone(), RunningEntry::default());
                state.retry_attempts.insert(
                    issue,
                    RetryEntry {
                        attempt: 1,
                        ..RetryEntry::default()
                    },
                );
                state
            },
            InvariantError::RunningAndRetrying,
        ),
        (
            {
                let mut state = OrchestratorState::default();
                let issue = issue_id("SYM-ZERO");
                state.claimed.insert(issue.clone());
                state.retry_attempts.insert(
                    issue,
                    RetryEntry {
                        attempt: 0,
                        ..RetryEntry::default()
                    },
                );
                state
            },
            InvariantError::RetryAttemptMustBePositive,
        ),
    ];

    for (state, expected_error) in cases {
        let error = validate_invariants(&state).expect_err("state should violate an invariant");
        assert_eq!(error, expected_error);

        let descriptor = invariant_descriptor(error.code())
            .expect("every public invariant error must resolve to catalog metadata");
        assert_eq!(descriptor.code, error.code());
    }
}

#[test]
fn lifecycle_corpus_traces_validate_against_public_contracts() {
    let scenarios = vec![
        vec![
            Event::Claim(issue_id("SYM-100")),
            Event::MarkRunning(issue_id("SYM-100")),
            update_event(
                "SYM-100",
                "session-1",
                "thread-1",
                "turn-1",
                1_700_000_000,
                100,
            ),
            Event::QueueRetry {
                issue_id: issue_id("SYM-100"),
                attempt: 1,
            },
            Event::MarkRunning(issue_id("SYM-100")),
            update_event(
                "SYM-100",
                "session-2",
                "thread-2",
                "turn-1",
                1_700_000_010,
                40,
            ),
            Event::Release(issue_id("SYM-100")),
        ],
        vec![
            Event::Claim(issue_id("SYM-200")),
            Event::Claim(issue_id("SYM-200")),
            Event::MarkRunning(issue_id("SYM-200")),
            Event::MarkRunning(issue_id("SYM-200")),
            Event::QueueRetry {
                issue_id: issue_id("SYM-200"),
                attempt: 0,
            },
            Event::QueueRetry {
                issue_id: issue_id("SYM-200"),
                attempt: 1,
            },
            Event::Release(issue_id("SYM-200")),
            Event::Release(issue_id("SYM-200")),
        ],
        vec![
            update_event(
                "SYM-300",
                "session-1",
                "thread-1",
                "turn-1",
                1_700_000_000,
                25,
            ),
            Event::Claim(issue_id("SYM-300")),
            Event::MarkRunning(issue_id("SYM-300")),
            update_event(
                "SYM-300",
                "session-1",
                "thread-1",
                "turn-1",
                1_700_000_001,
                25,
            ),
            Event::Release(issue_id("SYM-300")),
        ],
    ];

    for scenario in scenarios {
        let trace = run_trace(scenario);
        assert_eq!(validate_trace(&trace), Ok(()));
    }
}

#[test]
fn deterministic_event_streams_preserve_trace_contracts_across_seeds() {
    let pool = issue_ids("SYM", 4);

    for seed in 0..64 {
        let events = deterministic_event_stream(&pool, 64, seed);
        let trace = run_trace(events);
        assert_eq!(validate_trace(&trace), Ok(()), "seed {}", seed);
    }
}

#[test]
fn interleaved_lifecycles_preserve_trace_contracts() {
    let left = vec![
        Event::Claim(issue_id("SYM-L")),
        Event::MarkRunning(issue_id("SYM-L")),
        Event::Release(issue_id("SYM-L")),
    ];
    let right = vec![
        Event::Claim(issue_id("SYM-R")),
        Event::MarkRunning(issue_id("SYM-R")),
        Event::QueueRetry {
            issue_id: issue_id("SYM-R"),
            attempt: 1,
        },
        Event::Release(issue_id("SYM-R")),
    ];

    for schedule in interleave_preserving_order(&left, &right, 20) {
        let trace = run_trace(schedule);
        assert_eq!(validate_trace(&trace), Ok(()));
    }
}
