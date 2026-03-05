#![forbid(unsafe_code)]

use symphony_agent_protocol::{
    LineOrigin, ProtocolError, ProtocolSequenceError, StreamLineParser, decode_line,
    decode_stdout_line, stderr_message, stdout_event, validate_startup_turn_sequence,
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
