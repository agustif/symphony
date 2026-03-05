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

    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::error(405, "method_not_allowed", false, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::error(404, "route_not_found", false, message)
    }

    pub fn issue_not_found(issue_identifier: &str) -> Self {
        Self::error(
            404,
            "issue_not_found",
            false,
            format!("issue `{issue_identifier}` not found"),
        )
    }

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::error(400, "parse_error", false, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::error(503, "service_unavailable", true, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::error(504, "upstream_timeout", true, message)
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

    fn error(status: u16, code: &str, retryable: bool, message: impl Into<String>) -> Self {
        let message = message.into();
        Self::json(
            status,
            serde_json::json!({
                "error": message,
                "error_envelope": {
                    "status": status,
                    "code": code,
                    "message": message,
                    "retryable": retryable,
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
        assert_eq!(response.body["error"], "upstream timed out");
        assert_eq!(response.body["error_envelope"]["status"], 504);
        assert_eq!(response.body["error_envelope"]["code"], "upstream_timeout");
        assert_eq!(
            response.body["error_envelope"]["message"],
            "upstream timed out"
        );
        assert_eq!(response.body["error_envelope"]["retryable"], true);
    }

    #[test]
    fn parse_and_unavailable_errors_use_standard_envelope_shape() {
        let parse = ApiResponse::parse_error("malformed request body");
        assert_eq!(parse.status, 400);
        assert_eq!(parse.body["error_envelope"]["status"], 400);
        assert_eq!(parse.body["error_envelope"]["code"], "parse_error");
        assert_eq!(parse.body["error_envelope"]["retryable"], false);

        let unavailable = ApiResponse::unavailable("runtime temporarily unavailable");
        assert_eq!(unavailable.status, 503);
        assert_eq!(unavailable.body["error_envelope"]["status"], 503);
        assert_eq!(
            unavailable.body["error_envelope"]["code"],
            "service_unavailable"
        );
        assert_eq!(unavailable.body["error_envelope"]["retryable"], true);
    }
}
