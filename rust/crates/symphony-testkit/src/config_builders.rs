//! Fixture builders for configuration and workflow types.
//!
//! K1.1.1: Issue/workflow/config fixture builders for test convenience.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use serde_json::json;

/// Builder for test RuntimeConfig instances.
///
/// # Example
/// ```ignore
/// use symphony_testkit::config_builders::runtime_config;
///
/// let config = runtime_config()
///     .with_project_slug("SYM")
///     .with_max_concurrent(5)
///     .build();
///
/// assert_eq!(config.tracker.project_slug, Some("SYM".to_owned()));
/// assert_eq!(config.agent.max_concurrent_agents, 5);
/// ```
pub struct RuntimeConfigBuilder {
    tracker_kind: String,
    tracker_endpoint: String,
    tracker_api_key: Option<String>,
    tracker_project_slug: Option<String>,
    tracker_active_states: Vec<String>,
    tracker_terminal_states: Vec<String>,
    poll_interval_ms: u64,
    workspace_root: PathBuf,
    max_concurrent_agents: u32,
    max_turns: u32,
    max_retry_backoff_ms: u64,
    codex_command: String,
    codex_approval_policy: Option<serde_json::Value>,
    codex_thread_sandbox: Option<String>,
    codex_turn_timeout_ms: u64,
    hook_timeout_ms: u64,
    server_port: Option<u16>,
    log_level: String,
}

impl Default for RuntimeConfigBuilder {
    fn default() -> Self {
        Self {
            tracker_kind: "linear".to_owned(),
            tracker_endpoint: "https://api.linear.app/graphql".to_owned(),
            tracker_api_key: None,
            tracker_project_slug: None,
            tracker_active_states: vec!["Todo".to_owned(), "In Progress".to_owned()],
            tracker_terminal_states: vec![
                "Closed".to_owned(),
                "Cancelled".to_owned(),
                "Canceled".to_owned(),
                "Duplicate".to_owned(),
                "Done".to_owned(),
            ],
            poll_interval_ms: 30_000,
            workspace_root: std::env::temp_dir().join("symphony_test_workspaces"),
            max_concurrent_agents: 10,
            max_turns: 20,
            max_retry_backoff_ms: 300_000,
            codex_command: "codex app-server".to_owned(),
            codex_approval_policy: Some(json!({"reject": {"sandbox_approval": true}})),
            codex_thread_sandbox: Some("workspace-write".to_owned()),
            codex_turn_timeout_ms: 3_600_000,
            hook_timeout_ms: 60_000,
            server_port: None,
            log_level: "info".to_owned(),
        }
    }
}

impl RuntimeConfigBuilder {
    /// Set the tracker project slug.
    pub fn with_project_slug(mut self, slug: impl Into<String>) -> Self {
        self.tracker_project_slug = Some(slug.into());
        self
    }

    /// Set the tracker API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.tracker_api_key = Some(key.into());
        self
    }

    /// Set the maximum concurrent agents.
    pub fn with_max_concurrent(mut self, max: u32) -> Self {
        self.max_concurrent_agents = max;
        self
    }

    /// Set the poll interval in milliseconds.
    pub fn with_poll_interval_ms(mut self, ms: u64) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    /// Set the workspace root path.
    pub fn with_workspace_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace_root = path.into();
        self
    }

    /// Set the server port.
    pub fn with_server_port(mut self, port: u16) -> Self {
        self.server_port = Some(port);
        self
    }

    /// Set the log level.
    pub fn with_log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = level.into();
        self
    }

    /// Set the active states for the tracker.
    pub fn with_active_states(mut self, states: Vec<&str>) -> Self {
        self.tracker_active_states = states.into_iter().map(|s| s.to_owned()).collect();
        self
    }

    /// Set the terminal states for the tracker.
    pub fn with_terminal_states(mut self, states: Vec<&str>) -> Self {
        self.tracker_terminal_states = states.into_iter().map(|s| s.to_owned()).collect();
        self
    }

    /// Set max turns per session.
    pub fn with_max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Build the RuntimeConfig.
    pub fn build(self) -> symphony_config::RuntimeConfig {
        symphony_config::RuntimeConfig {
            tracker: symphony_config::TrackerConfig {
                kind: self.tracker_kind,
                endpoint: self.tracker_endpoint,
                api_key: self.tracker_api_key,
                project_slug: self.tracker_project_slug,
                active_states: self.tracker_active_states,
                terminal_states: self.tracker_terminal_states,
            },
            polling: symphony_config::PollingConfig {
                interval_ms: self.poll_interval_ms,
            },
            workspace: symphony_config::WorkspaceConfig {
                root: self.workspace_root,
            },
            hooks: symphony_config::HooksConfig {
                after_create: None,
                before_run: None,
                after_run: None,
                before_remove: None,
                timeout_ms: self.hook_timeout_ms,
            },
            agent: symphony_config::AgentConfig {
                max_concurrent_agents: self.max_concurrent_agents,
                max_turns: self.max_turns,
                max_retry_backoff_ms: self.max_retry_backoff_ms,
                max_concurrent_agents_by_state: HashMap::new(),
            },
            codex: symphony_config::CodexConfig {
                command: self.codex_command,
                approval_policy: self.codex_approval_policy,
                thread_sandbox: self.codex_thread_sandbox,
                turn_sandbox_policy: None,
                turn_timeout_ms: self.codex_turn_timeout_ms,
                read_timeout_ms: 5_000,
                stall_timeout_ms: 300_000,
            },
            log_level: symphony_config::LogLevelConfig {
                level: self.log_level,
            },
            server: symphony_config::ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: self.server_port,
            },
            observability: symphony_config::ObservabilityConfig::default(),
            version: 0,
        }
    }
}

