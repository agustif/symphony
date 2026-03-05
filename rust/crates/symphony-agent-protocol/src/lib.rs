#![forbid(unsafe_code)]

mod app_server_event;
mod line_origin;
mod parsed_line;
mod parser;
mod protocol_error;
mod stderr_line;

pub use app_server_event::AppServerEvent;
pub use line_origin::LineOrigin;
pub use parsed_line::{ParsedLine, stderr_message, stdout_event};
pub use parser::{decode_line, decode_stderr_line, decode_stdout_line};
pub use protocol_error::ProtocolError;
pub use stderr_line::StderrLine;

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
    fn helper_extracts_stdout_events() {
        let parsed = decode_line(LineOrigin::Stdout, r#"{"method":"turn.end"}"#)
            .expect("stdout line should decode");
        let event = stdout_event(&parsed).expect("stdout event should exist");
        assert_eq!(event.method, "turn.end");
        assert_eq!(stderr_message(&parsed), None);
    }
}
