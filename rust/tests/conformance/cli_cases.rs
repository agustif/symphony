#![forbid(unsafe_code)]

use std::collections::HashMap;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::net::{Ipv4Addr, TcpListener, TcpStream};
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "../../crates/symphony-cli/src/reload_classification.rs"]
mod reload_classification;

use reload_classification::{
    HostOwnedConfigChange, WorkflowReloadDisposition, classify_workflow_reload,
};
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

#[test]
fn cli_port_override_takes_precedence_over_workflow_server_port() {
    let env = TestEnv::from_entries(&[("LINEAR_API_KEY", "env-token")]);
    let config = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  port: 3000
---
Run tests.
"#,
        &env,
    );
    let overrides = CliOverrides {
        server_port: Some(9000),
        ..Default::default()
    };

    let applied = apply_cli_overrides(config, &overrides).expect("overrides should apply");
    assert_eq!(applied.server.port, Some(9000));
}

#[test]
fn cli_bind_override_takes_precedence_over_workflow_server_host() {
    let config = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  host: 127.0.0.1
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let overrides = CliOverrides {
        server_host: Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
        ..Default::default()
    };

    let applied = apply_cli_overrides(config, &overrides).expect("overrides should apply");
    assert_eq!(
        applied.server.host,
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    );
}

#[test]
fn workflow_reload_classifies_server_port_changes_as_restart_required() {
    let current = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  port: 3000
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let next = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  port: 4051
---
Run tests.
"#,
        &TestEnv::default(),
    );

    assert_eq!(
        classify_workflow_reload(&current, &next),
        WorkflowReloadDisposition::RestartRequired {
            reasons: vec![HostOwnedConfigChange::ServerPort],
        }
    );
}

#[test]
fn workflow_reload_classifies_server_host_changes_as_restart_required() {
    let current = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  host: 127.0.0.1
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let next = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
server:
  host: 0.0.0.0
---
Run tests.
"#,
        &TestEnv::default(),
    );

    assert_eq!(
        classify_workflow_reload(&current, &next),
        WorkflowReloadDisposition::RestartRequired {
            reasons: vec![HostOwnedConfigChange::ServerHost],
        }
    );
}

#[test]
fn workflow_reload_classifies_tracker_changes_as_restart_required() {
    let current = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
  active_states:
    - Todo
    - In Progress
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let mut next = config_from_workflow(
        r#"---
tracker:
  kind: linear
  endpoint: https://example.invalid/graphql
  project_slug: next-project
  active_states:
    - Todo
    - Doing
---
Run tests.
"#,
        &TestEnv::from_entries(&[("LINEAR_API_KEY", "next-token")]),
    );
    next.tracker.kind = "mock-tracker".to_owned();
    next.log_level.level = "debug".to_owned();

    assert_eq!(
        classify_workflow_reload(&current, &next),
        WorkflowReloadDisposition::RestartRequired {
            reasons: vec![
                HostOwnedConfigChange::TrackerKind,
                HostOwnedConfigChange::TrackerEndpoint,
                HostOwnedConfigChange::TrackerApiKey,
                HostOwnedConfigChange::TrackerProjectSlug,
                HostOwnedConfigChange::TrackerActiveStates,
                HostOwnedConfigChange::LogLevel,
            ],
        }
    );
}

#[test]
fn workflow_reload_classifies_workspace_root_changes_as_restart_required() {
    let current = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
workspace:
  root: /tmp/symphony-current
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let next = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
workspace:
  root: /tmp/symphony-next
---
Run tests.
"#,
        &TestEnv::default(),
    );

    assert_eq!(
        classify_workflow_reload(&current, &next),
        WorkflowReloadDisposition::RestartRequired {
            reasons: vec![HostOwnedConfigChange::WorkspaceRoot],
        }
    );
}

#[test]
fn workflow_reload_keeps_runtime_safe_changes_live_applicable() {
    let current = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
polling:
  interval_ms: 15000
agent:
  max_turns: 8
---
Run tests.
"#,
        &TestEnv::default(),
    );
    let next = config_from_workflow(
        r#"---
tracker:
  kind: linear
  project_slug: env-project
polling:
  interval_ms: 45000
agent:
  max_turns: 21
---
Run tests.
"#,
        &TestEnv::default(),
    );

    assert_eq!(
        classify_workflow_reload(&current, &next),
        WorkflowReloadDisposition::LiveApply
    );
}

