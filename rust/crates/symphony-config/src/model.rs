use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub const DEFAULT_LINEAR_ENDPOINT: &str = "https://api.linear.app/graphql";
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 30_000;
pub const DEFAULT_WORKSPACE_DIR: &str = "symphony_workspaces";
pub const DEFAULT_HOOK_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_MAX_CONCURRENT_AGENTS: u32 = 10;
pub const DEFAULT_MAX_TURNS: u32 = 20;
pub const DEFAULT_MAX_RETRY_BACKOFF_MS: u64 = 300_000;
pub const DEFAULT_CODEX_COMMAND: &str = "codex app-server";
pub const DEFAULT_CODEX_TURN_TIMEOUT_MS: u64 = 3_600_000;
pub const DEFAULT_CODEX_READ_TIMEOUT_MS: u64 = 5_000;
pub const DEFAULT_CODEX_STALL_TIMEOUT_MS: i64 = 300_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub kind: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub project_slug: Option<String>,
    pub active_states: Vec<String>,
    pub terminal_states: Vec<String>,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            kind: "linear".to_owned(),
            endpoint: DEFAULT_LINEAR_ENDPOINT.to_owned(),
            api_key: None,
            project_slug: None,
            active_states: vec!["Todo".to_owned(), "In Progress".to_owned()],
            terminal_states: vec![
                "Closed".to_owned(),
                "Cancelled".to_owned(),
                "Canceled".to_owned(),
                "Duplicate".to_owned(),
                "Done".to_owned(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollingConfig {
    pub interval_ms: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval_ms: DEFAULT_POLL_INTERVAL_MS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub root: PathBuf,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: std::env::temp_dir().join(DEFAULT_WORKSPACE_DIR),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HooksConfig {
    pub after_create: Option<String>,
    pub before_run: Option<String>,
    pub after_run: Option<String>,
    pub before_remove: Option<String>,
    pub timeout_ms: u64,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            after_create: None,
            before_run: None,
            after_run: None,
            before_remove: None,
            timeout_ms: DEFAULT_HOOK_TIMEOUT_MS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_concurrent_agents: u32,
    pub max_turns: u32,
    pub max_retry_backoff_ms: u64,
    pub max_concurrent_agents_by_state: HashMap<String, u32>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: DEFAULT_MAX_CONCURRENT_AGENTS,
            max_turns: DEFAULT_MAX_TURNS,
            max_retry_backoff_ms: DEFAULT_MAX_RETRY_BACKOFF_MS,
            max_concurrent_agents_by_state: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodexConfig {
    pub command: String,
    pub approval_policy: Option<JsonValue>,
    pub thread_sandbox: Option<String>,
    pub turn_sandbox_policy: Option<JsonValue>,
    pub turn_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub stall_timeout_ms: i64,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            command: DEFAULT_CODEX_COMMAND.to_owned(),
            approval_policy: None,
            thread_sandbox: None,
            turn_sandbox_policy: None,
            turn_timeout_ms: DEFAULT_CODEX_TURN_TIMEOUT_MS,
            read_timeout_ms: DEFAULT_CODEX_READ_TIMEOUT_MS,
            stall_timeout_ms: DEFAULT_CODEX_STALL_TIMEOUT_MS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLevelConfig {
    pub level: String,
}

impl Default for LogLevelConfig {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub port: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    pub tracker: TrackerConfig,
    pub polling: PollingConfig,
    pub workspace: WorkspaceConfig,
    pub hooks: HooksConfig,
    pub agent: AgentConfig,
    pub codex: CodexConfig,
    pub log_level: LogLevelConfig,
    pub server: ServerConfig,
    /// Monotonically increasing version to detect config changes for hot-reload
    #[serde(default)]
    pub version: u64,
}

impl RuntimeConfig {
    /// Create a new config with an incremented version for hot-reload detection
    pub fn increment_version(&self) -> Self {
        let mut config = self.clone();
        config.version = config.version.saturating_add(1);
        config
    }
}

/// CLI override values for config fields.
///
/// These take precedence over config file values and environment variables
/// in the following order of priority:
///
/// 1. CLI arguments (highest priority)
/// 2. Environment variables ($VAR or ${VAR})
/// 3. Config file values
/// 4. Default values (lowest priority)
///
/// # Example
///
/// ```ignore
/// use symphony_config::{RuntimeConfig, CliOverrides, apply_cli_overrides};
///
/// // Load config from file
/// let config = RuntimeConfig::default();
///
/// // Create CLI overrides from user input
/// let overrides = CliOverrides {
///     polling_interval_ms: Some(60000),
///     max_concurrent_agents: Some(20),
///     log_level: Some("debug".to_string()),
///     ..Default::default()
/// };
///
/// // Apply overrides (validates the result)
/// let config = apply_cli_overrides(config, &overrides)?;
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliOverrides {
    /// Override polling interval in milliseconds
    pub polling_interval_ms: Option<u64>,
    /// Override max concurrent agents
    pub max_concurrent_agents: Option<u32>,
    /// Override max turns per agent
    pub max_turns: Option<u32>,
    /// Override max retry backoff in milliseconds
    pub max_retry_backoff_ms: Option<u64>,
    /// Override workspace root path
    pub workspace_root: Option<std::path::PathBuf>,
    /// Override log level (trace, debug, info, warn, error)
    pub log_level: Option<String>,
    /// Override tracker API endpoint
    pub tracker_endpoint: Option<String>,
    /// Override tracker API key
    pub tracker_api_key: Option<String>,
    /// Override tracker project slug
    pub tracker_project_slug: Option<String>,
    /// Override optional HTTP server port (`0` allows ephemeral local bind)
    pub server_port: Option<u16>,
}

impl CliOverrides {
    /// Create empty CliOverrides
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any overrides are set
    pub fn is_empty(&self) -> bool {
        self.polling_interval_ms.is_none()
            && self.max_concurrent_agents.is_none()
            && self.max_turns.is_none()
            && self.max_retry_backoff_ms.is_none()
            && self.workspace_root.is_none()
            && self.log_level.is_none()
            && self.tracker_endpoint.is_none()
            && self.tracker_api_key.is_none()
            && self.tracker_project_slug.is_none()
            && self.server_port.is_none()
    }
}
