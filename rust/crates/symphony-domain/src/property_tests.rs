use proptest::{collection::vec, prelude::*, sample::select};

use crate::{
    AgentUpdate, Command, Event, IssueId, OrchestratorState, TransitionRejection, Usage, reduce,
    validate_invariants,
};

#[derive(Clone, Debug)]
struct ReductionStep {
    before: OrchestratorState,
    event: Event,
    after: OrchestratorState,
    commands: Vec<Command>,
}

fn arb_issue_id() -> impl Strategy<Value = IssueId> {
    select(vec!["SYM-1", "SYM-2", "SYM-3", "SYM-4", "SYM-5"])
        .prop_map(|label| IssueId(label.to_owned()))
}

fn arb_usage() -> impl Strategy<Value = Usage> {
    (0u64..500, 0u64..500).prop_map(|(input_tokens, output_tokens)| Usage {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens + output_tokens,
    })
}

fn arb_rate_limits() -> impl Strategy<Value = Option<serde_json::Value>> {
    prop_oneof![
        Just(None),
        (0u8..100u8).prop_map(|remaining| {
            Some(serde_json::json!({
                "primary": {
                    "remaining": remaining
                }
            }))
        }),
    ]
}

fn arb_agent_update() -> impl Strategy<Value = AgentUpdate> {
    (
        arb_issue_id(),
        prop::option::of(0u8..4u8),
        prop::option::of(0u8..4u8),
        prop::option::of(0u8..6u8),
        prop::option::of(1000u32..2000u32),
        prop::option::of(select(vec!["turn/start", "turn/update", "turn/end"])),
        prop::option::of(1_700_000_000u64..1_700_000_100u64),
        prop::option::of(select(vec!["processing", "retrying", "complete"])),
        prop::option::of(arb_usage()),
        arb_rate_limits(),
    )
        .prop_map(
            |(
                issue_id,
                session_suffix,
                thread_suffix,
                turn_suffix,
                pid,
                event,
                timestamp,
                message,
                usage,
                rate_limits,
            )| AgentUpdate {
                issue_id,
                session_id: session_suffix.map(|suffix| format!("session-{suffix}")),
                thread_id: thread_suffix.map(|suffix| format!("thread-{suffix}")),
                turn_id: turn_suffix.map(|suffix| format!("turn-{suffix}")),
                pid,
                event: event.map(str::to_owned),
                timestamp,
                message: message.map(str::to_owned),
                usage,
                rate_limits,
            },
        )
}

fn arb_event() -> impl Strategy<Value = Event> {
    prop_oneof![
        3 => arb_issue_id().prop_map(Event::Claim),
        3 => arb_issue_id().prop_map(Event::MarkRunning),
        3 => (arb_issue_id(), 0u32..6u32).prop_map(|(issue_id, attempt)| Event::QueueRetry {
            issue_id,
            attempt,
        }),
        2 => arb_issue_id().prop_map(Event::Release),
        4 => arb_agent_update().prop_map(|update| Event::UpdateAgent(Box::new(update))),
    ]
}

fn arb_event_stream() -> impl Strategy<Value = Vec<Event>> {
    vec(arb_event(), 1..64)
}

fn replay(events: &[Event]) -> Vec<ReductionStep> {
    let mut state = OrchestratorState::default();
    let mut steps = Vec::with_capacity(events.len());

    for event in events {
        let before = state.clone();
        let (after, commands) = reduce(state, event.clone());
        steps.push(ReductionStep {
            before,
            event: event.clone(),
            after: after.clone(),
            commands,
        });
        state = after;
    }

    steps
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        .. ProptestConfig::default()
    })]

    #[test]
    fn property_event_streams_preserve_invariants(events in arb_event_stream()) {
        for step in replay(&events) {
            prop_assert_eq!(validate_invariants(&step.after), Ok(()));
        }
    }

    #[test]
    fn property_reducer_is_deterministic_for_generated_events(events in arb_event_stream()) {
        let first = replay(&events);
        let second = replay(&events);

        prop_assert_eq!(first.len(), second.len());
        for (left, right) in first.iter().zip(second.iter()) {
            prop_assert_eq!(&left.after, &right.after);
            prop_assert_eq!(&left.commands, &right.commands);
        }
    }

    #[test]
    fn property_transition_rejections_are_state_preserving(events in arb_event_stream()) {
        for step in replay(&events) {
            if matches!(
                step.commands.first(),
                Some(Command::TransitionRejected {
                    reason: TransitionRejection::AlreadyClaimed
                        | TransitionRejection::MissingClaim
                        | TransitionRejection::AlreadyRunning
                        | TransitionRejection::InvalidRetryAttempt
                        | TransitionRejection::RetryAttemptRegression,
                    ..
                })
            ) {
                prop_assert_eq!(step.before, step.after);
            }
        }
    }

    #[test]
    fn property_codex_totals_are_monotonic_across_generated_updates(events in arb_event_stream()) {
        let mut previous_totals = crate::CodexTotals::default();

        for step in replay(&events) {
            prop_assert!(step.after.codex_totals.input_tokens >= previous_totals.input_tokens);
            prop_assert!(step.after.codex_totals.output_tokens >= previous_totals.output_tokens);
            prop_assert!(step.after.codex_totals.total_tokens >= previous_totals.total_tokens);
            previous_totals = step.after.codex_totals.clone();
        }
    }

    #[test]
    fn property_successful_agent_updates_keep_topology_stable(events in arb_event_stream()) {
        for step in replay(&events) {
            if matches!(step.event, Event::UpdateAgent(_)) && step.commands.is_empty() {
                prop_assert_eq!(step.before.claimed, step.after.claimed);
                prop_assert_eq!(step.before.running.len(), step.after.running.len());
                for issue_id in step.before.running.keys() {
                    prop_assert!(step.after.running.contains_key(issue_id));
                }
                prop_assert_eq!(step.before.retry_attempts.len(), step.after.retry_attempts.len());
                for issue_id in step.before.retry_attempts.keys() {
                    prop_assert!(step.after.retry_attempts.contains_key(issue_id));
                }
            }
        }
    }
}
