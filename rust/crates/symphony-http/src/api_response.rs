#[derive(Clone, Debug, PartialEq)]
pub struct ApiResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

impl ApiResponse {
    pub fn ok(body: serde_json::Value) -> Self {
        Self { status: 200, body }
    }

    pub fn accepted(body: serde_json::Value) -> Self {
        Self { status: 202, body }
    }

    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::error(405, "method_not_allowed", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::error(404, "route_not_found", message)
    }

    pub fn issue_not_found(issue_identifier: &str) -> Self {
        Self::error(
            404,
            "issue_not_found",
            format!("issue `{issue_identifier}` not found"),
        )
    }

    fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status,
            body: serde_json::json!({
                "error": message,
                "error_envelope": {
                    "code": code,
                    "message": message,
                }
            }),
        }
    }
}
