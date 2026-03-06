#![forbid(unsafe_code)]

use symphony_agent_protocol::{
    LineOrigin, ProtocolError, ProtocolFailureReason, ProtocolPolicyOutcome, ProtocolSequenceError,
    StreamLineParser, SupportedToolSpec, build_initialize_request, build_session_id,
    classify_policy_outcome, decode_line, decode_stdout_line, extract_rate_limits,
    extract_thread_id, extract_tool_call_id, extract_tool_name, extract_turn_id, extract_usage,
    stderr_message, stdout_event, validate_startup_turn_sequence,
};
use symphony_testkit::{protocol_stderr_line, protocol_stdout_line};

#[test]
fn protocol_stdout_lines_decode_into_events() {
    let line = protocol_stdout_line("turn.start");
    let event = decode_stdout_line(&line).expect("stdout line should decode");
    assert_eq!(event.method, "turn.start");
}

#[test]
fn protocol_line_helpers_separate_stdout_and_stderr() {
    let stdout = decode_line(LineOrigin::Stdout, &protocol_stdout_line("turn.end"))
        .expect("stdout line should decode");
    assert_eq!(
        stdout_event(&stdout).map(|event| event.method.as_str()),
        Some("turn.end")
    );
    assert_eq!(stderr_message(&stdout), None);

    let stderr = decode_line(
        LineOrigin::Stderr,
        &protocol_stderr_line("panic: missing token"),
    )
    .expect("stderr line should decode");
    assert_eq!(stdout_event(&stderr), None);
    assert_eq!(stderr_message(&stderr), Some("panic: missing token"));
}

#[test]
fn protocol_rejects_invalid_stdout_json() {
    let error = decode_stdout_line("not-json").expect_err("invalid json should fail");
    assert!(matches!(error, ProtocolError::InvalidStdoutLine(_)));
}

#[test]
fn protocol_stream_preserves_stdout_sequence_for_multi_line_chunks() {
    let mut parser = StreamLineParser::default();
    let chunk = format!(
        "{}\n{}\n",
        protocol_stdout_line("turn.start"),
        protocol_stdout_line("turn.end")
    );

    let decoded = parser.push_chunk(LineOrigin::Stdout, &chunk);
    assert_eq!(decoded.len(), 2);

    let methods = decoded
        .into_iter()
        .map(|result| {
            let parsed = result.expect("stdout line should parse");
            stdout_event(&parsed)
                .map(|event| event.method.clone())
                .expect("decoded line should be stdout event")
        })
        .collect::<Vec<_>>();

    assert_eq!(methods, vec!["turn.start", "turn.end"]);
}

#[test]
fn protocol_stream_keeps_interleaved_stdout_and_stderr_state_isolated() {
    let mut parser = StreamLineParser::default();
    let partial_stdout = r#"{"method":"turn.start""#;
    let complete_stdout_suffix = "}\n";

    let first = parser.push_chunk(LineOrigin::Stdout, partial_stdout);
    assert!(first.is_empty());
    assert_eq!(parser.pending_stdout(), partial_stdout);

    let stderr = parser.push_chunk(LineOrigin::Stderr, "warning: noisy channel\n");
    assert_eq!(stderr.len(), 1);
    let stderr_line = stderr[0].as_ref().expect("stderr line should decode");
    assert_eq!(stderr_message(stderr_line), Some("warning: noisy channel"));
    assert_eq!(parser.pending_stdout(), partial_stdout);

    let second = parser.push_chunk(LineOrigin::Stdout, complete_stdout_suffix);
    assert_eq!(second.len(), 1);
    let event = second[0]
        .as_ref()
        .ok()
        .and_then(stdout_event)
        .expect("stdout event should decode after completion");
    assert_eq!(event.method, "turn.start");
}

