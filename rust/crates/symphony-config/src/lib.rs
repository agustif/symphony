#![forbid(unsafe_code)]

mod env;
mod error;
mod loader;
mod model;
mod normalize;
mod validate;

pub use env::{EnvProvider, ProcessEnv};
pub use error::ConfigError;
pub use loader::{apply_cli_overrides, from_front_matter, from_front_matter_with_env};
pub use model::{
    AgentConfig, CliOverrides, CodexConfig, HooksConfig, LogLevelConfig, PollingConfig,
    RuntimeConfig, TrackerConfig, WorkspaceConfig,
};
pub use validate::validate;
