#[derive(Clone, Debug, PartialEq)]
pub struct ApiResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: serde_json::Value,
    pub rendered_body: String,
}

impl ApiResponse {
    pub fn ok(body: serde_json::Value) -> Self {
        Self::json(200, body)
    }

    pub fn accepted(body: serde_json::Value) -> Self {
        Self::json(202, body)
    }

    pub fn html(document: impl Into<String>) -> Self {
        let document = document.into();
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: serde_json::json!({
                "dashboard_html": document,
            }),
            rendered_body: document,
        }
    }

    pub fn method_not_allowed() -> Self {
        Self::error(405, "method_not_allowed", "Method not allowed")
    }

    pub fn not_found() -> Self {
        Self::error(404, "not_found", "Route not found")
    }

    pub fn issue_not_found() -> Self {
        Self::error(404, "issue_not_found", "Issue not found")
    }

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::error(400, "parse_error", message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::error(503, "service_unavailable", message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::error(504, "upstream_timeout", message)
    }

    pub fn orchestrator_unavailable() -> Self {
        Self::error(
            503,
            "orchestrator_unavailable",
            "Orchestrator is unavailable",
        )
    }

    pub fn snapshot_error(
        generated_at: impl Into<String>,
        code: &str,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self::json(
            200,
            serde_json::json!({
                "generated_at": generated_at.into(),
                "error": {
                    "code": code,
                    "message": message,
                }
            }),
        )
    }

    fn json(status: u16, body: serde_json::Value) -> Self {
        let rendered_body = serde_json::to_string(&body).unwrap_or_else(|_error| "{}".to_owned());
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body,
            rendered_body,
        }
    }

    fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        Self::json(
            status,
            serde_json::json!({
                "error": {
                    "code": code,
                    "message": message,
                }
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ApiResponse;

    #[test]
    fn html_response_uses_text_html_content_type() {
        let response = ApiResponse::html("<main>dashboard</main>");
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert_eq!(response.rendered_body, "<main>dashboard</main>");
    }

    #[test]
    fn timeout_error_sets_retryable_envelope() {
        let response = ApiResponse::timeout("upstream timed out");
        assert_eq!(response.status, 504);
        assert_eq!(response.body["error"]["code"], "upstream_timeout");
        assert_eq!(response.body["error"]["message"], "upstream timed out");
    }

    #[test]
    fn parse_and_unavailable_errors_use_standard_envelope_shape() {
        let parse = ApiResponse::parse_error("malformed request body");
        assert_eq!(parse.status, 400);
        assert_eq!(parse.body["error"]["code"], "parse_error");

        let unavailable = ApiResponse::unavailable("runtime temporarily unavailable");
        assert_eq!(unavailable.status, 503);
        assert_eq!(unavailable.body["error"]["code"], "service_unavailable");
    }

    #[test]
    fn orchestrator_unavailable_uses_expected_code() {
        let response = ApiResponse::orchestrator_unavailable();
        assert_eq!(response.status, 503);
        assert_eq!(response.body["error"]["code"], "orchestrator_unavailable");
        assert_eq!(
            response.body["error"]["message"],
            "Orchestrator is unavailable"
        );
    }

    #[test]
    fn generic_errors_match_elixir_contract() {
        let method_not_allowed = ApiResponse::method_not_allowed();
        assert_eq!(method_not_allowed.status, 405);
        assert_eq!(
            method_not_allowed.body["error"]["message"],
            "Method not allowed"
        );

        let not_found = ApiResponse::not_found();
        assert_eq!(not_found.status, 404);
        assert_eq!(not_found.body["error"]["code"], "not_found");
        assert_eq!(not_found.body["error"]["message"], "Route not found");

        let issue_not_found = ApiResponse::issue_not_found();
        assert_eq!(issue_not_found.status, 404);
        assert_eq!(issue_not_found.body["error"]["code"], "issue_not_found");
        assert_eq!(issue_not_found.body["error"]["message"], "Issue not found");
    }
}