#[test]
fn protocol_stream_finishes_interrupted_stdout_and_stderr_without_newlines() {
    let mut parser = StreamLineParser::default();

    assert!(
        parser
            .push_chunk(
                LineOrigin::Stdout,
                r#"{"id":11,"result":{"thread":{"id":"thread-int"}}}"#,
            )
            .is_empty()
    );
    assert!(
        parser
            .push_chunk(LineOrigin::Stderr, "warn: tail chunk")
            .is_empty()
    );

    let trailing = parser.finish();
    assert_eq!(trailing.len(), 2);

    let stdout = trailing[0]
        .as_ref()
        .expect("stdout trailing line should decode");
    let event = stdout_event(stdout).expect("stdout event should exist");
    assert_eq!(extract_thread_id(event), Some("thread-int".to_owned()));

    let stderr = trailing[1]
        .as_ref()
        .expect("stderr trailing line should decode");
    assert_eq!(stderr_message(stderr), Some("warn: tail chunk"));
}

#[test]
fn protocol_handshake_sequence_accepts_startup_then_turn_lifecycle() {
    let methods = [
        "initialize",
        "initialized",
        "session/new",
        "turn/start",
        "turn/completed",
    ];
    assert!(validate_startup_turn_sequence(methods).is_ok());
}

#[test]
fn protocol_handshake_sequence_rejects_turn_before_session_start() {
    let methods = ["initialize", "initialized", "turn/start"];
    let error = validate_startup_turn_sequence(methods).expect_err("must fail");
    assert_eq!(
        error,
        ProtocolSequenceError::UnexpectedStartupMethod {
            expected: "thread/start or session/new",
            observed: "turn/start".to_owned(),
        }
    );
}

#[test]
fn protocol_policy_maps_input_required_to_permanent_failure() {
    let event = decode_stdout_line(r#"{"method":"item/tool/requestUserInput","params":{}}"#)
        .expect("input-required event should decode");
    assert_eq!(
        classify_policy_outcome(&event),
        Some(ProtocolPolicyOutcome::PermanentFailure(
            ProtocolFailureReason::TurnInputRequired
        ))
    );
}

#[test]
fn protocol_policy_maps_turn_timeout_to_retryable_failure() {
    let event = decode_stdout_line(
        r#"{"method":"turn/failed","error":{"code":"turn_timeout","message":"timed out"}}"#,
    )
    .expect("turn-failed timeout event should decode");
    assert_eq!(
        classify_policy_outcome(&event),
        Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::TurnTimeout
        ))
    );
}

#[test]
fn protocol_policy_does_not_hard_fail_unsupported_tool_call_marker_event() {
    let event = decode_stdout_line(r#"{"method":"unsupported_tool_call","params":{}}"#)
        .expect("unsupported-tool event should decode");
    assert_eq!(classify_policy_outcome(&event), None);
}

#[test]
fn protocol_policy_maps_response_timeout_to_retryable_failure() {
    let event =
        decode_stdout_line(r#"{"method":"startup/failed","error":{"code":"response_timeout"}}"#)
            .expect("response-timeout event should decode");
    assert_eq!(
        classify_policy_outcome(&event),
        Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::ResponseTimeout
        ))
    );
}

#[test]
fn protocol_policy_maps_methodless_error_timeout_envelope_to_retryable_failure() {
    let event = decode_stdout_line(r#"{"id":4,"error":{"reason":"handshake_timeout"}}"#)
        .expect("methodless timeout envelope should decode");
    assert_eq!(
        classify_policy_outcome(&event),
        Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::ResponseTimeout
        ))
    );
}

#[test]
fn protocol_initialize_payload_advertises_supported_tools() {
    let initialize = build_initialize_request(
        serde_json::json!(1),
        "symphony-rust",
        "0.1.0",
        &[SupportedToolSpec::new("linear_graphql")],
    );
    assert_eq!(initialize["method"], "initialize");
    let supported = initialize["params"]["capabilities"]["tools"]["supported"]
        .as_array()
        .expect("supported tools should be advertised");
    assert_eq!(supported.len(), 1);
    assert_eq!(supported[0]["name"], "linear_graphql");
}

#[test]
fn protocol_initialize_payload_preserves_supported_tool_hook_points() {
    let initialize = build_initialize_request(
        serde_json::json!(1),
        "symphony-rust",
        "0.1.0",
        &[SupportedToolSpec::new("linear_graphql").with_hook_points(["before_turn"])],
    );
    assert_eq!(
        initialize["params"]["capabilities"]["tools"]["supported"][0]["hookPoints"][0],
        serde_json::json!("before_turn")
    );
}

