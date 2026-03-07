#![forbid(unsafe_code)]

mod api_response;
mod http_method;
mod payloads;
mod routes;
mod state_handlers;

pub use api_response::ApiResponse;
pub use http_method::HttpMethod;
pub use routes::{
    API_V1_PREFIX, DASHBOARD_ROUTE, ISSUE_ROUTE_PREFIX, REFRESH_ROUTE, STATE_ROUTE, issue_route,
    legacy_issue_route,
};
pub use state_handlers::{
    dashboard_envelope_to_html, dashboard_to_html, handle_get, handle_get_with_workspace,
    handle_get_with_workspace_envelope, handle_post, handle_request, handle_request_with_workspace,
    handle_request_with_workspace_envelope, issue_detail_to_json,
    issue_detail_to_json_with_workspace, issue_to_json, refresh_to_json,
    refresh_to_json_with_coalesced, snapshot_to_json, state_envelope_to_json, state_to_json,
};

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_observability::{
        RuntimeActivitySnapshot, RuntimeSnapshot, SnapshotErrorView, StateSnapshot,
        StateSnapshotEnvelope, ThroughputSnapshot,
    };
    use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};
    use symphony_workspace::workspace_path;

    #[test]
    fn serves_dashboard_route_with_html_document() {
        let mut runtime = runtime_snapshot(1, 1);
        runtime.input_tokens = 1_200;
        runtime.output_tokens = 11_145;
        runtime.total_tokens = 12_345;
        runtime.seconds_running = 90.0;
        runtime.rate_limits = Some(serde_json::json!({
            "primary": {
                "remaining": 11
            }
        }));
        runtime.activity = RuntimeActivitySnapshot {
            poll_in_progress: true,
            last_poll_started_at: Some(1_700_000_000),
            last_poll_completed_at: Some(1_699_999_970),
            next_poll_due_at: Some(1_700_000_030),
            last_runtime_activity_at: Some(1_700_000_001),
            throughput: ThroughputSnapshot {
                window_seconds: 5.0,
                input_tokens_per_second: 1.2,
                output_tokens_per_second: 2.4,
                total_tokens_per_second: 3.6,
            },
        };

        let mut running = issue_snapshot("SYM-1", "SYM-1", "Running", 0);
        running.session_id = Some("session-123".to_owned());
        running.turn_count = 3;
        running.last_event = Some("tool_call_completed".to_owned());
        running.last_message = Some(r#"{"name":"apply_patch"}"#.to_owned());

        let mut retrying = issue_snapshot("SYM-2", "SYM-2<script>", "Retrying", 2);
        retrying.retry_error = Some("response_timeout".to_owned());

        let snapshot = state_snapshot(runtime, vec![running, retrying]);

        let response = handle_get(DASHBOARD_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(response.rendered_body.contains("<!doctype html>"));
        assert!(response.rendered_body.contains("Symphony Dashboard"));
        assert!(response.rendered_body.contains("Metrics"));
        assert!(response.rendered_body.contains("Rate limits"));
        assert!(response.rendered_body.contains("Running sessions"));
        assert!(response.rendered_body.contains("Retry queue"));
        assert!(response.rendered_body.contains("Poll Status"));
        assert!(response.rendered_body.contains("checking now"));
        assert!(response.rendered_body.contains("Throughput"));
        assert!(response.rendered_body.contains("3.6 tps"));
        assert!(response.rendered_body.contains("12,345"));
        assert!(response.rendered_body.contains("1m 30s"));
        assert!(
            response
                .rendered_body
                .contains("dynamic tool call completed: apply_patch")
        );
        assert!(response.rendered_body.contains("SYM-2&lt;script&gt;"));
    }

    #[test]
    fn serves_state_endpoint_with_elixir_surface_fields() {
        let snapshot = state_snapshot(
            runtime_snapshot(2, 1),
            vec![
                issue_snapshot("SYM-7", "SYM-7", "Running", 0),
                issue_snapshot("SYM-8", "SYM-8", "Retrying", 2),
            ],
        );

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["counts"]["running"], 2);
        assert_eq!(response.body["running"][0]["issue_identifier"], "SYM-7");
        assert_eq!(response.body["retrying"][0]["attempt"], 2);
        assert!(response.body["running"][0]["last_message"].is_null());
        assert!(response.body.get("runtime").is_none());
        assert!(response.body.get("issues").is_none());
    }

    #[test]
    fn serves_state_endpoint_with_codex_totals() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 1,
                retrying: 0,
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                seconds_running: 42.5,
                rate_limits: Some(serde_json::json!({
                    "primary": {
                        "remaining": 11
                    }
                })),
                activity: symphony_observability::RuntimeActivitySnapshot::default(),
            },
            issues: vec![issue_snapshot("SYM-9", "SYM-9", "Running", 0)],
        };

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["codex_totals"]["input_tokens"], 100);
        assert_eq!(response.body["codex_totals"]["output_tokens"], 50);
        assert_eq!(response.body["codex_totals"]["total_tokens"], 150);
        assert_eq!(response.body["codex_totals"]["seconds_running"], 42.5);
        assert_eq!(response.body["rate_limits"]["primary"]["remaining"], 11);
        assert_eq!(response.body["activity"]["poll_in_progress"], false);
        assert!(response.body["activity"]["throughput"]["total_tokens_per_second"].is_number());
    }

    #[test]
    fn serves_issue_endpoint_for_spec_route() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Retrying", 3)],
        );

        let response = handle_get(&issue_route("SYM-1"), &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue_identifier"], "SYM-1");
        assert_eq!(response.body["attempts"]["current_retry_attempt"], 3);
        assert_eq!(response.body["attempts"]["restart_count"], 2);
        assert_eq!(response.body["retry"]["attempt"], 3);
        assert!(response.body["retry"].get("issue_id").is_none());
        assert!(response.body.get("issue").is_none());
    }

    #[test]
    fn serves_issue_endpoint_for_legacy_route() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Running", 0)],
        );

        let response = handle_get(&legacy_issue_route("SYM-1"), &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue_id"], "SYM-1");
        assert!(response.body["running"].get("issue_id").is_none());
        assert!(response.body["running"]["last_message"].is_null());
        assert!(response.body.get("issue").is_none());
    }

    #[test]
    fn issue_routes_round_trip_percent_encoded_identifiers() {
        let identifier = "SYM/1 ?#%";
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot(identifier, identifier, "Running", 0)],
        );

        let response = handle_get(&issue_route(identifier), &snapshot);

        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue_id"], identifier);
        assert_eq!(response.body["issue_identifier"], identifier);
    }

    #[test]
    fn dashboard_links_percent_encode_issue_identifiers() {
        let identifier = "SYM/1 ?#%";
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot(identifier, identifier, "Running", 0)],
        );

        let html = dashboard_to_html(&snapshot);

        assert!(html.contains(&issue_route(identifier)));
    }

    #[test]
    fn issue_detail_workspace_fallback_uses_sanitized_workspace_path() {
        let identifier = "SYM/1";
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot(identifier, identifier, "Running", 0)],
        );
        let root = std::env::temp_dir().join("symphony-http-tests");

        let response = handle_get_with_workspace(&issue_route(identifier), &snapshot, Some(&root));
        let expected = workspace_path(&root, identifier)
            .expect("workspace path should sanitize identifier")
            .to_string_lossy()
            .into_owned();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["workspace"]["path"], expected);
    }

    #[test]
    fn refresh_endpoint_accepts_post() {
        let response = handle_post(
            REFRESH_ROUTE,
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 202);
        assert_eq!(response.body["queued"], true);
        assert_eq!(response.body["operations"][0], "poll");
    }

    #[test]
    fn returns_method_not_allowed_for_defined_route_with_wrong_method() {
        let response = handle_request(
            HttpMethod::Post,
            STATE_ROUTE,
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 405);
        assert_eq!(response.body["error"]["code"], "method_not_allowed");
    }

    #[test]
    fn returns_method_not_allowed_for_dashboard_post() {
        let response = handle_request(
            HttpMethod::Post,
            DASHBOARD_ROUTE,
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 405);
        assert_eq!(response.body["error"]["code"], "method_not_allowed");
        assert_eq!(response.body["error"]["message"], "Method not allowed");
    }

    #[test]
    fn returns_not_found_for_unknown_routes() {
        let response = handle_get(
            "/api/v1/unknown/route",
            &state_snapshot(runtime_snapshot(0, 0), vec![]),
        );
        assert_eq!(response.status, 404);
        assert_eq!(response.body["error"]["code"], "not_found");
        assert_eq!(response.body["error"]["message"], "Route not found");
    }

    #[test]
    fn returns_consistent_error_envelope_for_missing_issue() {
        let response = handle_get(
            &issue_route("SYM-404"),
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 404);
        assert_eq!(response.body["error"]["code"], "issue_not_found");
        assert_eq!(response.body["error"]["message"], "Issue not found");
    }

    #[test]
    fn preserves_null_last_message_in_state_view() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-13", "SYM-13", "Running", 0)],
        );

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert!(response.body["running"][0]["last_message"].is_null());
    }

    #[test]
    fn humanizes_high_signal_codex_messages_in_state_and_issue_views() {
        let mut issue = issue_snapshot("SYM-17", "SYM-17", "Running", 0);
        issue.last_event = Some("approval_auto_approved".to_owned());
        issue.last_message =
            Some(r#"{"method":"item/tool/requestUserInput","decision":"allow"}"#.to_owned());
        issue.last_event_at = Some(1_700_000_000);

        let snapshot = state_snapshot(runtime_snapshot(1, 0), vec![issue]);

        let state_response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(
            state_response.body["running"][0]["last_message"],
            "tool input request (auto-approved): allow"
        );

        let issue_response = handle_get(&issue_route("SYM-17"), &snapshot);
        assert_eq!(
            issue_response.body["recent_events"][0]["message"],
            "tool input request (auto-approved): allow"
        );
    }

    #[test]
    fn serves_state_timeout_error_with_elixir_contract_shape() {
        let response = handle_get_with_workspace_envelope(
            STATE_ROUTE,
            &StateSnapshotEnvelope::timeout(),
            None,
        );

        assert_eq!(response.status, 200);
        assert_eq!(response.body["error"]["code"], "snapshot_timeout");
        assert_eq!(response.body["error"]["message"], "Snapshot timed out");
        assert!(response.body["generated_at"].is_string());
    }

    #[test]
    fn serves_stale_state_with_error_marker_and_last_good_snapshot() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 1),
            vec![
                issue_snapshot("SYM-1", "SYM-1", "Running", 0),
                issue_snapshot("SYM-2", "SYM-2", "Retrying", 2),
            ],
        );

        let response = handle_get_with_workspace_envelope(
            STATE_ROUTE,
            &StateSnapshotEnvelope::stale(snapshot, SnapshotErrorView::timeout()),
            None,
        );

        assert_eq!(response.status, 200);
        assert_eq!(response.body["counts"]["running"], 1);
        assert_eq!(response.body["error"]["code"], "snapshot_timeout");
        assert!(response.body.get("snapshot_status").is_none());
    }

    #[test]
    fn renders_degraded_dashboard_without_crashing() {
        let response = handle_get_with_workspace_envelope(
            DASHBOARD_ROUTE,
            &StateSnapshotEnvelope::unavailable(),
            None,
        );

        assert_eq!(response.status, 200);
        assert!(response.rendered_body.contains("Snapshot unavailable"));
        assert!(response.rendered_body.contains("snapshot_unavailable"));
        assert!(response.rendered_body.contains("offline"));
        assert!(
            response
                .rendered_body
                .contains("tracker totals are unavailable")
        );
        assert!(response.rendered_body.contains(">n/a<"));
    }

    #[test]
    fn issue_route_uses_stale_snapshot_when_available() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-42", "SYM-42", "Running", 0)],
        );

        let response = handle_get_with_workspace_envelope(
            &issue_route("SYM-42"),
            &StateSnapshotEnvelope::stale(snapshot, SnapshotErrorView::timeout()),
            None,
        );

        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue_identifier"], "SYM-42");
    }
}
