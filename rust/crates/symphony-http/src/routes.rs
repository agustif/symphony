use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, percent_encode};

pub const DASHBOARD_ROUTE: &str = "/";
pub const API_V1_PREFIX: &str = "/api/v1";
pub const STATE_ROUTE: &str = "/api/v1/state";
pub const REFRESH_ROUTE: &str = "/api/v1/refresh";
pub const ISSUE_ROUTE_PREFIX: &str = "/api/v1/";

pub fn issue_route(issue_id: &str) -> String {
    format!("{ISSUE_ROUTE_PREFIX}{}", encode_issue_identifier(issue_id))
}

pub(crate) fn encode_issue_identifier(issue_id: &str) -> String {
    percent_encode(issue_id.as_bytes(), NON_ALPHANUMERIC).to_string()
}

pub(crate) fn decode_issue_identifier(issue_identifier: &str) -> Option<String> {
    percent_decode_str(issue_identifier)
        .decode_utf8()
        .ok()
        .map(|decoded| decoded.into_owned())
}
