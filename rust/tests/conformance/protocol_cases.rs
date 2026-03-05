#![forbid(unsafe_code)]

use symphony_agent_protocol::{
    LineOrigin, ProtocolError, decode_line, decode_stdout_line, stderr_message, stdout_event,
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
