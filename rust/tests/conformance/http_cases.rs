#![forbid(unsafe_code)]

use std::env::temp_dir;

use serde_json::json;
use symphony_http::{
    DASHBOARD_ROUTE, HttpMethod, STATE_ROUTE, handle_get, handle_get_with_workspace_envelope,
    handle_request, issue_route,
};
use symphony_observability::{SnapshotErrorView, StateSnapshotEnvelope};
use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};
use symphony_workspace::workspace_path;

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
fn http_state_route_preserves_cached_observability_fields_when_stale() {
    let mut runtime = runtime_snapshot(1, 0);
    runtime.input_tokens = 10;
    runtime.output_tokens = 20;
    runtime.total_tokens = 30;
    runtime.rate_limits = Some(json!({
        "primary": {
            "remaining": 9
        }
    }));
    runtime.activity.poll_in_progress = true;
    runtime.activity.last_poll_started_at = Some(1_700_000_010);
    runtime.activity.last_runtime_activity_at = Some(1_700_000_012);
    runtime.activity.throughput.total_tokens_per_second = 2.5;

    let mut running = issue_snapshot("SYM-1", "SYM-1", "Running", 0);
    running.last_event = Some("worker/started".to_owned());
    running.last_message = None;

    let response = handle_get_with_workspace_envelope(
        STATE_ROUTE,
        &StateSnapshotEnvelope::stale(
            state_snapshot(runtime, vec![running]),
            SnapshotErrorView::timeout(),
        ),
        None,
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["error"]["code"], "snapshot_timeout");
    assert!(response.body["generated_at"].as_str().is_some());
    assert_eq!(response.body["codex_totals"]["total_tokens"], 30);
    assert_eq!(response.body["rate_limits"]["primary"]["remaining"], 9);
    assert_eq!(response.body["activity"]["poll_in_progress"], true);
    assert!(
        response.body["activity"]["last_poll_started_at"]
            .as_str()
            .is_some()
    );
    assert!(
        response.body["activity"]["last_runtime_activity_at"]
            .as_str()
            .is_some()
    );
    assert_eq!(
        response.body["activity"]["throughput"]["total_tokens_per_second"],
        2.5
    );
    assert_eq!(
        response.body["running"][0]["last_message"],
        "worker started"
    );
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
    assert_eq!(response.body["health"]["status"], "busy");
    assert_eq!(response.body["health"]["has_running_work"], true);
    assert_eq!(response.body["health"]["has_retry_backlog"], true);
    assert_eq!(response.body["issue_totals"]["total"], 2);
    assert_eq!(response.body["issue_totals"]["active"], 2);
    assert_eq!(response.body["task_maps"]["runtime_running_gap"], 0);
    assert_eq!(response.body["task_maps"]["runtime_retrying_gap"], 0);
    assert_eq!(
        response.body["summary"]["runtime"]["health"],
        "status=busy has_running_work=true has_retry_backlog=true poll_in_progress=false has_rate_limits=true"
    );
    assert_eq!(
        response.body["summary"]["state"]["issues"],
        "total=2 active=2 terminal=0 running=1 retrying=1 completed=0 failed=0 canceled=0 pending=0 unknown=0"
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
fn http_state_route_humanizes_event_only_and_operator_messages() {
    let mut running = issue_snapshot("SYM-10", "SYM-10", "Running", 0);
    running.last_event = Some("worker/started".to_owned());
    running.last_message = None;

    let mut approval = issue_snapshot("SYM-11", "SYM-11", "Running", 0);
    approval.last_event = Some("approval_required".to_owned());
    approval.last_message = Some(r#"{"command":"gh pr view"}"#.to_owned());

    let response = handle_get(
        STATE_ROUTE,
        &state_snapshot(runtime_snapshot(2, 0), vec![running, approval]),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body["running"][0]["last_message"],
        "worker started"
    );
    assert_eq!(
        response.body["running"][1]["last_message"],
        "approval required: gh pr view"
    );
}

#[test]
fn http_state_route_humanizes_protocol_aliases_and_structured_failures() {
    let mut session_new = issue_snapshot("SYM-14", "SYM-14", "Running", 0);
    session_new.last_event = None;
    session_new.last_message = Some(
        r#"{"method":"session/new","thread":{"id":"thread-1"},"turn":{"id":"turn-2"}}"#.to_owned(),
    );

    let mut approval = issue_snapshot("SYM-15", "SYM-15", "Running", 0);
    approval.last_event = None;
    approval.last_message =
        Some(r#"{"method":"turn/approval_required","command":"git push origin"}"#.to_owned());

    let mut notification = issue_snapshot("SYM-16", "SYM-16", "Running", 0);
    notification.last_event = Some("notification".to_owned());
    notification.last_message = Some(r#"{"event":{"approval_required":true}}"#.to_owned());

    let mut tool_failure = issue_snapshot("SYM-17", "SYM-17", "Running", 0);
    tool_failure.last_event = Some("tool_call_failed".to_owned());
    tool_failure.last_message = Some(
        r#"{"name":"linear_graphql","error":"transport_failure","output":{"message":"connection refused"}}"#
            .to_owned(),
    );

    let response = handle_get(
        STATE_ROUTE,
        &state_snapshot(
            runtime_snapshot(4, 0),
            vec![session_new, approval, notification, tool_failure],
        ),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body["running"][0]["last_message"],
        "session started (thread-1/turn-2)"
    );
    assert_eq!(
        response.body["running"][1]["last_message"],
        "approval required: git push origin"
    );
    assert_eq!(
        response.body["running"][2]["last_message"],
        "approval required"
    );
    assert_eq!(
        response.body["running"][3]["last_message"],
        "dynamic tool call failed: linear_graphql (transport failure: connection refused)"
    );
}

#[test]
fn http_operator_surfaces_redact_and_sort_structured_payloads() {
    let mut running = issue_snapshot("SYM-12", "SYM-12", "Running", 0);
    running.last_event = Some("notification".to_owned());
    running.last_message = Some(
        r#"{"zeta":1,"authorization":"Bearer abc","alpha":{"token":"xyz","ok":"yes"}}"#.to_owned(),
    );
    running.last_event_at = Some(1_700_000_060);

    let mut retrying = issue_snapshot("SYM-13", "SYM-13", "Todo", 2);
    retrying.retry_error = Some(r#"{"token":"abc","a":"ok"}"#.to_owned());

    let snapshot = state_snapshot(runtime_snapshot(1, 1), vec![running, retrying]);

    let state_response = handle_get(STATE_ROUTE, &snapshot);
    assert_eq!(
        state_response.body["running"][0]["last_message"],
        r#"{"alpha":{"ok":"yes","token":"[REDACTED]"},"authorization":"[REDACTED]","zeta":1}"#
    );
    assert_eq!(
        state_response.body["retrying"][0]["error"],
        r#"{"a":"ok","token":"[REDACTED]"}"#
    );

    let issue_response = handle_get(&issue_route("SYM-12"), &snapshot);
    assert_eq!(
        issue_response.body["recent_events"][0]["message"],
        r#"{"alpha":{"ok":"yes","token":"[REDACTED]"},"authorization":"[REDACTED]","zeta":1}"#
    );

    let retry_issue_response = handle_get(&issue_route("SYM-13"), &snapshot);
    assert_eq!(
        retry_issue_response.body["last_error"],
        r#"{"a":"ok","token":"[REDACTED]"}"#
    );
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
    let expected_workspace = workspace_path(&root, "SYM-55")
        .expect("workspace path should resolve for conformance root");
    assert_eq!(
        response.body["workspace"]["path"],
        expected_workspace.to_string_lossy().as_ref()
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

#[test]
fn http_state_route_normalizes_negative_zero_runtime_totals() {
    let mut runtime = runtime_snapshot(0, 0);
    runtime.seconds_running = -0.0;
    runtime.activity.throughput.window_seconds = -0.0;
    runtime.activity.throughput.total_tokens_per_second = -0.0;

    let response = handle_get(STATE_ROUTE, &state_snapshot(runtime, vec![]));

    assert_eq!(response.status, 200);
    assert_eq!(response.body["codex_totals"]["seconds_running"], json!(0.0));
    assert_eq!(
        response.body["activity"]["throughput"]["window_seconds"],
        json!(0.0)
    );
    assert_eq!(
        response.body["activity"]["throughput"]["total_tokens_per_second"],
        json!(0.0)
    );
}

#[test]
fn http_dashboard_route_polishes_stale_operator_rendering() {
    let mut running = issue_snapshot("SYM-21", "SYM-21", "Running", 0);
    running.last_event = Some("worker/started".to_owned());
    running.last_message = None;
    running.last_event_at = Some(1_700_000_060);

    let mut retrying = issue_snapshot("SYM-22", "SYM-22", "Todo", 2);
    retrying.retry_due_at = Some(1_700_000_120);
    retrying.retry_error = Some("tracker timeout".to_owned());

    let response = handle_get_with_workspace_envelope(
        DASHBOARD_ROUTE,
        &StateSnapshotEnvelope::stale(
            state_snapshot(runtime_snapshot(1, 1), vec![running, retrying]),
            SnapshotErrorView::timeout(),
        ),
        None,
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "text/html; charset=utf-8");
    assert!(response.rendered_body.contains("Operator Briefing"));
    assert!(response.rendered_body.contains("Issue Status"));
    assert!(response.rendered_body.contains("Snapshot Summary"));
    assert!(
        response
            .rendered_body
            .contains("Serving cached runtime state")
    );
    assert!(response.rendered_body.contains("across 2 tracked issues"));
    assert!(response.rendered_body.contains("Snapshot Health"));
    assert!(response.rendered_body.contains("Runtime Health"));
    assert!(response.rendered_body.contains("Latest Issue Update"));
    assert!(response.rendered_body.contains("Next Retry"));
    assert!(response.rendered_body.contains("Task Map Drift"));
    assert!(response.rendered_body.contains("worker started"));
    assert!(response.rendered_body.contains("tracker timeout"));
    assert!(response.rendered_body.contains("stale"));
}

#[test]
fn http_dashboard_route_polishes_offline_rendering_without_cached_snapshot() {
    let response = handle_get_with_workspace_envelope(
        DASHBOARD_ROUTE,
        &StateSnapshotEnvelope::from_error(SnapshotErrorView::new(
            symphony_observability::SnapshotErrorCode::Unavailable,
            r#"{"token":"abc","message":"backend unavailable"}"#,
        )),
        None,
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "text/html; charset=utf-8");
    assert!(response.rendered_body.contains("offline"));
    assert!(
        response
            .rendered_body
            .contains("No cached snapshot is available yet")
    );
    assert!(response.rendered_body.contains("Issue Status"));
    assert!(response.rendered_body.contains("Snapshot Summary"));
    assert!(response.rendered_body.contains("Runtime Health"));
    assert!(response.rendered_body.contains("Task Map Drift"));
    assert!(response.rendered_body.contains("Response generated"));
    assert!(response.rendered_body.contains("backend unavailable"));
    assert!(response.rendered_body.contains("[REDACTED]"));
    assert!(
        response
            .rendered_body
            .contains("tracker totals are unavailable")
    );
    assert!(response.rendered_body.contains(">n/a<"));
    assert!(!response.rendered_body.contains("Snapshot generated"));
}

#[test]
fn http_dashboard_route_renders_issue_status_and_snapshot_summary_from_live_state() {
    let mut runtime = runtime_snapshot(1, 1);
    runtime.input_tokens = 40;
    runtime.output_tokens = 20;
    runtime.total_tokens = 60;
    runtime.activity.last_poll_completed_at = Some(1_700_000_010);
    runtime.activity.next_poll_due_at = Some(1_700_000_040);

    let mut running = issue_snapshot("SYM-31", "SYM-31", "Running", 0);
    running.last_event = Some("notification".to_owned());
    running.last_message = Some(r#"{"message":"syncing tracker state"}"#.to_owned());

    let mut retrying = issue_snapshot("SYM-32", "SYM-32", "Todo", 2);
    retrying.retry_error = Some("response_timeout".to_owned());

    let mut completed = issue_snapshot("SYM-33", "SYM-33", "Completed", 0);
    completed.last_message = Some("done".to_owned());

    let response = handle_get_with_workspace_envelope(
        DASHBOARD_ROUTE,
        &StateSnapshotEnvelope::ready(state_snapshot(runtime, vec![running, retrying, completed])),
        None,
    );

    assert_eq!(response.status, 200);
    assert!(response.rendered_body.contains("Issue Status"));
    assert!(response.rendered_body.contains("Snapshot Summary"));
    assert!(response.rendered_body.contains("Tracked"));
    assert!(response.rendered_body.contains("Terminal"));
    assert!(
        response
            .rendered_body
            .contains("running=1 retrying=1 total_active=2")
    );
    assert!(
        response
            .rendered_body
            .contains("total=3 active=2 terminal=1 running=1 retrying=1 completed=1")
    );
    assert!(
        response
            .rendered_body
            .contains("running_identifiers=SYM-31")
    );
    assert!(
        response
            .rendered_body
            .contains("retrying_identifiers=SYM-32")
    );
}

#[test]
fn http_dashboard_route_renders_live_activity_and_rate_limit_panels() {
    let mut runtime = runtime_snapshot(1, 0);
    runtime.input_tokens = 70;
    runtime.output_tokens = 40;
    runtime.total_tokens = 110;
    runtime.rate_limits = Some(json!({
        "authorization": "Bearer abc",
        "nested": {
            "token": "xyz"
        },
        "primary": {
            "remaining": 17
        }
    }));
    runtime.activity.poll_in_progress = false;
    runtime.activity.last_poll_completed_at = Some(1_700_000_010);
    runtime.activity.next_poll_due_at = Some(1_700_000_040);
    runtime.activity.last_runtime_activity_at = Some(1_700_000_020);
    runtime.activity.throughput.window_seconds = 5.0;
    runtime.activity.throughput.input_tokens_per_second = 7.0;
    runtime.activity.throughput.output_tokens_per_second = 4.0;
    runtime.activity.throughput.total_tokens_per_second = 11.0;

    let mut running = issue_snapshot("SYM-34", "SYM-34", "Running", 0);
    running.last_event = Some("worker/started".to_owned());
    running.last_message = None;

    let response = handle_get_with_workspace_envelope(
        DASHBOARD_ROUTE,
        &StateSnapshotEnvelope::ready(state_snapshot(runtime, vec![running])),
        None,
    );

    assert_eq!(response.status, 200);
    assert!(response.rendered_body.contains("Metrics"));
    assert!(response.rendered_body.contains("Poll Status"));
    assert!(response.rendered_body.contains("Last Activity"));
    assert!(response.rendered_body.contains("Throughput"));
    assert!(response.rendered_body.contains("Rate limits"));
    assert!(response.rendered_body.contains("idle"));
    assert!(response.rendered_body.contains("last completed"));
    assert!(response.rendered_body.contains("next poll"));
    assert!(response.rendered_body.contains("11.0 tps"));
    assert!(response.rendered_body.contains("5.0s window"));
    assert!(response.rendered_body.contains("remaining"));
    assert!(response.rendered_body.contains("[REDACTED]"));
    assert!(response.rendered_body.contains("worker started"));
}

#[test]
fn http_method_not_allowed_returns_405() {
    let snapshot = state_snapshot(runtime_snapshot(0, 0), vec![]);

    for method in [
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ] {
        let response = handle_request(method, STATE_ROUTE, &snapshot);
        assert_eq!(
            response.status, 405,
            "{method:?} to STATE_ROUTE should return 405"
        );
        assert_eq!(response.body["error"]["code"], "method_not_allowed");
        assert_eq!(response.body["error"]["message"], "Method not allowed");
    }
}

#[test]
fn http_api_error_envelope_uses_correct_schema() {
    let snapshot = state_snapshot(runtime_snapshot(0, 0), vec![]);

    let not_found = handle_get(&issue_route("SYM-999"), &snapshot);
    assert_eq!(not_found.status, 404);
    assert!(
        not_found.body.get("error").is_some(),
        "error envelope must be present"
    );
    assert!(
        not_found.body["error"].is_object(),
        "error must be an object"
    );
    assert!(
        not_found.body["error"].get("code").is_some(),
        "error.code must be present"
    );
    assert!(
        not_found.body["error"].get("message").is_some(),
        "error.message must be present"
    );
    assert!(
        not_found.body["error"]["code"].is_string(),
        "error.code must be a string"
    );
    assert!(
        not_found.body["error"]["message"].is_string(),
        "error.message must be a string"
    );
    let error_keys: Vec<&str> = not_found.body["error"]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        error_keys.len(),
        2,
        "error envelope must contain exactly code and message, got: {error_keys:?}"
    );

    // Route that doesn't match any pattern should return not_found
    let route_not_found = handle_get("/api/v2/nonexistent", &snapshot);
    assert_eq!(route_not_found.status, 404);
    assert_eq!(route_not_found.body["error"]["code"], "not_found");
    assert!(route_not_found.body["error"]["message"].is_string());

    let method_err = handle_request(HttpMethod::Post, STATE_ROUTE, &snapshot);
    assert_eq!(method_err.status, 405);
    assert_eq!(method_err.body["error"]["code"], "method_not_allowed");
    assert!(method_err.body["error"]["message"].is_string());
    let method_err_keys: Vec<&str> = method_err.body["error"]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        method_err_keys.len(),
        2,
        "405 error envelope must contain exactly code and message"
    );
}

// =============================================================================
// C5.2.1: Optional HTTP and linear_graphql extension coverage
// =============================================================================

#[test]
fn http_state_route_handles_linear_graphql_tool_call_humanization() {
    let mut running = issue_snapshot("SYM-GQL1", "SYM-GQL1", "Running", 0);
    running.last_event = Some("tool_call".to_owned());
    running.last_message = Some(
        r#"{"method":"tool/call","name":"linear_graphql","arguments":{"query":"{ issue(id: \"123\") { title } }"}}"#
            .to_owned(),
    );

    let response = handle_get(
        STATE_ROUTE,
        &state_snapshot(runtime_snapshot(1, 0), vec![running]),
    );

    assert_eq!(response.status, 200);
    assert!(
        response.body["running"][0]["last_message"]
            .as_str()
            .unwrap()
            .contains("linear_graphql"),
        "linear_graphql tool call should be humanized with tool name"
    );
}

#[test]
fn http_issue_route_surfaces_linear_graphql_tool_call_errors() {
    let mut running = issue_snapshot("SYM-GQL2", "SYM-GQL2", "Running", 0);
    running.last_event = Some("tool_call_failed".to_owned());
    running.last_message = Some(
        r#"{"name":"linear_graphql","error":"rate_limit_exceeded","output":{"message":"API rate limit exceeded"}}"#
            .to_owned(),
    );

    let response = handle_get(
        &issue_route("SYM-GQL2"),
        &state_snapshot(runtime_snapshot(1, 0), vec![running]),
    );

    assert_eq!(response.status, 200);
    let last_message = response.body["running"]["last_message"]
        .as_str()
        .expect("should have last_message");
    assert!(
        last_message.contains("linear_graphql"),
        "error message should include tool name"
    );
    assert!(
        last_message.contains("rate"),
        "error message should include error type"
    );
}

#[test]
fn http_state_route_redacts_linear_graphql_auth_headers() {
    let mut running = issue_snapshot("SYM-GQL3", "SYM-GQL3", "Running", 0);
    running.last_event = Some("notification".to_owned());
    running.last_message = Some(
        r#"{"tool":"linear_graphql","authorization":"Bearer lin_api_secret123","query":"query { viewer { name } }"}"#
            .to_owned(),
    );

    let response = handle_get(
        STATE_ROUTE,
        &state_snapshot(runtime_snapshot(1, 0), vec![running]),
    );

    assert_eq!(response.status, 200);
    let message = response.body["running"][0]["last_message"]
        .as_str()
        .expect("should have message");
    assert!(
        !message.contains("lin_api_secret123"),
        "Linear API key should be redacted"
    );
    assert!(
        message.contains("[REDACTED]"),
        "redacted placeholder should appear"
    );
}

#[test]
fn http_extension_handles_missing_optional_workspace_root_gracefully() {
    let mut running = issue_snapshot("SYM-EXT1", "SYM-EXT1", "Running", 0);
    running.session_id = Some("session-ext1".to_owned());

    // Call with None workspace root (optional extension behavior)
    let response = handle_get_with_workspace_envelope(
        &issue_route("SYM-EXT1"),
        &StateSnapshotEnvelope::ready(state_snapshot(runtime_snapshot(1, 0), vec![running])),
        None,
    );

    assert_eq!(response.status, 200);
    // Workspace path may be null or absent when no root is provided
    assert!(
        response.body["workspace"]["path"].is_null()
            || response.body.get("workspace").is_none()
            || response.body["workspace"].as_object().unwrap().is_empty(),
        "workspace path should be null or absent when no root provided"
    );
}

#[test]
fn http_state_route_validates_json_content_type_header() {
    let snapshot = state_snapshot(runtime_snapshot(0, 0), vec![]);
    let response = handle_get(STATE_ROUTE, &snapshot);

    assert_eq!(response.status, 200);
    assert!(
        response.content_type.starts_with("application/json"),
        "state route should return JSON content type, got: {}",
        response.content_type
    );
}

#[test]
fn http_dashboard_route_validates_html_content_type_header() {
    let response = handle_get_with_workspace_envelope(
        DASHBOARD_ROUTE,
        &StateSnapshotEnvelope::ready(state_snapshot(runtime_snapshot(0, 0), vec![])),
        None,
    );

    assert_eq!(response.status, 200);
    assert!(
        response.content_type.starts_with("text/html"),
        "dashboard route should return HTML content type, got: {}",
        response.content_type
    );
}
