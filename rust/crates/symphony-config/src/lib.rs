#![forbid(unsafe_code)]

mod env;
mod error;
mod loader;
mod model;
mod normalize;
mod validate;

pub use env::{EnvProvider, ProcessEnv};
pub use error::ConfigError;
pub use loader::{
    apply_cli_overrides, extract_front_matter, from_figment, from_front_matter,
    from_front_matter_with_env, parse_json_with_path, parse_yaml_with_path,
};
pub use model::{
    AgentConfig, CliOverrides, CodexConfig, HooksConfig, LogLevelConfig, PollingConfig,
    RuntimeConfig, ServerConfig, TrackerConfig, WorkspaceConfig,
};
pub use validate::validate;
