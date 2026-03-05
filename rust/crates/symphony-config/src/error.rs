use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid type for `{field}`; expected {expected}")]
    InvalidType {
        field: &'static str,
        expected: &'static str,
    },
    #[error("invalid integer for `{field}`: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("`polling.interval_ms` must be > 0")]
    InvalidPollInterval,
    #[error("`tracker.kind` is required")]
    MissingTrackerKind,
    #[error("unsupported tracker kind: {0}")]
    UnsupportedTrackerKind(String),
    #[error("`tracker.api_key` is required after env resolution")]
    MissingTrackerApiKey,
    #[error("`tracker.project_slug` is required for linear tracker")]
    MissingTrackerProjectSlug,
    #[error("`codex.command` must be non-empty")]
    MissingCodexCommand,
}
