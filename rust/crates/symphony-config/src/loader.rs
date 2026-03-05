use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use serde_yaml::{Mapping, Value};

use crate::{
    ConfigError, EnvProvider, ProcessEnv, RuntimeConfig,
    env::{expand_workspace_root, resolve_env_reference},
    model::{
        AgentConfig, CliOverrides, CodexConfig, DEFAULT_HOOK_TIMEOUT_MS, DEFAULT_LINEAR_ENDPOINT,
        HooksConfig, LogLevelConfig, PollingConfig, TrackerConfig, WorkspaceConfig,
    },
    normalize::{normalize_state_list, normalize_state_name},
    validate,
};

pub fn from_front_matter(front_matter: &Mapping) -> Result<RuntimeConfig, ConfigError> {
    from_front_matter_with_env(front_matter, &ProcessEnv)
}

pub fn from_front_matter_with_env(
    front_matter: &Mapping,
    env: &dyn EnvProvider,
) -> Result<RuntimeConfig, ConfigError> {
    let mut config = RuntimeConfig::default();

    if let Some(section) = section(front_matter, "tracker")? {
        config.tracker = parse_tracker_config(section, config.tracker, env)?;
    }

    if let Some(section) = section(front_matter, "polling")? {
        config.polling = parse_polling_config(section, config.polling)?;
    }

    if let Some(section) = section(front_matter, "workspace")? {
        config.workspace = parse_workspace_config(section, config.workspace, env)?;
    }

    if let Some(section) = section(front_matter, "hooks")? {
        config.hooks = parse_hooks_config(section, config.hooks)?;
    }

    if let Some(section) = section(front_matter, "agent")? {
        config.agent = parse_agent_config(section, config.agent)?;
    }

    if let Some(section) = section(front_matter, "codex")? {
        config.codex = parse_codex_config(section, config.codex)?;
    }

    apply_tracker_defaults(&mut config.tracker, env);
    validate(&config)?;
    Ok(config)
}

fn parse_tracker_config(
    section: &Mapping,
    mut tracker: TrackerConfig,
    env: &dyn EnvProvider,
) -> Result<TrackerConfig, ConfigError> {
    if let Some(kind) = optional_trimmed_string(section, "kind", "tracker.kind")? {
        tracker.kind = kind.to_ascii_lowercase();
    }

    if let Some(endpoint) = optional_trimmed_string(section, "endpoint", "tracker.endpoint")? {
        tracker.endpoint = endpoint;
    }

    if let Some(api_key) = optional_string(section, "api_key", "tracker.api_key")? {
        tracker.api_key = resolve_env_reference(&api_key, env);
    }

    if let Some(project_slug) =
        optional_trimmed_string(section, "project_slug", "tracker.project_slug")?
    {
        tracker.project_slug = Some(project_slug);
    }

    if let Some(active_states) =
        optional_string_list(section, "active_states", "tracker.active_states")?
    {
        tracker.active_states = normalize_state_list(active_states);
    }

    if let Some(terminal_states) =
        optional_string_list(section, "terminal_states", "tracker.terminal_states")?
    {
        tracker.terminal_states = normalize_state_list(terminal_states);
    }

    Ok(tracker)
}

fn parse_polling_config(
    section: &Mapping,
    mut polling: PollingConfig,
) -> Result<PollingConfig, ConfigError> {
    if let Some(interval_ms) = optional_positive_u64(section, "interval_ms", "polling.interval_ms")?
    {
        polling.interval_ms = interval_ms;
    }

    Ok(polling)
}

fn parse_workspace_config(
    section: &Mapping,
    mut workspace: WorkspaceConfig,
    env: &dyn EnvProvider,
) -> Result<WorkspaceConfig, ConfigError> {
    if let Some(root) = optional_string(section, "root", "workspace.root")?
        && let Some(expanded) = expand_workspace_root(&root, env)
    {
        workspace.root = normalize_workspace_root(expanded);
    }

    Ok(workspace)
}

