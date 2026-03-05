#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub kind: String,
    pub project_slug: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub tracker: TrackerConfig,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("poll_interval_ms must be > 0")]
    InvalidPollInterval,
}

pub fn validate(config: &RuntimeConfig) -> Result<(), ConfigError> {
    if config.poll_interval_ms == 0 {
        return Err(ConfigError::InvalidPollInterval);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_poll_interval() {
        let cfg = RuntimeConfig {
            tracker: TrackerConfig {
                kind: "linear".to_owned(),
                project_slug: "symphony".to_owned(),
            },
            poll_interval_ms: 0,
        };

        assert_eq!(validate(&cfg), Err(ConfigError::InvalidPollInterval));
    }
}
