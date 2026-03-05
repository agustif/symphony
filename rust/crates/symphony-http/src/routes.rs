pub const STATE_ROUTE: &str = "/api/v1/state";
pub const ISSUE_ROUTE_PREFIX: &str = "/api/v1/issues/";

pub fn issue_route(issue_id: &str) -> String {
    format!("{ISSUE_ROUTE_PREFIX}{issue_id}")
}