fn parse_hooks_config(
    section: &Mapping,
    mut hooks: HooksConfig,
) -> Result<HooksConfig, ConfigError> {
    hooks.after_create = optional_string(section, "after_create", "hooks.after_create")?;
    hooks.before_run = optional_string(section, "before_run", "hooks.before_run")?;
    hooks.after_run = optional_string(section, "after_run", "hooks.after_run")?;
    hooks.before_remove = optional_string(section, "before_remove", "hooks.before_remove")?;

    if let Some(timeout_ms) = optional_i64(section, "timeout_ms", "hooks.timeout_ms")? {
        hooks.timeout_ms = if timeout_ms > 0 {
            timeout_ms as u64
        } else {
            DEFAULT_HOOK_TIMEOUT_MS
        };
    }

    Ok(hooks)
}

fn parse_agent_config(
    section: &Mapping,
    mut agent: AgentConfig,
) -> Result<AgentConfig, ConfigError> {
    if let Some(max_concurrent_agents) = optional_positive_u32(
        section,
        "max_concurrent_agents",
        "agent.max_concurrent_agents",
    )? {
        agent.max_concurrent_agents = max_concurrent_agents;
    }

    if let Some(max_turns) = optional_positive_u32(section, "max_turns", "agent.max_turns")? {
        agent.max_turns = max_turns;
    }

    if let Some(max_retry_backoff_ms) = optional_positive_u64(
        section,
        "max_retry_backoff_ms",
        "agent.max_retry_backoff_ms",
    )? {
        agent.max_retry_backoff_ms = max_retry_backoff_ms;
    }

    if let Some(Value::Mapping(state_limits)) =
        section.get(yaml_key("max_concurrent_agents_by_state"))
    {
        agent.max_concurrent_agents_by_state = parse_state_limits(state_limits)?;
    } else if section
        .get(yaml_key("max_concurrent_agents_by_state"))
        .is_some()
    {
        return Err(ConfigError::InvalidType {
            field: "agent.max_concurrent_agents_by_state",
            expected: "map",
        });
    }

    Ok(agent)
}

fn parse_codex_config(
    section: &Mapping,
    mut codex: CodexConfig,
) -> Result<CodexConfig, ConfigError> {
    if let Some(command) = optional_string_allow_empty(section, "command", "codex.command")? {
        codex.command = command;
    }

    codex.approval_policy =
        optional_trimmed_string(section, "approval_policy", "codex.approval_policy")?;
    codex.thread_sandbox =
        optional_trimmed_string(section, "thread_sandbox", "codex.thread_sandbox")?;
    codex.turn_sandbox_policy =
        optional_trimmed_string(section, "turn_sandbox_policy", "codex.turn_sandbox_policy")?;

    if let Some(turn_timeout_ms) =
        optional_positive_u64(section, "turn_timeout_ms", "codex.turn_timeout_ms")?
    {
        codex.turn_timeout_ms = turn_timeout_ms;
    }

    if let Some(read_timeout_ms) =
        optional_positive_u64(section, "read_timeout_ms", "codex.read_timeout_ms")?
    {
        codex.read_timeout_ms = read_timeout_ms;
    }

    if let Some(stall_timeout_ms) =
        optional_i64(section, "stall_timeout_ms", "codex.stall_timeout_ms")?
    {
        codex.stall_timeout_ms = stall_timeout_ms;
    }

    Ok(codex)
}

fn apply_tracker_defaults(tracker: &mut TrackerConfig, env: &dyn EnvProvider) {
    tracker.kind = tracker.kind.trim().to_ascii_lowercase();
    tracker.active_states = normalize_state_list(std::mem::take(&mut tracker.active_states));
    tracker.terminal_states = normalize_state_list(std::mem::take(&mut tracker.terminal_states));

    if tracker.kind.eq_ignore_ascii_case("linear") {
        if tracker.endpoint.trim().is_empty() {
            tracker.endpoint = DEFAULT_LINEAR_ENDPOINT.to_owned();
        }

        if tracker.api_key.is_none() {
            tracker.api_key = env.get("LINEAR_API_KEY");
        }
    }
}

