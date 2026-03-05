use crate::{
    API_V1_PREFIX, ApiResponse, HttpMethod, ISSUE_ROUTE_PREFIX, REFRESH_ROUTE, STATE_ROUTE,
};
use symphony_observability::{IssueSnapshot, RuntimeSnapshot, StateSnapshot};

use crate::payloads::{
    IssueApiView, IssueLegacyView, RefreshAcceptedView, RuntimeLegacyView, StateApiView,
};

pub fn snapshot_to_json(snapshot: &RuntimeSnapshot) -> serde_json::Value {
    RuntimeLegacyView::from(snapshot).to_json()
}

pub fn issue_to_json(issue: &IssueSnapshot) -> serde_json::Value {
    IssueLegacyView::from(issue).to_json()
}

pub fn state_to_json(snapshot: &StateSnapshot) -> serde_json::Value {
    StateApiView::from(snapshot).to_json()
}

pub fn issue_detail_to_json(issue: &IssueSnapshot) -> serde_json::Value {
    IssueApiView::from(issue).to_json()
}

pub fn refresh_to_json() -> serde_json::Value {
    RefreshAcceptedView::default().to_json()
}

pub fn handle_request(method: HttpMethod, path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    if path == STATE_ROUTE {
        return match method {
            HttpMethod::Get => ApiResponse::ok(state_to_json(snapshot)),
            _ => ApiResponse::method_not_allowed("method not allowed for `/api/v1/state`"),
        };
    }

    if path == REFRESH_ROUTE {
        return match method {
            HttpMethod::Post => ApiResponse::accepted(refresh_to_json()),
            _ => ApiResponse::method_not_allowed("method not allowed for `/api/v1/refresh`"),
        };
    }

    if let Some(issue_identifier) = issue_identifier_from_path(path) {
        return match method {
            HttpMethod::Get => snapshot
                .issue_by_id_or_identifier(issue_identifier)
                .map_or_else(
                    || ApiResponse::issue_not_found(issue_identifier),
                    |issue| ApiResponse::ok(issue_detail_to_json(issue)),
                ),
            _ => ApiResponse::method_not_allowed("method not allowed for issue route"),
        };
    }

    ApiResponse::not_found("route not found")
}

pub fn handle_get(path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    handle_request(HttpMethod::Get, path, snapshot)
}

pub fn handle_post(path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    handle_request(HttpMethod::Post, path, snapshot)
}

fn issue_identifier_from_path(path: &str) -> Option<&str> {
    if let Some(issue_identifier) = path.strip_prefix(ISSUE_ROUTE_PREFIX) {
        return normalize_issue_identifier(issue_identifier);
    }

    let short_route = path.strip_prefix(API_V1_PREFIX)?.strip_prefix('/')?;
    if matches!(short_route, "state" | "refresh") || short_route.starts_with("issues/") {
        return None;
    }

    normalize_issue_identifier(short_route)
}

fn normalize_issue_identifier(issue_identifier: &str) -> Option<&str> {
    if issue_identifier.is_empty() || issue_identifier.contains('/') {
        None
    } else {
        Some(issue_identifier)
    }
}
