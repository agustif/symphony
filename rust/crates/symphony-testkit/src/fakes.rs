use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use symphony_agent_protocol::{AppServerEvent, ProtocolError, decode_stdout_line};
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};

use crate::issue_id;

pub fn tracker_issue(identifier: &str, state: &str) -> TrackerIssue {
    TrackerIssue::new(issue_id(identifier), identifier, TrackerState::new(state))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FakeTrackerResponse {
    Issues(Vec<TrackerIssue>),
    Error(String),
    Delay {
        duration: Duration,
        response: Box<FakeTrackerResponse>,
    },
}

impl FakeTrackerResponse {
    pub fn issues(issues: Vec<TrackerIssue>) -> Self {
        Self::Issues(issues)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    pub fn delay(duration: Duration, response: FakeTrackerResponse) -> Self {
        Self::Delay {
            duration,
            response: Box::new(response),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FakeTrackerConfig {
    /// Whether to simulate network latency
    pub latency: Option<Duration>,
    /// Whether to fail after a certain number of calls
    pub fail_after: Option<usize>,
    /// Custom error message for failures
    pub error_message: Option<String>,
}

impl Default for FakeTrackerConfig {
    fn default() -> Self {
        Self {
            latency: None,
            fail_after: None,
            error_message: None,
        }
    }
}

impl FakeTrackerConfig {
    pub fn with_latency(mut self, duration: Duration) -> Self {
        self.latency = Some(duration);
        self
    }

    pub fn fail_after(mut self, count: usize) -> Self {
        self.fail_after = Some(count);
        self
    }

    pub fn with_error_message(mut self, message: impl Into<String>) -> Self {
        self.error_message = Some(message.into());
        self
    }
}

#[derive(Debug)]
pub struct FakeTracker {
    scripted: Mutex<VecDeque<FakeTrackerResponse>>,
    default_issues: Vec<TrackerIssue>,
    fetch_count: AtomicUsize,
    config: FakeTrackerConfig,
}

impl Default for FakeTracker {
    fn default() -> Self {
        Self {
            scripted: Mutex::new(VecDeque::new()),
            default_issues: Vec::new(),
            fetch_count: AtomicUsize::new(0),
            config: FakeTrackerConfig::default(),
        }
    }
}

impl FakeTracker {
    pub fn scripted(scripted: Vec<FakeTrackerResponse>) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            ..Self::default()
        }
    }

    pub fn with_config(config: FakeTrackerConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn with_default_issues(default_issues: Vec<TrackerIssue>) -> Self {
        Self {
            default_issues,
            ..Self::default()
        }
    }

    pub fn push_response(&self, response: FakeTrackerResponse) {
        let mut guard = self
            .scripted
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.push_back(response);
    }

    pub fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::Relaxed)
    }

    pub fn remaining_scripted(&self) -> usize {
        let guard = self
            .scripted
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.len()
    }

    pub fn reset_fetch_count(&self) {
        self.fetch_count.store(0, Ordering::Relaxed);
    }

    async fn apply_latency(&self) {
        if let Some(latency) = self.config.latency {
            tokio::time::sleep(latency).await;
        }
    }

    fn check_fail_condition(&self) -> Option<TrackerError> {
        if let Some(fail_after) = self.config.fail_after {
            let count = self.fetch_count.load(Ordering::Relaxed);
            if count >= fail_after {
                let message = self
                    .config
                    .error_message
                    .clone()
                    .unwrap_or_else(|| format!("Failed after {} calls", fail_after));
                return Some(TrackerError::transport(message));
            }
        }
        None
    }
}

#[async_trait]
impl TrackerClient for FakeTracker {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
        self.apply_latency().await;

        if let Some(err) = self.check_fail_condition() {
            return Err(err);
        }

        self.fetch_count.fetch_add(1, Ordering::Relaxed);

        let response = {
            let mut guard = self
                .scripted
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.pop_front()
        };

        match response.unwrap_or_else(|| FakeTrackerResponse::Issues(self.default_issues.clone())) {
            FakeTrackerResponse::Issues(issues) => Ok(issues),
            FakeTrackerResponse::Error(message) => Err(TrackerError::transport(message)),
            FakeTrackerResponse::Delay { duration, response } => {
                tokio::time::sleep(duration).await;
                match *response {
                    FakeTrackerResponse::Issues(issues) => Ok(issues),
                    FakeTrackerResponse::Error(msg) => Err(TrackerError::transport(msg)),
                    FakeTrackerResponse::Delay { .. } => {
                        unreachable!("Nested delays not supported")
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FakeAppServerStream {
    lines: VecDeque<String>,
}

impl FakeAppServerStream {
    pub fn from_methods(methods: &[&str]) -> Self {
        let mut stream = Self::default();
        for method in methods {
            stream.push_valid(method);
        }
        stream
    }

    pub fn push_valid(&mut self, method: &str) {
        let escaped = method.replace('\\', "\\\\").replace('"', "\\\"");
        self.lines.push_back(format!(r#"{{"method":"{escaped}"}}"#));
    }

    pub fn push_malformed(&mut self, line: &str) {
        self.lines.push_back(line.to_owned());
    }

    pub fn pop_line(&mut self) -> Option<String> {
        self.lines.pop_front()
    }

    pub fn decode_next(&mut self) -> Option<Result<AppServerEvent, ProtocolError>> {
        self.pop_line().map(|line| decode_stdout_line(&line))
    }

    pub fn drain_decoded(&mut self) -> Vec<Result<AppServerEvent, ProtocolError>> {
        let mut decoded = Vec::new();
        while let Some(line) = self.decode_next() {
            decoded.push(line);
        }
        decoded
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct FakeWorkspaceHooks {
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeWorkspaceHooks {
    pub fn record(&self, hook_name: &str) -> usize {
        let mut calls = self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        calls.push(hook_name.to_owned());
        calls.len()
    }

    pub fn calls(&self) -> Vec<String> {
        let calls = self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        calls.clone()
    }

    pub fn clear(&self) {
        let mut calls = self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        calls.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_tracker_replays_script_and_counts_fetches() {
        let tracker = FakeTracker::scripted(vec![
            FakeTrackerResponse::Issues(vec![tracker_issue("SYM-1", "Todo")]),
            FakeTrackerResponse::Error("temporary outage".to_owned()),
        ]);

        let first = tracker.fetch_candidates().await;
        let second = tracker.fetch_candidates().await;

        assert!(first.is_ok());
        assert!(second.is_err());
        assert_eq!(tracker.fetch_count(), 2);
        assert_eq!(tracker.remaining_scripted(), 0);
    }

    #[test]
    fn fake_protocol_stream_decodes_and_surfaces_invalid_lines() {
        let mut stream = FakeAppServerStream::from_methods(&["turn.start"]);
        stream.push_malformed("{not-json}");
        let decoded = stream.drain_decoded();

        assert_eq!(
            decoded[0].as_ref().map(|message| message.method.as_str()),
            Ok("turn.start")
        );
        assert!(matches!(
            decoded[1],
            Err(ProtocolError::InvalidStdoutLine(_))
        ));
    }

    #[test]
    fn fake_workspace_hooks_preserve_call_order() {
        let hooks = FakeWorkspaceHooks::default();
        hooks.record("preflight");
        hooks.record("postflight");
        assert_eq!(hooks.calls(), vec!["preflight", "postflight"]);
        hooks.clear();
        assert!(hooks.calls().is_empty());
    }
}