fn parse_state_limits(raw_limits: &Mapping) -> Result<HashMap<String, u32>, ConfigError> {
    let mut parsed = HashMap::new();

    for (state, limit) in raw_limits {
        let Some(state) = state.as_str() else {
            return Err(ConfigError::InvalidStateLimitKey);
        };

        let Some(normalized_state) = normalize_state_name(state) else {
            return Err(ConfigError::InvalidStateLimitKey);
        };

        let parsed_limit = parse_state_limit_value(&normalized_state, limit)?;
        parsed.insert(normalized_state, parsed_limit);
    }

    Ok(parsed)
}

fn parse_state_limit_value(state: &str, value: &Value) -> Result<u32, ConfigError> {
    let parsed = match value {
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                u32::try_from(value).ok()
            } else {
                number.as_i64().and_then(|value| u32::try_from(value).ok())
            }
        }
        Value::String(raw) => raw
            .trim()
            .parse::<u64>()
            .ok()
            .and_then(|value| u32::try_from(value).ok()),
        _ => None,
    };

    parsed
        .filter(|value| *value > 0)
        .ok_or_else(|| ConfigError::InvalidStateLimitValue {
            state: state.to_owned(),
            value: value_to_string(value),
        })
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(raw) => raw.to_owned(),
        _ => serde_yaml::to_string(value)
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|_| "<non-string-yaml-value>".to_owned()),
    }
}

fn normalize_workspace_root(root: PathBuf) -> PathBuf {
    let absolute = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()
            .map(|current_dir| current_dir.join(root))
            .unwrap_or_else(|_| PathBuf::from("."))
    };

    normalize_path(&absolute)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let has_root = path.has_root();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

fn section<'a>(root: &'a Mapping, name: &'static str) -> Result<Option<&'a Mapping>, ConfigError> {
    match root.get(yaml_key(name)) {
        None => Ok(None),
        Some(Value::Mapping(section)) => Ok(Some(section)),
        Some(_) => Err(ConfigError::InvalidType {
            field: name,
            expected: "map",
        }),
    }
}

fn optional_trimmed_string(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<String>, ConfigError> {
    Ok(optional_string(map, key, field)?
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty()))
}

fn optional_string(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<String>, ConfigError> {
    match map.get(yaml_key(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.to_owned())),
        Some(_) => Err(ConfigError::InvalidType {
            field,
            expected: "string",
        }),
    }
}

fn optional_string_allow_empty(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<String>, ConfigError> {
    match map.get(yaml_key(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.to_owned())),
        Some(_) => Err(ConfigError::InvalidType {
            field,
            expected: "string",
        }),
    }
}

