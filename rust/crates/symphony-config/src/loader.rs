use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use serde_yaml::{Mapping, Value};

use crate::{
    ConfigError, EnvProvider, ProcessEnv, RuntimeConfig,
    env::{expand_workspace_root, resolve_env_reference},
    model::{
        AgentConfig, CodexConfig, DEFAULT_HOOK_TIMEOUT_MS, DEFAULT_LINEAR_ENDPOINT, HooksConfig,
        PollingConfig, TrackerConfig, WorkspaceConfig,
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
mod tests {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };

    use serde_yaml::{Mapping, Value};

    use crate::{
        ConfigError, EnvProvider,
        loader::from_front_matter_with_env,
        model::{DEFAULT_HOOK_TIMEOUT_MS, DEFAULT_LINEAR_ENDPOINT},
    };

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
        let front_matter = parse_yaml(
            r#"
tracker:
  kind: linear
  api_key: token
  project_slug: symphony
codex:
  command: "   "
"#,
        );

        assert_eq!(
            from_front_matter_with_env(&front_matter, &TestEnv::from_entries(&[])),
            Err(ConfigError::MissingCodexCommand)
        );
    }

    fn parse_yaml(yaml: &str) -> Mapping {
        let value: Value = serde_yaml::from_str(yaml).expect("yaml should parse");
        value
            .as_mapping()
            .cloned()
            .expect("root should be a mapping")
    }
}
