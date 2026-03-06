#![forbid(unsafe_code)]

use std::env::temp_dir;

use serde_json::json;
use symphony_http::{STATE_ROUTE, handle_get, handle_get_with_workspace_envelope, issue_route};
use symphony_observability::{SnapshotErrorView, StateSnapshotEnvelope};
use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};

#[test]
fn http_state_route_returns_runtime_and_issue_data() {
    let snapshot = state_snapshot(
        runtime_snapshot(2, 1),
        vec![
            issue_snapshot("SYM-1", "SYM-1", "Running", 0),
            issue_snapshot("SYM-2", "SYM-2", "Retrying", 1),
        ],
    );

    let response = handle_get(STATE_ROUTE, &snapshot);
    assert_eq!(response.status, 200);
    assert_eq!(response.body["counts"]["running"], 2);
    assert_eq!(response.body["counts"]["retrying"], 1);
    assert_eq!(response.body["running"][0]["issue_id"], "SYM-1");
    assert_eq!(response.body["retrying"][0]["issue_id"], "SYM-2");
    assert!(response.body["running"][0]["last_message"].is_null());
}

#[test]
fn http_issue_route_returns_single_issue_view() {
    let snapshot = state_snapshot(
        runtime_snapshot(1, 0),
        vec![issue_snapshot("SYM-42", "SYM-42", "Retrying", 3)],
    );

    let response = handle_get(&issue_route("SYM-42"), &snapshot);
    assert_eq!(response.status, 200);
    assert_eq!(response.body["issue_id"], "SYM-42");
    assert_eq!(response.body["attempts"]["current_retry_attempt"], 3);
    assert_eq!(response.body["attempts"]["restart_count"], 2);
    assert_eq!(response.body["retry"]["attempt"], 3);
    assert!(response.body["retry"].get("issue_id").is_none());
}

#[test]
fn http_issue_route_returns_not_found_for_missing_issue() {
    let response = handle_get(
        &issue_route("SYM-999"),
        &state_snapshot(runtime_snapshot(0, 0), vec![]),
    );
    assert_eq!(response.status, 404);
    assert_eq!(response.body["error"]["code"], "issue_not_found");
    assert_eq!(response.body["error"]["message"], "Issue not found");
}

#[test]
fn http_state_route_surfaces_snapshot_timeout_with_200_error_envelope() {
    let response =
        handle_get_with_workspace_envelope(STATE_ROUTE, &StateSnapshotEnvelope::timeout(), None);

    assert_eq!(response.status, 200);
    assert_eq!(response.body["error"]["code"], "snapshot_timeout");
    assert_eq!(response.body["error"]["message"], "Snapshot timed out");
}

#[test]
fn http_state_route_serves_cached_state_with_error_marker_when_stale() {
    let snapshot = state_snapshot(
        runtime_snapshot(1, 0),
        vec![issue_snapshot("SYM-1", "SYM-1", "Running", 0)],
    );

    let response = handle_get_with_workspace_envelope(
        STATE_ROUTE,
        &StateSnapshotEnvelope::stale(snapshot, SnapshotErrorView::timeout()),
        None,
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["error"]["code"], "snapshot_timeout");
    assert_eq!(response.body["running"][0]["issue_id"], "SYM-1");
    assert!(response.body.get("snapshot_status").is_none());
}