fn optional_string_list(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<Vec<String>>, ConfigError> {
    let Some(value) = map.get(yaml_key(key)) else {
        return Ok(None);
    };

    match value {
        Value::String(value) => {
            let parsed = value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            Ok(Some(parsed))
        }
        Value::Sequence(values) => {
            let mut parsed = Vec::with_capacity(values.len());
            for value in values {
                let Some(entry) = value.as_str() else {
                    return Err(ConfigError::InvalidType {
                        field,
                        expected: "list of strings",
                    });
                };

                let entry = entry.trim();
                if !entry.is_empty() {
                    parsed.push(entry.to_owned());
                }
            }
            Ok(Some(parsed))
        }
        Value::Null => Ok(None),
        _ => Err(ConfigError::InvalidType {
            field,
            expected: "string or list of strings",
        }),
    }
}

fn optional_positive_u64(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<u64>, ConfigError> {
    let Some(value) = map.get(yaml_key(key)) else {
        return Ok(None);
    };

    let parsed = parse_i64(value, field)?;
    if parsed <= 0 {
        return Err(ConfigError::InvalidInteger {
            field,
            value: parsed.to_string(),
        });
    }

    Ok(Some(parsed as u64))
}

fn optional_positive_u32(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<u32>, ConfigError> {
    let Some(value) = optional_positive_u64(map, key, field)? else {
        return Ok(None);
    };

    let value = u32::try_from(value).map_err(|_| ConfigError::InvalidInteger {
        field,
        value: value.to_string(),
    })?;

    Ok(Some(value))
}

fn optional_i64(
    map: &Mapping,
    key: &'static str,
    field: &'static str,
) -> Result<Option<i64>, ConfigError> {
    let Some(value) = map.get(yaml_key(key)) else {
        return Ok(None);
    };

    Ok(Some(parse_i64(value, field)?))
}

fn parse_i64(value: &Value, field: &'static str) -> Result<i64, ConfigError> {
    match value {
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                return Ok(integer);
            }

            if let Some(unsigned) = number.as_u64()
                && let Ok(integer) = i64::try_from(unsigned)
            {
                return Ok(integer);
            }

            Err(ConfigError::InvalidInteger {
                field,
                value: number.to_string(),
            })
        }
        Value::String(raw) => raw
            .trim()
            .parse::<i64>()
            .map_err(|_| ConfigError::InvalidInteger {
                field,
                value: raw.to_owned(),
            }),
        _ => Err(ConfigError::InvalidType {
            field,
            expected: "integer or string integer",
        }),
    }
}

fn yaml_key(key: &'static str) -> Value {
    Value::String(key.to_owned())
}

#[cfg(test)]
fn parse_yaml(yaml: &str) -> Mapping {
    let value: Value = serde_yaml::from_str(yaml).expect("yaml should parse");
    value
        .as_mapping()
        .cloned()
        .expect("root should be a mapping")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };

    use crate::{
        ConfigError, EnvProvider,
        loader::{from_front_matter_with_env, parse_yaml, yaml_key},
        model::{DEFAULT_HOOK_TIMEOUT_MS, DEFAULT_LINEAR_ENDPOINT},
    };
    use serde_yaml::Value;

    struct TestEnv {
        values: HashMap<String, String>,
    }

    impl TestEnv {
        fn from_entries(entries: &[(&str, &str)]) -> Self {
            let values = entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect();
            Self { values }
        }
    }

    impl EnvProvider for TestEnv {
        fn get(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }
    }

    #[test]
    fn resolves_env_indirection_numeric_coercions_and_state_normalization() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: ${TRACKER_TOKEN}
  project_slug: symphony
  active_states:
    - " Todo "
    - "In   Progress"
polling:
  interval_ms: "45000"
workspace:
  root: $WORKSPACE_ROOT
hooks:
  timeout_ms: -100
agent:
  max_concurrent_agents: "12"
  max_concurrent_agents_by_state:
    In Progress: 3
    todo: "2"
codex:
  command: codex app-server --json
"#,
        );
        let env = TestEnv::from_entries(&[
            ("TRACKER_TOKEN", "secret-token"),
            ("WORKSPACE_ROOT", "/tmp/custom-workspaces"),
        ]);

        let config = from_front_matter_with_env(&front_matter, &env).expect("config should parse");

        assert_eq!(config.tracker.api_key.as_deref(), Some("secret-token"));
        assert_eq!(config.tracker.project_slug.as_deref(), Some("symphony"));
        assert_eq!(config.tracker.endpoint, DEFAULT_LINEAR_ENDPOINT);
        assert_eq!(
            config.tracker.active_states,
            vec!["todo".to_owned(), "in progress".to_owned()]
        );
        assert_eq!(config.polling.interval_ms, 45_000);
        assert_eq!(
            config.workspace.root,
            PathBuf::from("/tmp/custom-workspaces")
        );
        assert_eq!(config.hooks.timeout_ms, DEFAULT_HOOK_TIMEOUT_MS);
        assert_eq!(config.agent.max_concurrent_agents, 12);
        assert_eq!(
            config
                .agent
                .max_concurrent_agents_by_state
                .get("in progress"),
            Some(&3)
        );
        assert_eq!(
            config.agent.max_concurrent_agents_by_state.get("todo"),
            Some(&2)
        );
    }

    #[test]
    fn supports_csv_and_sequence_states_with_normalization() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
  active_states: Todo, In Progress,  todo
  terminal_states:
    - Closed
    - " In   Progress "
