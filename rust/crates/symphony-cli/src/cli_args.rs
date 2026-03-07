use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
#[command(name = "symphony", about = "Symphony runtime CLI")]
pub struct CliArgs {
    #[arg(value_name = "WORKFLOW_PATH")]
    pub workflow: Option<PathBuf>,

    #[arg(
        long,
        short = 'c',
        value_name = "PATH",
        default_value = "symphony.runtime.json"
    )]
    pub config: PathBuf,

    /// Override polling interval in milliseconds
    #[arg(long, value_name = "MILLISECONDS")]
    pub polling_interval_ms: Option<u64>,

    /// Override max concurrent agents
    #[arg(long, value_name = "N")]
    pub max_concurrent_agents: Option<u32>,

    /// Override max turns per agent
    #[arg(long, value_name = "N")]
    pub max_turns: Option<u32>,

    /// Override max retry backoff in milliseconds
    #[arg(long, value_name = "MILLISECONDS")]
    pub max_retry_backoff_ms: Option<u64>,

    /// Override workspace root path
    #[arg(long, value_name = "PATH")]
    pub workspace_root: Option<PathBuf>,

    /// Override log level (trace, debug, info, warn, error)
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// Override tracker API endpoint
    #[arg(long, value_name = "URL")]
    pub tracker_endpoint: Option<String>,

    /// Override tracker API key
    #[arg(long, value_name = "KEY")]
    pub tracker_api_key: Option<String>,

    /// Override tracker project slug
    #[arg(long, value_name = "SLUG")]
    pub tracker_project_slug: Option<String>,

    /// Bind optional HTTP server to this address
    #[arg(long, value_name = "IP_ADDR")]
    pub bind: Option<IpAddr>,

    /// Enable optional HTTP server on this port (`0` requests an ephemeral local port)
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Optional directory for host log file output
    #[arg(long, value_name = "PATH")]
    pub logs_root: Option<PathBuf>,
}

impl CliArgs {
    /// Convert to CliOverrides for config application
    pub fn to_cli_overrides(&self) -> symphony_config::CliOverrides {
        symphony_config::CliOverrides {
            polling_interval_ms: self.polling_interval_ms,
            max_concurrent_agents: self.max_concurrent_agents,
            max_turns: self.max_turns,
            max_retry_backoff_ms: self.max_retry_backoff_ms,
            workspace_root: self.workspace_root.clone(),
            log_level: self.log_level.clone(),
            tracker_endpoint: self.tracker_endpoint.clone(),
            tracker_api_key: self.tracker_api_key.clone(),
            tracker_project_slug: self.tracker_project_slug.clone(),
            server_host: self.bind,
            server_port: self.port,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_cli_overrides_with_all_fields() {
        let args = CliArgs {
            workflow: Some(PathBuf::from("WORKFLOW.md")),
            config: PathBuf::from("symphony.runtime.json"),
            polling_interval_ms: Some(60000),
            max_concurrent_agents: Some(20),
            max_turns: Some(30),
            max_retry_backoff_ms: Some(600000),
            workspace_root: Some(PathBuf::from("/tmp/workspaces")),
            log_level: Some("debug".to_string()),
            tracker_endpoint: Some("https://custom.endpoint.com".to_string()),
            tracker_api_key: Some("api-key".to_string()),
            tracker_project_slug: Some("my-project".to_string()),
            bind: Some("0.0.0.0".parse().expect("bind address should parse")),
            port: Some(8080),
            logs_root: Some(PathBuf::from("/tmp/logs")),
        };

        let overrides = args.to_cli_overrides();
        assert_eq!(overrides.polling_interval_ms, Some(60000));
        assert_eq!(overrides.max_concurrent_agents, Some(20));
        assert_eq!(overrides.max_turns, Some(30));
        assert_eq!(overrides.max_retry_backoff_ms, Some(600000));
        assert_eq!(
            overrides.workspace_root,
            Some(PathBuf::from("/tmp/workspaces"))
        );
        assert_eq!(overrides.log_level, Some("debug".to_string()));
        assert_eq!(
            overrides.tracker_endpoint,
            Some("https://custom.endpoint.com".to_string())
        );
        assert_eq!(overrides.tracker_api_key, Some("api-key".to_string()));
        assert_eq!(
            overrides.tracker_project_slug,
            Some("my-project".to_string())
        );
        assert_eq!(
            overrides.server_host,
            Some("0.0.0.0".parse().expect("bind address should parse"))
        );
        assert_eq!(overrides.server_port, Some(8080));
    }

    #[test]
    fn converts_to_cli_overrides_with_no_overrides() {
        let args = CliArgs {
            workflow: None,
            config: PathBuf::from("symphony.runtime.json"),
            polling_interval_ms: None,
            max_concurrent_agents: None,
            max_turns: None,
            max_retry_backoff_ms: None,
            workspace_root: None,
            log_level: None,
            tracker_endpoint: None,
            tracker_api_key: None,
            tracker_project_slug: None,
            bind: None,
            port: None,
            logs_root: None,
        };

        let overrides = args.to_cli_overrides();
        assert!(overrides.is_empty());
    }
}
