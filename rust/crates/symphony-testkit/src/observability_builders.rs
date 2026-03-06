use symphony_observability::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

use crate::issue_id;

pub fn runtime_snapshot(running: usize, retrying: usize) -> RuntimeSnapshot {
    RuntimeSnapshot {
        running,
        retrying,
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        seconds_running: 0.0,
        rate_limits: None,
        activity: symphony_observability::RuntimeActivitySnapshot::default(),
    }
}

pub fn issue_snapshot(
    id: &str,
    identifier: &str,
    state: &str,
    retry_attempts: u32,
) -> IssueSnapshot {
    IssueSnapshot {
        id: issue_id(id),
        identifier: identifier.to_owned(),
        state: state.to_owned(),
        retry_attempts,
        workspace_path: None,
        session_id: None,
        turn_count: 0,
        last_event: None,
        last_message: None,
        started_at: None,
        last_event_at: None,
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        retry_due_at: None,
        retry_error: None,
    }
}

pub fn state_snapshot(runtime: RuntimeSnapshot, issues: Vec<IssueSnapshot>) -> StateSnapshot {
    StateSnapshot { runtime, issues }
}