#[test]
fn workflow_reload_restart_reasons_report_stable_field_paths() {
    let reasons = [
        HostOwnedConfigChange::TrackerKind,
        HostOwnedConfigChange::TrackerEndpoint,
        HostOwnedConfigChange::TrackerApiKey,
        HostOwnedConfigChange::TrackerProjectSlug,
        HostOwnedConfigChange::TrackerActiveStates,
        HostOwnedConfigChange::WorkspaceRoot,
        HostOwnedConfigChange::ServerHost,
        HostOwnedConfigChange::ServerPort,
        HostOwnedConfigChange::LogLevel,
    ];

    assert_eq!(
        reasons.map(HostOwnedConfigChange::field_path),
        [
            "tracker.kind",
            "tracker.endpoint",
            "tracker.api_key",
            "tracker.project_slug",
            "tracker.active_states",
            "workspace.root",
            "server.host",
            "server.port",
            "log_level.level",
        ]
    );
}

#[cfg(unix)]
struct HostProcess {
    child: Child,
}

#[cfg(unix)]
impl HostProcess {
    fn id(&self) -> u32 {
        self.child.id()
    }

    fn try_wait(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().expect("child wait should succeed")
    }

    fn wait(&mut self) -> std::process::ExitStatus {
        self.child.wait().expect("child wait should succeed")
    }

    fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }
}

#[cfg(unix)]
impl Drop for HostProcess {
    fn drop(&mut self) {
        if self.try_wait().is_none() {
            send_signal(self.id(), "KILL");
            let _ = self.child.wait();
        }
    }
}

#[cfg(unix)]
#[test]
fn cli_host_initializes_logs_root_and_shuts_down_cleanly_with_http_enabled() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root =
        host_temp_dir("cli_host_initializes_logs_root_and_shuts_down_cleanly_with_http_enabled");
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    let port = reserve_local_port();
    write_host_workflow(&workflow_path, Some(port), "Prompt body.");

    let mut host = spawn_cli_host(&binary_path, &workflow_path, &logs_root);

    wait_for(
        Duration::from_secs(5),
        || log_path.exists() && http_get(port, "/api/v1/state").is_some(),
        "CLI host should initialize logs root and HTTP server",
    );
    send_signal(host.id(), "TERM");
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should terminate after SIGTERM",
    );

    assert!(status.success(), "expected clean shutdown, got {status}");
    assert!(
        log_path.exists(),
        "expected log file at {}",
        log_path.display()
    );
    let log_contents = fs::read_to_string(&log_path).expect("log file should be readable");
    assert!(log_contents.contains("HTTP server started"));
}

#[cfg(unix)]
#[test]
fn cli_host_runs_without_http_and_shuts_down_cleanly() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root = host_temp_dir("cli_host_runs_without_http_and_shuts_down_cleanly");
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    write_host_workflow(&workflow_path, None, "Prompt body.");

    let mut host = spawn_cli_host(&binary_path, &workflow_path, &logs_root);

    wait_for(
        Duration::from_secs(5),
        || log_path.exists() && log_contains(&log_path, "Starting poll loop"),
        "CLI host should initialize logs root and poll loop without HTTP",
    );
    send_signal(host.id(), "TERM");
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should terminate after SIGTERM without HTTP enabled",
    );

    assert!(status.success(), "expected clean shutdown, got {status}");
    let log_contents = fs::read_to_string(&log_path).expect("log file should be readable");
    assert!(log_contents.contains("Starting poll loop"));
    assert!(!log_contents.contains("HTTP server started"));
}

#[cfg(unix)]
#[test]
fn cli_host_restart_required_reload_exits_and_releases_http_port() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root = host_temp_dir("cli_host_restart_required_reload_exits_and_releases_http_port");
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    let port = reserve_local_port();
    write_host_workflow(&workflow_path, Some(port), "Prompt body.");

    let mut host = spawn_cli_host(&binary_path, &workflow_path, &logs_root);

    wait_for(
        Duration::from_secs(5),
        || http_get(port, "/api/v1/state").is_some(),
        "CLI host should start HTTP server before restart-required reload",
    );

    write_host_workflow_with_endpoint(
        &workflow_path,
        Some(port),
        "http://127.0.0.1:10/graphql",
        "Prompt body after host-owned change.",
    );

    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(8),
        "CLI host should exit when host-owned workflow config changes",
    );

    assert_eq!(status.code(), Some(14));
    assert!(
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok(),
        "expected HTTP port {port} to be released after restart-required exit"
    );
    let log_contents = fs::read_to_string(&log_path).expect("log file should be readable");
    assert!(log_contents.contains("restart required before applying new workflow"));
    assert!(log_contents.contains("tracker.endpoint"));
}