/// Create a new RuntimeConfig builder with test defaults.
pub fn runtime_config() -> RuntimeConfigBuilder {
    RuntimeConfigBuilder::default()
}

/// Builder for test workflow markdown content.
///
/// # Example
/// ```ignore
/// use symphony_testkit::config_builders::workflow_content;
///
/// let markdown = workflow_content()
///     .with_project_slug("MYPROJ")
///     .with_prompt("Fix all the bugs")
///     .build();
///
/// assert!(markdown.contains("project_slug: MYPROJ"));
/// assert!(markdown.contains("Fix all the bugs"));
/// ```
pub struct WorkflowContentBuilder {
    tracker_kind: String,
    tracker_endpoint: String,
    tracker_api_token: String,
    tracker_project_slug: String,
    tracker_active_states: Vec<String>,
    tracker_terminal_states: Vec<String>,
    poll_interval_ms: u64,
    workspace_root: String,
    max_concurrent_agents: u32,
    max_turns: u32,
    prompt: String,
}

impl Default for WorkflowContentBuilder {
    fn default() -> Self {
        Self {
            tracker_kind: "linear".to_owned(),
            tracker_endpoint: "https://api.linear.app/graphql".to_owned(),
            tracker_api_token: "test-token".to_owned(),
            tracker_project_slug: "SYM".to_owned(),
            tracker_active_states: vec!["Todo".to_owned(), "In Progress".to_owned()],
            tracker_terminal_states: vec![
                "Closed".to_owned(),
                "Cancelled".to_owned(),
                "Canceled".to_owned(),
                "Duplicate".to_owned(),
                "Done".to_owned(),
            ],
            poll_interval_ms: 30_000,
            workspace_root: "/tmp/symphony_workspaces".to_owned(),
            max_concurrent_agents: 10,
            max_turns: 20,
            prompt: "You are an agent for this repository.".to_owned(),
        }
    }
}

impl WorkflowContentBuilder {
    /// Set the project slug.
    pub fn with_project_slug(mut self, slug: impl Into<String>) -> Self {
        self.tracker_project_slug = slug.into();
        self
    }

    /// Set the API token.
    pub fn with_api_token(mut self, token: impl Into<String>) -> Self {
        self.tracker_api_token = token.into();
        self
    }

    /// Set the prompt body.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Set the poll interval.
    pub fn with_poll_interval_ms(mut self, ms: u64) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    /// Set the maximum concurrent agents.
    pub fn with_max_concurrent(mut self, max: u32) -> Self {
        self.max_concurrent_agents = max;
        self
    }

    /// Set the maximum turns per session.
    pub fn with_max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set active states.
    pub fn with_active_states(mut self, states: Vec<&str>) -> Self {
        self.tracker_active_states = states.into_iter().map(|s| s.to_owned()).collect();
        self
    }

    /// Set terminal states.
    pub fn with_terminal_states(mut self, states: Vec<&str>) -> Self {
        self.tracker_terminal_states = states.into_iter().map(|s| s.to_owned()).collect();
        self
    }

    /// Build the workflow markdown content.
    pub fn build(self) -> String {
        let active_states_yaml = self
            .tracker_active_states
            .iter()
            .map(|s| format!("      - {}", s))
            .collect::<Vec<_>>()
            .join("\n");
        let terminal_states_yaml = self
            .tracker_terminal_states
            .iter()
            .map(|s| format!("      - {}", s))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"---
tracker:
  kind: {}
  endpoint: {}
  api_key: {}
  project_slug: {}
  active_states:
{}
  terminal_states:
{}
polling:
  interval_ms: {}
workspace:
  root: {}
agent:
  max_concurrent_agents: {}
  max_turns: {}
---

{}"#,
            self.tracker_kind,
            self.tracker_endpoint,
            self.tracker_api_token,
            self.tracker_project_slug,
            active_states_yaml,
            terminal_states_yaml,
            self.poll_interval_ms,
            self.workspace_root,
            self.max_concurrent_agents,
            self.max_turns,
            self.prompt
        )
    }
}

/// Create a new workflow content builder with test defaults.
pub fn workflow_content() -> WorkflowContentBuilder {
    WorkflowContentBuilder::default()
}

/// Create a minimal valid workflow markdown string with defaults.
pub fn minimal_workflow() -> String {
    workflow_content().build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_builder_creates_valid_config() {
        let config = runtime_config()
            .with_project_slug("TESTPROJ")
            .with_api_key("secret-key")
            .with_max_concurrent(5)
            .with_poll_interval_ms(5000)
            .build();

        assert_eq!(config.tracker.project_slug, Some("TESTPROJ".to_owned()));
        assert_eq!(config.tracker.api_key, Some("secret-key".to_owned()));
        assert_eq!(config.agent.max_concurrent_agents, 5);
        assert_eq!(config.polling.interval_ms, 5000);
    }

    #[test]
    fn workflow_content_builder_creates_valid_markdown() {
        let markdown = workflow_content()
            .with_project_slug("MYPROJ")
            .with_prompt("Fix all the issues")
            .build();

        assert!(markdown.contains("---"));
        assert!(markdown.contains("project_slug: MYPROJ"));
        assert!(markdown.contains("Fix all the issues"));
        assert!(markdown.contains("active_states:"));
        assert!(markdown.contains("terminal_states:"));
    }

    #[test]
    fn minimal_workflow_is_valid() {
        let markdown = minimal_workflow();
        assert!(markdown.starts_with("---"));
        assert!(markdown.contains("project_slug: SYM"));
    }
}
