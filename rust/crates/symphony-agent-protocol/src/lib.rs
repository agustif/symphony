#![forbid(unsafe_code)]

mod app_server_event;
mod event_extract;
mod event_policy;
mod line_origin;
mod parsed_line;
mod parser;
mod protocol_error;
mod protocol_method;
mod sequence_validator;
mod startup_payload;
mod stderr_line;
mod stream_line_parser;

pub use app_server_event::AppServerEvent;
pub use event_extract::{
    ProtocolUsage, build_session_id, extract_thread_id, extract_tool_call_id, extract_tool_name,
    extract_turn_id, extract_usage,
};
pub use event_policy::{ProtocolFailureReason, ProtocolPolicyOutcome, classify_policy_outcome};
pub use line_origin::LineOrigin;
pub use parsed_line::{ParsedLine, stderr_message, stdout_event};
pub use parser::{decode_line, decode_stderr_line, decode_stdout_line};
pub use protocol_error::ProtocolError;
pub use protocol_method::{ProtocolMethodCategory, ProtocolMethodKind, canonical_method_name};
pub use sequence_validator::{
    ProtocolSequenceError, ProtocolSequenceValidator, StartupPhase, validate_startup_turn_sequence,
};
pub use startup_payload::{
    SupportedToolSpec, build_initialize_request, build_initialized_notification,
    build_thread_start_request, build_turn_start_request,
};
pub use stderr_line::StderrLine;
pub use stream_line_parser::StreamLineParser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_stdout_json() {
        let line = r#"{"method":"turn.start","params":{"issue_id":"SYM-7"}}"#;
        let event = decode_stdout_line(line).expect("valid protocol line");
        assert_eq!(event.method, "turn.start");
        assert_eq!(event.params["issue_id"], "SYM-7");
    }

    #[test]
    fn parses_protocol_metadata_fields() {
        let line =
            r#"{"id":3,"method":"turn.completed","result":{"turn":{"id":"turn-1"}},"error":null}"#;
        let event = decode_stdout_line(line).expect("protocol metadata line should decode");
        assert_eq!(event.id, Some(serde_json::json!(3)));
        assert_eq!(
            event.result,
            Some(serde_json::json!({"turn": {"id": "turn-1"}}))
        );
        assert_eq!(event.error, None);
    }

    #[test]
    fn categorizes_event_method_types() {
        let line = r#"{"method":"turn/start","params":{"issue_id":"SYM-7"}}"#;
        let event = decode_stdout_line(line).expect("valid protocol line");
        assert_eq!(event.method_kind(), ProtocolMethodKind::TurnStart);
        assert_eq!(event.method_category(), ProtocolMethodCategory::Turn);
    }

    #[test]
    fn decodes_stderr_line_without_json_parsing() {
        let parsed = decode_line(LineOrigin::Stderr, "panic: file missing\n")
            .expect("stderr line should decode");
        assert_eq!(stderr_message(&parsed), Some("panic: file missing"));
        assert!(stdout_event(&parsed).is_none());
    }

    #[test]
    fn rejects_invalid_stdout_json() {
        let error = decode_stdout_line("not-json").expect_err("invalid json should fail");
        assert!(matches!(error, ProtocolError::InvalidStdoutLine(_)));
    }

    #[test]
    fn rejects_malformed_event_envelope_with_missing_method() {
        let error = decode_stdout_line(r#"{"params":{"issue_id":"SYM-7"}}"#)
            .expect_err("missing method should fail");
        assert!(matches!(error, ProtocolError::InvalidStdoutLine(_)));
    }

    #[test]
    fn rejects_malformed_event_envelope_with_non_string_method() {
        let error = decode_stdout_line(r#"{"method":123,"params":{"issue_id":"SYM-7"}}"#)
            .expect_err("non-string method should fail");
        assert!(matches!(error, ProtocolError::InvalidStdoutLine(_)));
    }

    #[test]
    fn helper_extracts_stdout_events() {
        let parsed = decode_line(LineOrigin::Stdout, r#"{"method":"turn.end"}"#)
            .expect("stdout line should decode");
        let event = stdout_event(&parsed).expect("stdout event should exist");
        assert_eq!(event.method, "turn.end");
        assert_eq!(stderr_message(&parsed), None);
    }

    #[test]
    fn stream_parser_buffers_partial_stdout_lines() {
        let mut parser = StreamLineParser::default();
        let first = parser.push_chunk(LineOrigin::Stdout, r#"{"method":"turn.st"#);
        assert!(first.is_empty());
        assert_eq!(parser.pending_stdout(), r#"{"method":"turn.st"#);

        let second = parser.push_chunk(LineOrigin::Stdout, "art\",\"params\":{\"n\":1}}\n");
        assert_eq!(second.len(), 1);

        let parsed = second
            .into_iter()
            .next()
            .expect("expected one decoded line")
            .expect("stdout event should parse");
        let event = stdout_event(&parsed).expect("stdout event should exist");
        assert_eq!(event.method, "turn.start");
        assert_eq!(event.params["n"], 1);
    }

    #[test]
    fn stream_parser_keeps_stdout_and_stderr_split() {
        let mut parser = StreamLineParser::default();

        let stdout = parser.push_chunk(LineOrigin::Stdout, "{\"method\":\"turn.end\"}\n");
        assert_eq!(stdout.len(), 1);
        assert!(stdout[0].as_ref().is_ok());

        let stderr_partial = parser.push_chunk(LineOrigin::Stderr, "panic: missing");
        assert!(stderr_partial.is_empty());
        assert_eq!(parser.pending_stderr(), "panic: missing");

        let stderr_complete = parser.push_chunk(LineOrigin::Stderr, " token\n");
        assert_eq!(stderr_complete.len(), 1);
        let parsed = stderr_complete
            .into_iter()
            .next()
            .expect("expected one stderr line")
            .expect("stderr line should decode");
        assert_eq!(stderr_message(&parsed), Some("panic: missing token"));
    }

    #[test]
    fn stream_parser_reports_malformed_stdout_line() {
        let mut parser = StreamLineParser::default();
        let parsed = parser.push_chunk(LineOrigin::Stdout, "{not-json}\n");
        assert_eq!(parsed.len(), 1);

        let error = parsed
            .into_iter()
            .next()
            .expect("expected one parsed result")
            .expect_err("line should be reported as malformed");

        assert!(matches!(
            error,
            ProtocolError::InvalidStdoutLine(message) if message.contains("line `{not-json}`")
        ));
    }
}