"#,
        );

        let config = from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[]))
            .expect("config should parse");

        assert_eq!(
            config.tracker.active_states,
            vec!["todo".to_owned(), "in progress".to_owned()]
        );
        assert_eq!(
            config.tracker.terminal_states,
            vec!["closed".to_owned(), "in progress".to_owned()]
        );
    }

    #[test]
    fn normalizes_relative_workspace_paths() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
workspace:
  root: ./tmp/workspaces/../issues
"#,
        );

        let config = from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[]))
            .expect("config should parse");

        assert!(config.workspace.root.is_absolute());
        assert!(config.workspace.root.ends_with(Path::new("tmp/issues")));
    }

    #[test]
    fn uses_canonical_linear_api_key_env_when_missing_inline_value() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  project_slug: symphony
"#,
        );
        let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "linear-key")]);

        let config = from_front_matter_with_env(&front_matter, &env).expect("config should parse");
        assert_eq!(config.tracker.api_key.as_deref(), Some("linear-key"));
    }

    #[test]
    fn rejects_unsupported_tracker_kind() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: github
  api_key: token
  project_slug: symphony
"#,
        );

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::UnsupportedTrackerKind("github".to_owned()))
        );
    }

    #[test]
    fn rejects_invalid_interval_value() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
polling:
  interval_ms: nope
"#,
        );

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::InvalidInteger {
                field: "polling.interval_ms",
                value: "nope".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_invalid_state_limit_values() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
agent:
  max_concurrent_agents_by_state:
    Todo: invalid
"#,
        );

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::InvalidStateLimitValue {
                state: "todo".to_owned(),
                value: "invalid".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_unknown_state_limit_key() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
  active_states:
    - Todo
agent:
  max_concurrent_agents_by_state:
    Blocked: 2
"#,
        );

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::UnknownStateLimitState("blocked".to_owned()))
        );
    }

    #[test]
    fn rejects_empty_codex_command() {
        let mut front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
"#,
        );

        // Manually set codex command to empty after parsing to test validation
        if let Some(Value::Mapping(codex)) = front_matter.get_mut(yaml_key("codex")) {
            codex.insert(yaml_key("command"), Value::String("".to_string()));
        } else {
            front_matter.insert(yaml_key("codex"), {
                let mut codex = serde_yaml::Mapping::new();
                codex.insert(yaml_key("command"), Value::String("".to_string()));
                Value::Mapping(codex)
            });
        }

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::MissingCodexCommand)
        );
    }
}

