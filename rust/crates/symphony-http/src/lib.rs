#![forbid(unsafe_code)]

mod api_response;
mod routes;
mod state_handlers;

pub use api_response::ApiResponse;
pub use routes::{ISSUE_ROUTE_PREFIX, STATE_ROUTE, issue_route};
pub use state_handlers::{handle_get, issue_to_json, snapshot_to_json, state_to_json};

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_testkit::{issue_snapshot, runtime_snapshot, state_snapshot};

    #[test]
    fn serves_state_endpoint() {
        let snapshot = state_snapshot(
            runtime_snapshot(2, 1),
            vec![issue_snapshot("SYM-7", "SYM-7", "Running", 0)],
        );

        let response = handle_get(STATE_ROUTE, &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["runtime"]["running"], 2);
        assert_eq!(response.body["issues"][0]["id"], "SYM-7");
    }

    #[test]
    fn serves_issue_endpoint() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-1", "SYM-1", "Retrying", 3)],
        );

        let response = handle_get(&issue_route("SYM-1"), &snapshot);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["issue"]["retry_attempts"], 3);
    }

    #[test]
    fn returns_not_found_for_unknown_routes() {
        let response = handle_get(
            "/api/v1/unknown",
            &state_snapshot(runtime_snapshot(0, 0), vec![]),
        );
        assert_eq!(response.status, 404);
    }
}
