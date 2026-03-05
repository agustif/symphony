use crate::{ConfigError, RuntimeConfig};

pub fn validate(config: &RuntimeConfig) -> Result<(), ConfigError> {
    if config.polling.interval_ms == 0 {
        return Err(ConfigError::InvalidPollInterval);
    }

    let tracker_kind = config.tracker.kind.trim();
    if tracker_kind.is_empty() {
        return Err(ConfigError::MissingTrackerKind);
    }

    if !tracker_kind.eq_ignore_ascii_case("linear") {
        return Err(ConfigError::UnsupportedTrackerKind(
            config.tracker.kind.clone(),
        ));
    }

    let has_api_key = config
        .tracker
        .api_key
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if !has_api_key {
        return Err(ConfigError::MissingTrackerApiKey);
    }

    let has_project_slug = config
        .tracker
        .project_slug
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if !has_project_slug {
        return Err(ConfigError::MissingTrackerProjectSlug);
    }

    if config.codex.command.trim().is_empty() {
        return Err(ConfigError::MissingCodexCommand);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{ConfigError, RuntimeConfig, validate};

    #[test]
    fn rejects_zero_poll_interval() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.polling.interval_ms = 0;

        assert_eq!(validate(&config), Err(ConfigError::InvalidPollInterval));
    }

    #[test]
    fn rejects_missing_api_key() {
        let mut config = RuntimeConfig::default();
        config.tracker.project_slug = Some("symphony".to_owned());

        assert_eq!(validate(&config), Err(ConfigError::MissingTrackerApiKey));
    }

    #[test]
    fn rejects_empty_codex_command() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.codex.command = "  ".to_owned();

        assert_eq!(validate(&config), Err(ConfigError::MissingCodexCommand));
    }
}
