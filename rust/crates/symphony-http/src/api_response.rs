#[derive(Clone, Debug, PartialEq)]
pub struct ApiResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

impl ApiResponse {
    pub fn ok(body: serde_json::Value) -> Self {
        Self { status: 200, body }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            body: serde_json::json!({
                "error": message.into(),
            }),
        }
    }
}
