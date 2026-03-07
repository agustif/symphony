#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::{env, fs};

use symphony_config::EnvProvider;

mod cli_cases;
mod http_cases;
mod observability_cases;
mod orchestrator_cases;
mod protocol_cases;
mod tracker_cases;
mod workflow_cases;
mod workspace_cases;

use symphony_domain::{Event, OrchestratorState, RetryEntry};
use symphony_testkit::{command_count, issue_id, run_trace, run_trace_from_state, validate_trace};

pub(crate) const REAL_CONFORMANCE_OPT_IN_ENV: &str = "SYMPHONY_REAL_CONFORMANCE";
pub(crate) const REAL_LINEAR_ENV_FILE: &str = "/Users/af/symphony/.env.local";
pub(crate) const REAL_LINEAR_WORKFLOW_FILE: &str = "/Users/af/symphony/elixir/WORKFLOW.md";
pub(crate) const REAL_LINEAR_PROJECT_SLUG_ENV: &str = "LINEAR_PROJECT_SLUG";

#[derive(Debug, Clone)]
pub(crate) struct RealLinearConformanceConfig {
    pub api_key: String,
    pub project_slug: String,
}

#[derive(Debug, Default)]
pub(crate) struct ConformanceEnv {
    values: HashMap<String, String>,
}

impl ConformanceEnv {
    pub(crate) fn from_entries<I, K, V>(entries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            values: entries
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

impl EnvProvider for ConformanceEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

pub(crate) fn skip_real_integration(test_name: &str, reason: impl AsRef<str>) {
    eprintln!(
        "skipped real integration conformance `{test_name}`: {}",
        reason.as_ref()
    );
}

pub(crate) fn live_linear_conformance_config() -> Result<RealLinearConformanceConfig, String> {
    if !real_conformance_opt_in_enabled() {
        return Err(format!(
            "set {}=1 to enable live conformance preflight",
            REAL_CONFORMANCE_OPT_IN_ENV
        ));
    }

    let api_key = env::var("LINEAR_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_env_file_value(REAL_LINEAR_ENV_FILE, "LINEAR_API_KEY"))
        .ok_or_else(|| {
            format!(
                "missing LINEAR_API_KEY in environment or {}",
                REAL_LINEAR_ENV_FILE
            )
        })?;

    let project_slug = env::var(REAL_LINEAR_PROJECT_SLUG_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_env_file_value(REAL_LINEAR_ENV_FILE, REAL_LINEAR_PROJECT_SLUG_ENV))
        .or_else(|| read_workflow_project_slug(REAL_LINEAR_WORKFLOW_FILE))
        .ok_or_else(|| {
            format!(
                "missing {} in environment, {}, or {}",
                REAL_LINEAR_PROJECT_SLUG_ENV, REAL_LINEAR_ENV_FILE, REAL_LINEAR_WORKFLOW_FILE
            )
        })?;

    Ok(RealLinearConformanceConfig {
        api_key,
        project_slug,
    })
}

fn real_conformance_opt_in_enabled() -> bool {
    env::var(REAL_CONFORMANCE_OPT_IN_ENV)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn read_env_file_value(path: &str, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;

    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let (candidate_key, raw_value) = trimmed.split_once('=')?;
        if candidate_key.trim() != key {
            return None;
        }

        let value = raw_value.trim().trim_matches('"').trim_matches('\'').trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_owned())
        }
    })
}

fn read_workflow_project_slug(path: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut lines = contents.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    lines
        .take_while(|line| line.trim() != "---")
        .find_map(|line| {
            let (key, raw_value) = line.split_once(':')?;
            if key.trim() != "project_slug" {
                return None;
            }

            let value = raw_value.trim().trim_matches('"').trim_matches('\'').trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            }
        })
}

#[test]
fn lifecycle_claim_run_release_is_conformant() {
    let issue = issue_id("SYM-9001");
    let events = vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Release(issue),
    ];

    let trace = run_trace(events);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
    assert!(trace.final_state.retry_attempts.is_empty());
}

#[test]
fn mark_running_conformance_always_claims_issue() {
    let issue = issue_id("SYM-42");
    let trace = run_trace(vec![Event::MarkRunning(issue.clone())]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(!trace.final_state.running.contains_key(&issue));
    assert!(!trace.final_state.claimed.contains(&issue));
}

#[test]
fn release_conformance_clears_seeded_retry_tracking() {
    let issue = issue_id("SYM-77");
    let mut state = OrchestratorState::default();
    state.retry_attempts.insert(
        issue.clone(),
        RetryEntry {
            attempt: 3,
            ..RetryEntry::default()
        },
    );
    state.claimed.insert(issue.clone());

    let trace = run_trace_from_state(state, vec![Event::Release(issue)]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert!(trace.final_state.retry_attempts.is_empty());
    assert!(trace.final_state.claimed.is_empty());
    assert!(trace.final_state.running.is_empty());
}

#[test]
fn command_stream_conformance_matches_event_count() {
    let issue = issue_id("SYM-11");
    let trace = run_trace(vec![
        Event::Claim(issue.clone()),
        Event::MarkRunning(issue.clone()),
        Event::Claim(issue.clone()),
        Event::Release(issue),
    ]);

    assert_eq!(validate_trace(&trace), Ok(()));
    assert_eq!(command_count(&trace), 3);
}
