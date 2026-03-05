#![forbid(unsafe_code)]

use symphony_http::{STATE_ROUTE, handle_get, issue_route};
use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};

#[test]
fn http_state_route_returns_runtime_and_issue_data() {
    let snapshot = state_snapshot(
        runtime_snapshot(2, 1),
        vec![issue_snapshot("SYM-1", "SYM-1", "Running", 0)],
    );

    let response = handle_get(STATE_ROUTE, &snapshot);
    assert_eq!(response.status, 200);
    assert_eq!(response.body["runtime"]["running"], 2);
    assert_eq!(response.body["runtime"]["retrying"], 1);
    assert_eq!(response.body["issues"][0]["id"], "SYM-1");
}

#[test]
fn http_issue_route_returns_single_issue_view() {
    let snapshot = state_snapshot(
        runtime_snapshot(1, 0),
        vec![issue_snapshot("SYM-42", "SYM-42", "Retrying", 3)],
    );

    let response = handle_get(&issue_route("SYM-42"), &snapshot);
    assert_eq!(response.status, 200);
    assert_eq!(response.body["issue"]["id"], "SYM-42");
    assert_eq!(response.body["issue"]["retry_attempts"], 3);
}

#[test]
fn http_issue_route_returns_not_found_for_missing_issue() {
    let response = handle_get(
        &issue_route("SYM-999"),
        &state_snapshot(runtime_snapshot(0, 0), vec![]),
    );
    assert_eq!(response.status, 404);
    assert_eq!(response.body["error"], "issue `SYM-999` not found");
}