#[cfg(unix)]
#[test]
fn cli_host_logs_retained_invalid_reload_and_keeps_serving_previous_http_state() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root = host_temp_dir(
        "cli_host_logs_retained_invalid_reload_and_keeps_serving_previous_http_state",
    );
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    let port = reserve_local_port();
    write_host_workflow(&workflow_path, Some(port), "Prompt body.");

    let mut host = spawn_cli_host(&binary_path, &workflow_path, &logs_root);

    wait_for(
        Duration::from_secs(5),
        || http_get(port, "/api/v1/state").is_some(),
        "CLI host should start HTTP server before retained-invalid reload",
    );

    fs::write(
        &workflow_path,
        r#"---
tracker:
  kind: linear
  endpoint: http://127.0.0.1:9/graphql
  api_key: [
  project_slug: SYM
server:
  port: 0
---
Broken prompt body.
"#,
    )
    .expect("workflow file should be writable");

    wait_for(
        Duration::from_secs(8),
        || log_contains(&log_path, "retaining last applied workflow"),
        "CLI host should log retained-invalid reload diagnostics",
    );
    assert!(
        http_get(port, "/api/v1/state").is_some(),
        "expected HTTP server to keep serving the last applied workflow after invalid reload"
    );

    send_signal(host.id(), "TERM");
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should terminate cleanly after retained-invalid reload",
    );

    assert!(status.success(), "expected clean shutdown, got {status}");
}

#[cfg(unix)]
#[test]
fn cli_host_runtime_safe_reload_applies_live_and_keeps_refresh_available() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root =
        host_temp_dir("cli_host_runtime_safe_reload_applies_live_and_keeps_refresh_available");
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    let port = reserve_local_port();
    write_host_workflow(&workflow_path, Some(port), "Prompt body.");

    let mut host = spawn_cli_host(&binary_path, &workflow_path, &logs_root);

    wait_for(
        Duration::from_secs(5),
        || http_get(port, "/api/v1/state").is_some(),
        "CLI host should start HTTP server before runtime-safe reload",
    );

    write_host_workflow_with_config(
        &workflow_path,
        Some(port),
        "http://127.0.0.1:9/graphql",
        500,
        "Updated prompt body.",
    );

    wait_for(
        Duration::from_secs(8),
        || {
            log_contains(&log_path, "workflow reload applied")
                && (log_contains(&log_path, "refresh signal queued")
                    || log_contains(&log_path, "refresh signal coalesced"))
        },
        "CLI host should apply runtime-safe reload and enqueue a refresh",
    );

    assert!(
        host.try_wait().is_none(),
        "host should remain running after runtime-safe reload"
    );
    assert!(
        http_get(port, "/api/v1/state").is_some(),
        "HTTP state route should remain available after runtime-safe reload"
    );
    let refresh_response = http_request("POST", port, "/api/v1/refresh")
        .expect("refresh route should remain available after runtime-safe reload");
    assert!(
        refresh_response.starts_with("HTTP/1.1 202"),
        "expected accepted refresh response, got {refresh_response}"
    );

    send_signal(host.id(), "TERM");
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should terminate cleanly after runtime-safe reload",
    );

    assert!(status.success(), "expected clean shutdown, got {status}");
}

#[cfg(unix)]
#[test]
fn cli_host_exits_with_http_bind_failure_code_and_message() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root = host_temp_dir("cli_host_exits_with_http_bind_failure_code_and_message");
    let workflow_path = root.join("WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");
    let occupied_listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("occupied listener should bind");
    let port = occupied_listener
        .local_addr()
        .expect("occupied listener addr should resolve")
        .port();
    write_host_workflow(&workflow_path, Some(port), "Prompt body.");

    let mut host = spawn_cli_host_with_stderr(&binary_path, &workflow_path, &logs_root);
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should fail fast when HTTP bind fails",
    );

    assert_eq!(status.code(), Some(12));
    let mut stderr = String::new();
    host.take_stderr()
        .expect("stderr should be piped for bind-failure test")
        .read_to_string(&mut stderr)
        .expect("stderr should be readable");
    assert!(stderr.contains("HTTP server task failed"));
    assert!(stderr.contains("failed to bind HTTP listener"));
    assert!(
        log_path.exists(),
        "logs root should still initialize before bind failure"
    );
}

#[cfg(unix)]
#[test]
fn cli_host_exits_with_missing_workflow_error_code_and_message() {
    let _guard = host_test_guard()
        .lock()
        .expect("host test mutex should lock");
    let Some(binary_path) = cli_binary_path() else {
        return;
    };
    let root = host_temp_dir("cli_host_exits_with_missing_workflow_error_code_and_message");
    let workflow_path = root.join("missing-WORKFLOW.md");
    let logs_root = root.join("logs");
    let log_path = logs_root.join("symphony.log");

    let mut host = spawn_cli_host_with_stderr(&binary_path, &workflow_path, &logs_root);
    let status = wait_for_exit(
        &mut host,
        Duration::from_secs(5),
        "CLI host should fail fast when the workflow path is missing",
    );

    assert_eq!(status.code(), Some(1));
    let mut stderr = String::new();
    host.take_stderr()
        .expect("stderr should be piped for missing-workflow test")
        .read_to_string(&mut stderr)
        .expect("stderr should be readable");
    assert!(stderr.contains("startup validation failed"));
    assert!(stderr.contains("workflow file does not exist"));
    assert!(
        !log_path.exists(),
        "logging should not initialize before workflow startup validation passes"
    );
}

