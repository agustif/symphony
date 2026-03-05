use std::collections::HashSet;

use crate::{ConfigError, RuntimeConfig, normalize::normalize_state_name};

pub fn validate(config: &RuntimeConfig) -> Result<(), ConfigError> {
    if config.polling.interval_ms == 0 {
        return Err(ConfigError::InvalidPollInterval);
    }

    if config.hooks.timeout_ms == 0 {
        return Err(ConfigError::InvalidHookTimeout);
    }

    if config.agent.max_concurrent_agents == 0 {
        return Err(ConfigError::InvalidMaxConcurrentAgents);
    }

    if config.agent.max_turns == 0 {
        return Err(ConfigError::InvalidMaxTurns);
    }

    if config.agent.max_retry_backoff_ms == 0 {
        return Err(ConfigError::InvalidMaxRetryBackoff);
    }

    if config.codex.turn_timeout_ms == 0 {
        return Err(ConfigError::InvalidCodexTurnTimeout);
    }

    if config.codex.read_timeout_ms == 0 {
        return Err(ConfigError::InvalidCodexReadTimeout);
    }

    if config.codex.stall_timeout_ms <= 0 {
        return Err(ConfigError::InvalidCodexStallTimeout);
    }

    if config.workspace.root.as_os_str().is_empty() {
        return Err(ConfigError::InvalidWorkspaceRoot);
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

    let normalized_active_states = config
        .tracker
        .active_states
        .iter()
        .filter_map(|state| normalize_state_name(state))
        .collect::<HashSet<_>>();
    if normalized_active_states.is_empty() {
        return Err(ConfigError::MissingTrackerActiveStates);
    }

    for (state, limit) in &config.agent.max_concurrent_agents_by_state {
        let Some(normalized_state) = normalize_state_name(state) else {
            return Err(ConfigError::InvalidStateLimitKey);
        };

        if !normalized_active_states.contains(&normalized_state) {
            return Err(ConfigError::UnknownStateLimitState(normalized_state));
        }

        if *limit > config.agent.max_concurrent_agents {
            return Err(ConfigError::StateLimitExceedsMaxConcurrent {
                state: normalized_state,
                limit: *limit,
                max: config.agent.max_concurrent_agents,
            });
        }
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
    fn rejects_empty_active_states() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.tracker.active_states = vec![];

        assert_eq!(
            validate(&config),
            Err(ConfigError::MissingTrackerActiveStates)
        );
    }

    #[test]
    fn rejects_out_of_range_state_limit() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.tracker.active_states = vec!["todo".to_owned()];
        config.agent.max_concurrent_agents = 2;
        config
            .agent
            .max_concurrent_agents_by_state
            .insert("todo".to_owned(), 3);

        assert_eq!(
            validate(&config),
            Err(ConfigError::StateLimitExceedsMaxConcurrent {
                state: "todo".to_owned(),
                limit: 3,
                max: 2,
            })
        );
    }

    #[test]
    fn rejects_unknown_state_limit_key() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.tracker.active_states = vec!["todo".to_owned()];
        config
            .agent
            .max_concurrent_agents_by_state
            .insert("blocked".to_owned(), 1);

        assert_eq!(
            validate(&config),
            Err(ConfigError::UnknownStateLimitState("blocked".to_owned()))
        );
    }

    #[test]
    fn rejects_invalid_codex_ranges() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        config.codex.stall_timeout_ms = 0;

        assert_eq!(
            validate(&config),
            Err(ConfigError::InvalidCodexStallTimeout)
        );
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