/// Apply CLI overrides to a RuntimeConfig.
///
/// This function applies CLI-specified overrides to a configuration, taking
/// precedence over config file and environment variable values.
///
/// # Precedence Order
///
/// 1. CLI arguments (applied by this function) - highest priority
/// 2. Environment variables ($VAR or ${VAR}) - already resolved in config
/// 3. Config file values - loaded from YAML
/// 4. Default values - from impl Default
///
/// # Validation
///
/// All override values are validated before being applied. Invalid values
/// will result in a `ConfigError::InvalidCliOverride`.
///
/// # Example
///
/// ```ignore
/// use symphony_config::{from_front_matter, CliOverrides, apply_cli_overrides};
///
/// // Load config from YAML front matter
/// let config = from_front_matter(&yaml_mapping)?;
///
/// // Create CLI overrides
/// let overrides = CliOverrides {
///     polling_interval_ms: Some(60000),
///     ..Default::default()
/// };
///
/// // Apply overrides (result is validated)
/// let config = apply_cli_overrides(config, &overrides)?;
/// ```
///
/// # Supported Override Fields
///
/// - `polling_interval_ms`: Must be > 0
/// - `max_concurrent_agents`: Must be > 0
/// - `max_turns`: Must be > 0
/// - `max_retry_backoff_ms`: Must be > 0
/// - `workspace_root`: Must be a valid path
/// - `log_level`: Must be one of: trace, debug, info, warn, error
/// - `tracker_endpoint`: Must not be empty
/// - `tracker_api_key`: Optional string
/// - `tracker_project_slug`: Must not be empty if provided
pub fn apply_cli_overrides(
    mut config: RuntimeConfig,
    overrides: &CliOverrides,
) -> Result<RuntimeConfig, ConfigError> {
    if let Some(interval_ms) = overrides.polling_interval_ms {
        if interval_ms == 0 {
            return Err(ConfigError::InvalidCliOverride {
                field: "polling.interval_ms".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        config.polling.interval_ms = interval_ms;
    }

    if let Some(max_concurrent) = overrides.max_concurrent_agents {
        if max_concurrent == 0 {
            return Err(ConfigError::InvalidCliOverride {
                field: "agent.max_concurrent_agents".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        config.agent.max_concurrent_agents = max_concurrent;
    }

    if let Some(max_turns) = overrides.max_turns {
        if max_turns == 0 {
            return Err(ConfigError::InvalidCliOverride {
                field: "agent.max_turns".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        config.agent.max_turns = max_turns;
    }

    if let Some(max_retry_backoff) = overrides.max_retry_backoff_ms {
        if max_retry_backoff == 0 {
            return Err(ConfigError::InvalidCliOverride {
                field: "agent.max_retry_backoff_ms".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }
        config.agent.max_retry_backoff_ms = max_retry_backoff;
    }

    if let Some(ref root) = overrides.workspace_root {
        let expanded =
            expand_workspace_root(root.to_str().unwrap_or(""), &ProcessEnv).ok_or_else(|| {
                ConfigError::InvalidCliOverride {
                    field: "workspace.root".to_string(),
                    reason: "failed to expand path".to_string(),
                }
            })?;
        config.workspace.root = normalize_workspace_root(expanded);
    }

    if let Some(ref log_level) = overrides.log_level {
        let level = log_level.trim().to_lowercase();
        if !matches!(
            level.as_str(),
            "trace" | "debug" | "info" | "warn" | "error"
        ) {
            return Err(ConfigError::InvalidCliOverride {
                field: "log.level".to_string(),
                reason: format!(
                    "must be one of: trace, debug, info, warn, error, got: {level}",
                    level = level
                ),
            });
        }
        config.log_level = LogLevelConfig { level };
    }

    if let Some(ref endpoint) = overrides.tracker_endpoint {
        if endpoint.trim().is_empty() {
            return Err(ConfigError::InvalidCliOverride {
                field: "tracker.endpoint".to_string(),
                reason: "must not be empty".to_string(),
            });
        }
        config.tracker.endpoint = endpoint.clone();
    }

    if let Some(ref api_key) = overrides.tracker_api_key {
        config.tracker.api_key = Some(api_key.clone());
    }

    if let Some(ref project_slug) = overrides.tracker_project_slug {
        if project_slug.trim().is_empty() {
            return Err(ConfigError::InvalidCliOverride {
                field: "tracker.project_slug".to_string(),
                reason: "must not be empty".to_string(),
            });
        }
        config.tracker.project_slug = Some(project_slug.clone());
    }

    validate(&config)?;
    Ok(config)
}

#[cfg(test)]
mod cli_override_tests {
    use super::*;

    #[test]
    fn applies_polling_interval_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());
        let overrides = CliOverrides {
            polling_interval_ms: Some(60000),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(result.polling.interval_ms, 60000);
    }

    #[test]
    fn rejects_zero_polling_interval_override() {
        let config = RuntimeConfig::default();
        let overrides = CliOverrides {
            polling_interval_ms: Some(0),
            ..Default::default()
        };

        assert_eq!(
            apply_cli_overrides(config, &overrides),
            Err(ConfigError::InvalidCliOverride {
                field: "polling.interval_ms".to_string(),
                reason: "must be greater than 0".to_string()
            })
        );
    }

    #[test]
    fn applies_max_concurrent_agents_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            max_concurrent_agents: Some(20),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(result.agent.max_concurrent_agents, 20);
    }

    #[test]
    fn rejects_zero_max_concurrent_agents_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            max_concurrent_agents: Some(0),
            ..Default::default()
        };

        assert_eq!(
            apply_cli_overrides(config, &overrides),
            Err(ConfigError::InvalidCliOverride {
                field: "agent.max_concurrent_agents".to_string(),
                reason: "must be greater than 0".to_string()
            })
        );
    }

    #[test]
    fn applies_workspace_root_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            workspace_root: Some(std::path::PathBuf::from("/tmp/test-workspaces")),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(
            result.workspace.root,
            std::path::PathBuf::from("/tmp/test-workspaces")
        );
    }

    #[test]
    fn applies_valid_log_level_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            log_level: Some("DEBUG".to_string()),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(result.log_level.level, "debug");
    }

    #[test]
    fn rejects_invalid_log_level_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            log_level: Some("invalid".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            apply_cli_overrides(config, &overrides),
            Err(ConfigError::InvalidCliOverride {
                field,
                reason: _
            }) if field == "log.level"
        ));
    }

    #[test]
    fn applies_tracker_endpoint_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            tracker_endpoint: Some("https://custom.linear.app/graphql".to_string()),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(
            result.tracker.endpoint,
            "https://custom.linear.app/graphql".to_string()
        );
    }

    #[test]
    fn applies_tracker_api_key_override() {
        let mut config = RuntimeConfig::default();
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            tracker_api_key: Some("cli-api-key".to_string()),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(result.tracker.api_key, Some("cli-api-key".to_string()));
    }

    #[test]
    fn applies_multiple_overrides() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let overrides = CliOverrides {
            polling_interval_ms: Some(45000),
            max_concurrent_agents: Some(15),
            log_level: Some("warn".to_string()),
            tracker_endpoint: Some("https://custom.endpoint.com".to_string()),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply override");
        assert_eq!(result.polling.interval_ms, 45000);
        assert_eq!(result.agent.max_concurrent_agents, 15);
        assert_eq!(result.log_level.level, "warn");
        assert_eq!(
            result.tracker.endpoint,
            "https://custom.endpoint.com".to_string()
        );
    }

    #[test]
    fn empty_overrides_leaves_config_unchanged() {
        let mut config = RuntimeConfig::default();
        config.tracker.api_key = Some("token".to_owned());
        config.tracker.project_slug = Some("symphony".to_owned());

        let original_config = config.clone();
        let overrides = CliOverrides::default();

        let result = apply_cli_overrides(config, &overrides).expect("should keep original");
        assert_eq!(
            result.polling.interval_ms,
            original_config.polling.interval_ms
        );
        assert_eq!(
            result.agent.max_concurrent_agents,
            original_config.agent.max_concurrent_agents
        );
    }

    #[test]
    fn override_takes_precedence_over_config_values() {
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
  endpoint: https://api.linear.app/graphql
polling:
  interval_ms: 30000
agent:
  max_concurrent_agents: 10
"#,
        );

        let config = from_front_matter(&front_matter).expect("config should parse");

        let overrides = CliOverrides {
            polling_interval_ms: Some(60000),
            max_concurrent_agents: Some(20),
            tracker_endpoint: Some("https://override.endpoint.com".to_string()),
            ..Default::default()
        };

        let result = apply_cli_overrides(config, &overrides).expect("should apply overrides");
        assert_eq!(result.polling.interval_ms, 60000);
        assert_eq!(result.agent.max_concurrent_agents, 20);
        assert_eq!(
            result.tracker.endpoint,
            "https://override.endpoint.com".to_string()
        );
    }

    #[test]
    fn is_empty_returns_true_for_no_overrides() {
        let overrides = CliOverrides::default();
        assert!(overrides.is_empty());
    }

    #[test]
    fn is_empty_returns_false_with_any_override() {
        let overrides = CliOverrides {
            polling_interval_ms: Some(1000),
            ..Default::default()
        };
        assert!(!overrides.is_empty());
    }
}
