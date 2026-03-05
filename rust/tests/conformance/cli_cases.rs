#![forbid(unsafe_code)]

use std::collections::HashMap;

use symphony_config::{
    CliOverrides, ConfigError, EnvProvider, apply_cli_overrides, from_front_matter_with_env,
};
use symphony_workflow::parse as parse_workflow;

#[derive(Debug, Default)]
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

fn config_from_workflow(workflow: &str, env: &dyn EnvProvider) -> symphony_config::RuntimeConfig {
    let parsed = parse_workflow(workflow).expect("workflow should parse");
    from_front_matter_with_env(&parsed.front_matter, env).expect("front matter should build config")
}

fn try_config_from_workflow(
    workflow: &str,
    env: &dyn EnvProvider,
) -> Result<symphony_config::RuntimeConfig, ConfigError> {
    let parsed = parse_workflow(workflow).expect("workflow should parse");
    from_front_matter_with_env(&parsed.front_matter, env)
}

#[test]
fn cli_overrides_take_precedence_over_env_and_workflow_values() {
    let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "env-token")]);
    let config = config_from_workflow(
        r#"---
tracker:
  api_key: ${LINEAR_API_KEY}
  project_slug: file-project
polling:
  interval_ms: 15000
agent:
  max_turns: 8
---
Run tests.
"#,
        &env,
    );
    let overrides = CliOverrides {
        polling_interval_ms: Some(45_000),
        max_turns: Some(12),
        tracker_api_key: Some("cli-token".to_owned()),
        tracker_project_slug: Some("cli-project".to_owned()),
        ..Default::default()
    };

    let applied = apply_cli_overrides(config, &overrides).expect("overrides should apply");
    assert_eq!(applied.polling.interval_ms, 45_000);
    assert_eq!(applied.agent.max_turns, 12);
    assert_eq!(applied.tracker.api_key.as_deref(), Some("cli-token"));
    assert_eq!(applied.tracker.project_slug.as_deref(), Some("cli-project"));
}

#[test]
fn workflow_env_values_apply_when_cli_override_is_missing() {
    let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "env-token")]);
    let config = config_from_workflow(
        r#"---
tracker:
  api_key: ${LINEAR_API_KEY}
  project_slug: env-project
polling:
  interval_ms: 22000
---
Run tests.
"#,
        &env,
    );
    let applied =
        apply_cli_overrides(config, &CliOverrides::default()).expect("empty overrides should pass");

    assert_eq!(applied.polling.interval_ms, 22_000);
    assert_eq!(applied.tracker.api_key.as_deref(), Some("env-token"));
}

#[test]
fn linear_tracker_env_indirection_applies_default_endpoint() {
    let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "env-token")]);
    let config = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
---
Run tests.
"#,
        &env,
    );

    assert_eq!(config.tracker.endpoint, "https://api.linear.app/graphql");
    assert_eq!(config.tracker.api_key.as_deref(), Some("env-token"));
}

#[test]
fn invalid_config_field_type_is_rejected() {
    let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "env-token")]);
    let error = try_config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
polling:
  interval_ms: {}
---
Run tests.
"#,
        &env,
    )
    .expect_err("invalid polling interval type should fail");

    assert_eq!(
        error,
        ConfigError::InvalidType {
            field: "polling.interval_ms",
            expected: "integer or string integer",
        }
    );
}
