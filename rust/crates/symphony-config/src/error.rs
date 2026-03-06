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
    #[error("`hooks.timeout_ms` must be > 0")]
    InvalidHookTimeout,
    #[error("`agent.max_concurrent_agents` must be > 0")]
    InvalidMaxConcurrentAgents,
    #[error("`agent.max_turns` must be > 0")]
    InvalidMaxTurns,
    #[error("`agent.max_retry_backoff_ms` must be > 0")]
    InvalidMaxRetryBackoff,
    #[error("`codex.turn_timeout_ms` must be > 0")]
    InvalidCodexTurnTimeout,
    #[error("`codex.read_timeout_ms` must be > 0")]
    InvalidCodexReadTimeout,
    #[error("`codex.stall_timeout_ms` must be > 0")]
    InvalidCodexStallTimeout,
    #[error("`workspace.root` must resolve to a non-empty path")]
    InvalidWorkspaceRoot,
    #[error("`tracker.kind` is required")]
    MissingTrackerKind,
    #[error("`tracker.active_states` must contain at least one state")]
    MissingTrackerActiveStates,
    #[error("unsupported tracker kind: {0}")]
    UnsupportedTrackerKind(String),
    #[error("`tracker.project_slug` is required for linear tracker")]
    MissingTrackerProjectSlug,
    #[error("`agent.max_concurrent_agents_by_state` contains an invalid or empty state key")]
    InvalidStateLimitKey,
    #[error("invalid state limit for `{state}`: {value}")]
    InvalidStateLimitValue { state: String, value: String },
    #[error("state limit references unknown active state: {0}")]
    UnknownStateLimitState(String),
    #[error("state limit for `{state}` ({limit}) exceeds `agent.max_concurrent_agents` ({max})")]
    StateLimitExceedsMaxConcurrent { state: String, limit: u32, max: u32 },
    #[error("`codex.command` must be non-empty")]
    MissingCodexCommand,
    #[error("invalid CLI override for `{field}`: {reason}")]
    InvalidCliOverride { field: String, reason: String },
}
