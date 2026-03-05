use symphony_observability::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

use crate::issue_id;

pub fn runtime_snapshot(running: usize, retrying: usize) -> RuntimeSnapshot {
    RuntimeSnapshot { running, retrying }
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
    }
}

pub fn state_snapshot(runtime: RuntimeSnapshot, issues: Vec<IssueSnapshot>) -> StateSnapshot {
    StateSnapshot { runtime, issues }
}