#[test]
fn http_state_route_exposes_observability_contract_fields() {
    let mut runtime = runtime_snapshot(1, 1);
    runtime.input_tokens = 120;
    runtime.output_tokens = 345;
    runtime.total_tokens = 465;
    runtime.seconds_running = 42.5;
    runtime.rate_limits = Some(json!({
        "primary": {
            "remaining": 17
        }
    }));
    runtime.activity.poll_in_progress = false;
    runtime.activity.last_poll_started_at = Some(1_700_000_010);
    runtime.activity.last_poll_completed_at = Some(1_700_000_015);
    runtime.activity.next_poll_due_at = Some(1_700_000_075);
    runtime.activity.last_runtime_activity_at = Some(1_700_000_060);
    runtime.activity.throughput.window_seconds = 5.0;
    runtime.activity.throughput.input_tokens_per_second = 4.0;
    runtime.activity.throughput.output_tokens_per_second = 3.0;
    runtime.activity.throughput.total_tokens_per_second = 7.0;

    let mut running = issue_snapshot("SYM-7", "SYM-7", "Running", 0);
    running.session_id = Some("thread-7-turn-2".to_owned());
    running.turn_count = 2;
    running.last_event = Some("approval.auto_approved".to_owned());
    running.last_message =
        Some(r#"{"method":"item/tool/requestUserInput","decision":"allow"}"#.to_owned());
    running.started_at = Some(1_700_000_000);
    running.last_event_at = Some(1_700_000_060);
    running.input_tokens = 12;
    running.output_tokens = 34;
    running.total_tokens = 46;

    let mut retrying = issue_snapshot("SYM-8", "SYM-8", "Todo", 3);
    retrying.retry_due_at = Some(1_700_000_120);
    retrying.retry_error = Some("tracker timeout".to_owned());

    let response = handle_get(
        STATE_ROUTE,
        &state_snapshot(runtime, vec![running, retrying]),
    );

    assert_eq!(response.status, 200);
    assert!(response.body["generated_at"].as_str().is_some());
    assert_eq!(response.body["counts"]["running"], 1);
    assert_eq!(response.body["counts"]["retrying"], 1);
    assert_eq!(response.body["codex_totals"]["input_tokens"], 120);
    assert_eq!(response.body["codex_totals"]["output_tokens"], 345);
    assert_eq!(response.body["codex_totals"]["total_tokens"], 465);
    assert_eq!(response.body["codex_totals"]["seconds_running"], 42.5);
    assert_eq!(response.body["rate_limits"]["primary"]["remaining"], 17);
    assert_eq!(response.body["activity"]["poll_in_progress"], false);
    assert!(
        response.body["activity"]["last_poll_started_at"]
            .as_str()
            .is_some()
    );
    assert!(
        response.body["activity"]["last_poll_completed_at"]
            .as_str()
            .is_some()
    );
    assert!(
        response.body["activity"]["next_poll_due_at"]
            .as_str()
            .is_some()
    );
    assert!(
        response.body["activity"]["last_runtime_activity_at"]
            .as_str()
            .is_some()
    );
    assert_eq!(
        response.body["activity"]["throughput"]["window_seconds"],
        5.0
    );
    assert_eq!(
        response.body["activity"]["throughput"]["total_tokens_per_second"],
        7.0
    );
    assert_eq!(response.body["running"][0]["issue_id"], "SYM-7");
    assert_eq!(response.body["running"][0]["session_id"], "thread-7-turn-2");
    assert_eq!(response.body["running"][0]["turn_count"], 2);
    assert_eq!(
        response.body["running"][0]["last_event"],
        "approval.auto_approved"
    );
    assert_eq!(
        response.body["running"][0]["last_message"],
        "tool input request (auto-approved): allow"
    );
    assert_eq!(response.body["running"][0]["tokens"]["total_tokens"], 46);
    assert_eq!(response.body["retrying"][0]["issue_id"], "SYM-8");
    assert_eq!(response.body["retrying"][0]["attempt"], 3);
    assert_eq!(response.body["retrying"][0]["error"], "tracker timeout");
    assert!(response.body["retrying"][0]["due_at"].as_str().is_some());
}

#[test]
fn http_issue_route_exposes_workspace_and_recent_event_contract_fields() {
    let mut running = issue_snapshot("SYM-55", "SYM-55", "Running", 0);
    running.session_id = Some("session-55".to_owned());
    running.turn_count = 4;
    running.last_event = Some("session_started".to_owned());
    running.last_message = Some(r#"{"session_id":"session-55"}"#.to_owned());
    running.started_at = Some(1_700_000_000);
    running.last_event_at = Some(1_700_000_300);
    running.input_tokens = 21;
    running.output_tokens = 34;
    running.total_tokens = 55;

    let root = temp_dir().join("symphony-http-conformance-root");
    let response = handle_get_with_workspace_envelope(
        &issue_route("SYM-55"),
        &StateSnapshotEnvelope::ready(state_snapshot(runtime_snapshot(1, 0), vec![running])),
        Some(root.as_path()),
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["issue_identifier"], "SYM-55");
    assert_eq!(response.body["issue_id"], "SYM-55");
    assert_eq!(response.body["status"], "running");
    assert_eq!(
        response.body["workspace"]["path"],
        root.join("SYM-55").to_string_lossy().as_ref()
    );
    assert_eq!(response.body["attempts"]["restart_count"], 0);
    assert_eq!(response.body["attempts"]["current_retry_attempt"], 0);
    assert_eq!(response.body["running"]["session_id"], "session-55");
    assert_eq!(response.body["running"]["turn_count"], 4);
    assert_eq!(
        response.body["running"]["last_message"],
        "session started (session-55)"
    );
    assert_eq!(response.body["running"]["tokens"]["total_tokens"], 55);
    assert!(response.body["logs"]["codex_session_logs"].is_array());
    assert_eq!(
        response.body["recent_events"][0]["event"],
        "session_started"
    );
    assert_eq!(
        response.body["recent_events"][0]["message"],
        "session started (session-55)"
    );
    assert_eq!(response.body["tracked"], json!({}));
    assert!(response.body["last_error"].is_null());
}
