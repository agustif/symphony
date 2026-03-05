use crate::{ApiResponse, ISSUE_ROUTE_PREFIX, STATE_ROUTE};
use symphony_observability::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

pub fn snapshot_to_json(snapshot: &RuntimeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "running": snapshot.running,
        "retrying": snapshot.retrying,
    })
}

pub fn issue_to_json(issue: &IssueSnapshot) -> serde_json::Value {
    serde_json::json!({
        "id": issue.id.0.as_str(),
        "identifier": issue.identifier.as_str(),
        "state": issue.state.as_str(),
        "retry_attempts": issue.retry_attempts,
    })
}

pub fn state_to_json(snapshot: &StateSnapshot) -> serde_json::Value {
    let issues = snapshot
        .issues
        .iter()
        .map(issue_to_json)
        .collect::<Vec<_>>();
    serde_json::json!({
        "runtime": snapshot_to_json(&snapshot.runtime),
        "issues": issues,
    })
}

pub fn handle_get(path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    if path == STATE_ROUTE {
        return ApiResponse::ok(state_to_json(snapshot));
    }

    if let Some(issue_id) = path.strip_prefix(ISSUE_ROUTE_PREFIX) {
        if issue_id.is_empty() || issue_id.contains('/') {
            return ApiResponse::not_found("route not found");
        }

        return snapshot.issue(issue_id).map_or_else(
            || ApiResponse::not_found(format!("issue `{issue_id}` not found")),
            |issue| {
                ApiResponse::ok(serde_json::json!({
                    "issue": issue_to_json(issue),
                }))
            },
        );
    }

    ApiResponse::not_found("route not found")
}