#[test]
fn protocol_extractors_read_nested_thread_turn_ids_and_build_session_id() {
    let event = decode_stdout_line(
        r#"{"method":"thread/start","result":{"thread":{"id":"thread-42"},"turn":{"id":"turn-99"}}}"#,
    )
    .expect("thread-start result should decode");
    let thread_id = extract_thread_id(&event);
    let turn_id = extract_turn_id(&event);

    assert_eq!(thread_id.as_deref(), Some("thread-42"));
    assert_eq!(turn_id.as_deref(), Some("turn-99"));
    assert_eq!(
        build_session_id(thread_id.as_deref(), turn_id.as_deref()),
        Some("thread-42-turn-99".to_owned())
    );
}

#[test]
fn protocol_extractors_read_nested_session_metadata_variants() {
    let event = decode_stdout_line(
        r#"{"id":2,"result":{"session":{"threadId":"thread-7","turn":{"id":"turn-8"}}}}"#,
    )
    .expect("session metadata payload should decode");
    let thread_id = extract_thread_id(&event);
    let turn_id = extract_turn_id(&event);

    assert_eq!(thread_id.as_deref(), Some("thread-7"));
    assert_eq!(turn_id.as_deref(), Some("turn-8"));
    assert_eq!(
        build_session_id(thread_id.as_deref(), turn_id.as_deref()),
        Some("thread-7-turn-8".to_owned())
    );
}

#[test]
fn protocol_usage_extractor_accepts_usage_shape_variants() {
    let event = decode_stdout_line(
        r#"{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"total":{"inputTokens":120,"completionTokens":30}}}}"#,
    )
    .expect("usage payload should decode");
    let usage = extract_usage(&event).expect("usage should be extracted");
    assert_eq!(usage.input_tokens, 120);
    assert_eq!(usage.output_tokens, 30);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn protocol_extractors_read_tool_call_id_and_name_variants() {
    let event = decode_stdout_line(
        r#"{"id":"call-9","method":"item/tool/call","params":{"tool":{"name":"linear_graphql"}}}"#,
    )
    .expect("tool-call payload should decode");
    assert_eq!(
        extract_tool_call_id(&event),
        Some(serde_json::json!("call-9"))
    );
    assert_eq!(extract_tool_name(&event), Some("linear_graphql".to_owned()));
}

#[test]
fn protocol_extracts_rate_limits_from_wrapper_and_bucket_variants() {
    let event = decode_stdout_line(
        r#"{"method":"notification","params":{"meta":{"rateLimitStatus":{"requests":{"remaining":3,"limit":9,"resetAt":"2026-03-06T18:00:00Z"}}}}}"#,
    )
    .expect("rate-limit payload should decode");
    assert_eq!(
        extract_rate_limits(&event),
        Some(serde_json::json!({
            "requests": {
                "remaining": 3,
                "limit": 9,
                "resetAt": "2026-03-06T18:00:00Z"
            }
        }))
    );
}

#[test]
fn protocol_stream_survives_high_volume_fragmented_stdout_chunks() {
    let mut parser = StreamLineParser::default();
    let total_events = 2_000usize;
    let stream = (0..total_events)
        .map(|index| format!(r#"{{"method":"turn/delta","params":{{"seq":{index}}}}}"#))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let mut decoded = Vec::new();
    let chunk_pattern = [1usize, 2, 3, 5, 8, 13, 21, 34, 55];
    let mut offset = 0usize;
    let mut step = 0usize;
    while offset < stream.len() {
        let width = chunk_pattern[step % chunk_pattern.len()];
        let next = (offset + width).min(stream.len());
        decoded.extend(parser.push_chunk(LineOrigin::Stdout, &stream[offset..next]));
        offset = next;
        step += 1;
    }

    assert!(parser.finish().is_empty());
    assert_eq!(decoded.len(), total_events);

    for (index, parsed) in decoded.into_iter().enumerate() {
        let parsed = parsed.expect("fragmented stdout line should decode");
        let event = stdout_event(&parsed).expect("decoded line should be stdout event");
        assert_eq!(event.method, "turn/delta");
        assert_eq!(event.params["seq"], serde_json::json!(index));
    }
}
