use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("protocol line cannot be empty")]
    EmptyLine,
    #[error("stdout protocol line is missing method")]
    MissingMethod,
    #[error("invalid stdout protocol line: {0}")]
    InvalidStdoutLine(String),
}
