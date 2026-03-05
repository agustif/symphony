use std::fmt::Write as _;

use crate::{
    API_V1_PREFIX, ApiResponse, DASHBOARD_ROUTE, HttpMethod, ISSUE_ROUTE_PREFIX, REFRESH_ROUTE,
    STATE_ROUTE,
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

pub fn dashboard_to_html(snapshot: &StateSnapshot) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        concat!(
            "<!doctype html>",
            "<html lang=\"en\">",
            "<head>",
            "<meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>Symphony Dashboard</title>",
            "<style>",
            "body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 1.5rem; background: #f8fafc; color: #0f172a; }}",
            "main {{ max-width: 60rem; margin: 0 auto; display: grid; gap: 1rem; }}",
            "section {{ background: #ffffff; border: 1px solid #e2e8f0; border-radius: 0.75rem; padding: 1rem; }}",
            "h1, h2 {{ margin: 0 0 0.5rem; }}",
            "dl {{ display: grid; grid-template-columns: max-content max-content; gap: 0.25rem 0.75rem; margin: 0; }}",
            "ul {{ margin: 0; padding-left: 1.25rem; }}",
            "code {{ background: #e2e8f0; border-radius: 0.375rem; padding: 0.1rem 0.35rem; }}",
            "</style>",
            "</head>",
            "<body>",
            "<main>",
            "<header>",
            "<h1>Symphony Dashboard</h1>",
            "<p>Current orchestration snapshot from in-memory runtime state.</p>",
            "</header>",
            "<section aria-labelledby=\"summary-heading\">",
            "<h2 id=\"summary-heading\">Summary</h2>",
            "<dl>",
            "<dt>Running</dt><dd>{}</dd>",
            "<dt>Retrying</dt><dd>{}</dd>",
            "<dt>Total Issues</dt><dd>{}</dd>",
            "</dl>",
            "</section>",
        ),
        snapshot.runtime.running,
        snapshot.runtime.retrying,
        snapshot.issues.len()
    );

    let running_issues = snapshot
        .issues
        .iter()
        .filter(|issue| issue.retry_attempts == 0)
        .collect::<Vec<_>>();
    let retrying_issues = snapshot
        .issues
        .iter()
        .filter(|issue| issue.retry_attempts > 0)
        .collect::<Vec<_>>();

    html.push_str("<section aria-labelledby=\"running-heading\"><h2 id=\"running-heading\">Running Issues</h2>");
    push_issue_list(&mut html, &running_issues, "No running issues.", false);
    html.push_str("</section>");

    html.push_str(
        "<section aria-labelledby=\"retrying-heading\"><h2 id=\"retrying-heading\">Retrying Issues</h2>",
    );
    push_issue_list(&mut html, &retrying_issues, "No retrying issues.", true);
    html.push_str("</section>");

    html.push_str(concat!(
        "<section aria-labelledby=\"api-heading\">",
        "<h2 id=\"api-heading\">API Endpoints</h2>",
        "<ul>",
        "<li><code>GET /api/v1/state</code></li>",
        "<li><code>GET /api/v1/&lt;issue_identifier&gt;</code></li>",
        "<li><code>POST /api/v1/refresh</code></li>",
        "</ul>",
        "</section>",
        "</main>",
        "</body>",
        "</html>"
    ));

    html
}

pub fn handle_request(method: HttpMethod, path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    if path == DASHBOARD_ROUTE {
        return match method {
            HttpMethod::Get => ApiResponse::html(dashboard_to_html(snapshot)),
            _ => ApiResponse::method_not_allowed("method not allowed for `/`"),
        };
    }

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

fn push_issue_list(
    html: &mut String,
    issues: &[&IssueSnapshot],
    empty_message: &str,
    include_attempt: bool,
) {
    if issues.is_empty() {
        let _ = write!(html, "<p>{}</p>", escape_html(empty_message));
        return;
    }

    html.push_str("<ul>");
    for issue in issues {
        let _ = write!(
            html,
            "<li><strong>{}</strong> <span>{}</span>",
            escape_html(&issue.identifier),
            escape_html(&issue.state),
        );
        if include_attempt {
            let _ = write!(html, " <span>(attempt {})</span>", issue.retry_attempts);
        }
        html.push_str("</li>");
    }
    html.push_str("</ul>");
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}
