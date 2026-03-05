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
    dashboard_to_html, handle_get, handle_post, handle_request, issue_detail_to_json,
    issue_to_json, refresh_to_json, snapshot_to_json, state_to_json,
};

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_observability::{RuntimeSnapshot, StateSnapshot};
    use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};

    #[test]
    fn serves_dashboard_route_with_html_document() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 1),
            vec![
                issue_snapshot("SYM-1", "SYM-1", "Running", 0),
                issue_snapshot("SYM-2", "SYM-2<script>", "Retrying", 2),
            ],
        );

        let response = handle_get(DASHBOARD_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(response.rendered_body.contains("<!doctype html>"));
        assert!(response.rendered_body.contains("Symphony Dashboard"));
        assert!(response.rendered_body.contains("SYM-2&lt;script&gt;"));
    }

    #[test]
    fn serves_state_endpoint_with_legacy_and_spec_fields() {
        let snapshot = state_snapshot(
            runtime_snapshot(2, 1),
            vec![
                issue_snapshot("SYM-7", "SYM-7", "Running", 0),
                issue_snapshot("SYM-8", "SYM-8", "Retrying", 2),
            ],
        );

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["runtime"]["running"], 2);
        assert_eq!(response.body["counts"]["running"], 2);
        assert_eq!(response.body["running"][0]["issue_identifier"], "SYM-7");
        assert_eq!(response.body["retrying"][0]["attempt"], 2);
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
            },
            issues: vec![issue_snapshot("SYM-9", "SYM-9", "Running", 0)],
        };

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["codex_totals"]["input_tokens"], 100);
        assert_eq!(response.body["codex_totals"]["output_tokens"], 50);
        assert_eq!(response.body["codex_totals"]["total_tokens"], 150);
    }

    #[test]
    fn serves_issue_endpoint_for_spec_route() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Retrying", 3)],
        );

        let response = handle_get(&issue_route("SYM-1"), &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue"]["retry_attempts"], 3);
        assert_eq!(response.body["issue_identifier"], "SYM-1");
    }

    #[test]
    fn serves_issue_endpoint_for_legacy_route() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Running", 0)],
        );

        let response = handle_get(&legacy_issue_route("SYM-1"), &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue"]["id"], "SYM-1");
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
        assert_eq!(
            response.body["error_envelope"]["code"],
            "method_not_allowed"
        );
        assert_eq!(response.body["error_envelope"]["status"], 405);
        assert_eq!(response.body["error_envelope"]["retryable"], false);
    }

    #[test]
    fn returns_method_not_allowed_for_dashboard_post() {
        let response = handle_request(
            HttpMethod::Post,
            DASHBOARD_ROUTE,
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 405);
        assert_eq!(
            response.body["error_envelope"]["code"],
            "method_not_allowed"
        );
        assert_eq!(
            response.body["error_envelope"]["message"],
            "method not allowed for `/`"
        );
    }

    #[test]
    fn returns_not_found_for_unknown_routes() {
        let response = handle_get(
            "/api/v1/unknown/route",
            &state_snapshot(runtime_snapshot(0, 0), vec![]),
        );
        assert_eq!(response.status, 404);
        assert_eq!(response.body["error"], "route not found");
        assert_eq!(response.body["error_envelope"]["status"], 404);
        assert_eq!(response.body["error_envelope"]["code"], "route_not_found");
        assert_eq!(
            response.body["error_envelope"]["message"],
            "route not found"
        );
        assert_eq!(response.body["error_envelope"]["retryable"], false);
    }

    #[test]
    fn returns_consistent_error_envelope_for_missing_issue() {
        let response = handle_get(
            &issue_route("SYM-404"),
            &state_snapshot(runtime_snapshot(0, 0), Vec::new()),
        );
        assert_eq!(response.status, 404);
        assert_eq!(response.body["error"], "issue `SYM-404` not found");
        assert_eq!(response.body["error_envelope"]["status"], 404);
        assert_eq!(response.body["error_envelope"]["code"], "issue_not_found");
        assert_eq!(
            response.body["error_envelope"]["message"],
            "issue `SYM-404` not found"
        );
        assert_eq!(response.body["error_envelope"]["retryable"], false);
    }
}