#[cfg(unix)]
fn cli_binary_path() -> Option<PathBuf> {
    std::env::var_os("CARGO_BIN_EXE_symphony-cli").map(PathBuf::from)
}

#[cfg(unix)]
fn host_temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-cli-conformance-{name}-{nonce}"));
    fs::create_dir_all(&path).expect("temp directory should be creatable");
    path
}

#[cfg(unix)]
fn reserve_local_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("local addr should resolve")
        .port()
}

#[cfg(unix)]
fn host_test_guard() -> &'static Mutex<()> {
    static HOST_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    HOST_TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
fn spawn_cli_host(binary_path: &Path, workflow_path: &Path, logs_root: &Path) -> HostProcess {
    spawn_cli_host_with(binary_path, workflow_path, logs_root, Stdio::null())
}

#[cfg(unix)]
fn spawn_cli_host_with_stderr(
    binary_path: &Path,
    workflow_path: &Path,
    logs_root: &Path,
) -> HostProcess {
    spawn_cli_host_with(binary_path, workflow_path, logs_root, Stdio::piped())
}

#[cfg(unix)]
fn spawn_cli_host_with(
    binary_path: &Path,
    workflow_path: &Path,
    logs_root: &Path,
    stderr: Stdio,
) -> HostProcess {
    let child = Command::new(binary_path)
        .arg(workflow_path)
        .arg("--logs-root")
        .arg(logs_root)
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .expect("CLI host should spawn");
    HostProcess { child }
}

#[cfg(unix)]
fn write_host_workflow(workflow_path: &Path, port: Option<u16>, prompt_body: &str) {
    write_host_workflow_with_config(
        workflow_path,
        port,
        "http://127.0.0.1:9/graphql",
        250,
        prompt_body,
    );
}

#[cfg(unix)]
fn write_host_workflow_with_endpoint(
    workflow_path: &Path,
    port: Option<u16>,
    endpoint: &str,
    prompt_body: &str,
) {
    write_host_workflow_with_config(workflow_path, port, endpoint, 250, prompt_body);
}

#[cfg(unix)]
fn write_host_workflow_with_config(
    workflow_path: &Path,
    port: Option<u16>,
    endpoint: &str,
    polling_interval_ms: u64,
    prompt_body: &str,
) {
    let server_section = port
        .map(|port| format!("server:\n  port: {port}\n"))
        .unwrap_or_default();
    fs::write(
        workflow_path,
        format!(
            r#"---
tracker:
  kind: linear
  endpoint: {endpoint}
  api_key: token
  project_slug: SYM
polling:
  interval_ms: {polling_interval_ms}
{server_section}---
{prompt_body}
"#
        ),
    )
    .expect("workflow file should be writable");
}

#[cfg(unix)]
fn http_get(port: u16, path: &str) -> Option<String> {
    http_request("GET", port, path)
}

#[cfg(unix)]
fn http_request(method: &str, port: u16, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("read timeout should be configurable");
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .expect("write timeout should be configurable");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    if response.starts_with("HTTP/1.1 200")
        || response.starts_with("HTTP/1.1 202")
        || response.starts_with("HTTP/1.1 404")
    {
        Some(response)
    } else {
        None
    }
}

#[cfg(unix)]
fn log_contains(log_path: &Path, needle: &str) -> bool {
    fs::read_to_string(log_path)
        .map(|contents| contents.contains(needle))
        .unwrap_or(false)
}

#[cfg(unix)]
fn wait_for<F>(timeout: Duration, mut condition: F, description: &str)
where
    F: FnMut() -> bool,
{
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    panic!("{description}");
}

#[cfg(unix)]
fn wait_for_exit(
    host: &mut HostProcess,
    timeout: Duration,
    description: &str,
) -> std::process::ExitStatus {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Some(status) = host.try_wait() {
            return status;
        }
        thread::sleep(Duration::from_millis(100));
    }

    send_signal(host.id(), "KILL");
    let _ = host.wait();
    panic!("{description}");
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .expect("kill command should spawn");
    assert!(status.success(), "kill -{signal} {pid} failed");
}
