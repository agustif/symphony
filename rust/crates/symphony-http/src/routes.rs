pub const DASHBOARD_ROUTE: &str = "/";
pub const API_V1_PREFIX: &str = "/api/v1";
pub const STATE_ROUTE: &str = "/api/v1/state";
pub const REFRESH_ROUTE: &str = "/api/v1/refresh";
pub const ISSUE_ROUTE_PREFIX: &str = "/api/v1/issues/";

pub fn issue_route(issue_id: &str) -> String {
    format!("{API_V1_PREFIX}/{issue_id}")
}

pub fn legacy_issue_route(issue_id: &str) -> String {
    format!("{ISSUE_ROUTE_PREFIX}{issue_id}")
}
