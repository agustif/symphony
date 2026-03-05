use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use symphony_agent_protocol::{ProtocolError, ProtocolMessage, decode_stdout_line};
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue};

use crate::issue_id;

pub fn tracker_issue(identifier: &str, state: &str) -> TrackerIssue {
    TrackerIssue {
        id: issue_id(identifier),
        identifier: identifier.to_owned(),
        state: state.to_owned(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FakeTrackerResponse {
    Issues(Vec<TrackerIssue>),
    Error(String),
}

#[derive(Debug)]
pub struct FakeTracker {
    scripted: Mutex<VecDeque<FakeTrackerResponse>>,
    default_issues: Vec<TrackerIssue>,
    fetch_count: AtomicUsize,
}

impl Default for FakeTracker {
    fn default() -> Self {
        Self {
            scripted: Mutex::new(VecDeque::new()),
            default_issues: Vec::new(),
            fetch_count: AtomicUsize::new(0),
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
}

#[async_trait]
impl TrackerClient for FakeTracker {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
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
            FakeTrackerResponse::Error(message) => Err(TrackerError::Request(message)),
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

    pub fn decode_next(&mut self) -> Option<Result<ProtocolMessage, ProtocolError>> {
        self.pop_line().map(|line| decode_stdout_line(&line))
    }

    pub fn drain_decoded(&mut self) -> Vec<Result<ProtocolMessage, ProtocolError>> {
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
        assert_eq!(decoded[1], Err(ProtocolError::InvalidLine));
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
