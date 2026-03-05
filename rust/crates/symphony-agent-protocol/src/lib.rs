#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub method: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("invalid protocol line")]
    InvalidLine,
}

pub fn decode_stdout_line(line: &str) -> Result<ProtocolMessage, ProtocolError> {
    serde_json::from_str::<ProtocolMessage>(line).map_err(|_| ProtocolError::InvalidLine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_stdout_json() {
        let line = r#"{"method":"turn.start"}"#;
        let msg = decode_stdout_line(line).expect("valid protocol line");
        assert_eq!(msg.method, "turn.start");
    }
}
