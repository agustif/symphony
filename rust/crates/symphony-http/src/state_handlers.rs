use std::fmt::Write as _;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    API_V1_PREFIX, ApiResponse, DASHBOARD_ROUTE, HttpMethod, ISSUE_ROUTE_PREFIX, REFRESH_ROUTE,
    STATE_ROUTE,
};
use symphony_observability::{
    IssueSnapshot, RuntimeSnapshot, SnapshotStatus, StateSnapshot, StateSnapshotEnvelope,
};

use crate::payloads::{
    IssueApiView, IssueLegacyView, RefreshAcceptedView, RetryIssueView, RunningIssueView,
    RuntimeLegacyView, StateApiView, now_iso8601_utc,
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

pub fn state_envelope_to_json(envelope: &StateSnapshotEnvelope) -> serde_json::Value {
    match (&envelope.snapshot, &envelope.error, envelope.status) {
        (Some(snapshot), Some(error), SnapshotStatus::Stale) => {
            let mut payload = state_to_json(snapshot);
            payload["error"] = serde_json::json!({
                "code": error.code.api_code(),
                "message": error.message,
            });
            payload
        }
        (Some(snapshot), _, _) => state_to_json(snapshot),
        (None, Some(error), _) => {
            ApiResponse::snapshot_error(
                now_iso8601_utc(),
                error.code.api_code(),
                error.message.clone(),
            )
            .body
        }
        (None, None, _) => {
            ApiResponse::snapshot_error(
                now_iso8601_utc(),
                "snapshot_unavailable",
                "Snapshot unavailable",
            )
            .body
        }
    }
}

pub fn issue_detail_to_json(issue: &IssueSnapshot) -> serde_json::Value {
    IssueApiView::from_issue(issue, None).to_json()
}

pub fn issue_detail_to_json_with_workspace(
    issue: &IssueSnapshot,
    workspace_root: Option<&Path>,
) -> serde_json::Value {
    IssueApiView::from_issue(issue, workspace_root).to_json()
}

pub fn refresh_to_json() -> serde_json::Value {
    refresh_to_json_with_coalesced(false)
}

pub fn refresh_to_json_with_coalesced(coalesced: bool) -> serde_json::Value {
    RefreshAcceptedView {
        coalesced,
        ..RefreshAcceptedView::default()
    }
    .to_json()
}

pub fn dashboard_to_html(snapshot: &StateSnapshot) -> String {
    dashboard_envelope_to_html(&StateSnapshotEnvelope::ready(snapshot.clone()))
}

pub fn dashboard_envelope_to_html(envelope: &StateSnapshotEnvelope) -> String {
    let snapshot = envelope.snapshot.as_ref().cloned().unwrap_or_default();
    let view = StateApiView::from(&snapshot);
    let now = DateTime::<Utc>::from(SystemTime::now());
    let (health_label, health_class) = match (&envelope.error, envelope.status) {
        (Some(_), SnapshotStatus::Stale) => ("stale", "status-stale"),
        (Some(_), _) => ("offline", "status-offline"),
        (None, _) => ("live", "status-live"),
    };

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
            "main {{ max-width: 78rem; margin: 0 auto; display: grid; gap: 1rem; }}",
            "section {{ background: #ffffff; border: 1px solid #e2e8f0; border-radius: 0.75rem; padding: 1rem; }}",
            "h1, h2, p {{ margin: 0; }}",
            "header {{ display: grid; gap: 0.5rem; }}",
            "ul {{ margin: 0; padding-left: 1.25rem; }}",
            "code, .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }}",
            "code {{ background: #e2e8f0; border-radius: 0.375rem; padding: 0.1rem 0.35rem; }}",
            "dl {{ display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 0.25rem 0.75rem; margin: 0; }}",
            ".metric-grid {{ display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr)); }}",
            ".metric-card {{ display: grid; gap: 0.4rem; }}",
            ".metric-label {{ color: #475569; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.04em; font-weight: 700; }}",
            ".metric-value {{ font-size: 2rem; font-weight: 700; }}",
            ".metric-detail {{ color: #64748b; font-size: 0.95rem; }}",
            ".section-header {{ display: flex; align-items: end; justify-content: space-between; gap: 1rem; margin-bottom: 0.9rem; }}",
            ".section-copy {{ color: #64748b; margin-top: 0.25rem; }}",
            ".table-wrap {{ overflow-x: auto; }}",
            "table {{ width: 100%; border-collapse: collapse; min-width: 44rem; }}",
            "th, td {{ padding: 0.75rem 0.5rem; border-top: 1px solid #e2e8f0; vertical-align: top; text-align: left; }}",
            "th {{ color: #475569; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.04em; border-top: none; padding-top: 0; }}",
            ".issue-stack, .detail-stack, .token-stack {{ display: grid; gap: 0.25rem; }}",
            ".issue-link {{ color: #2563eb; text-decoration: none; font-size: 0.9rem; }}",
            ".issue-id {{ font-weight: 700; }}",
            ".muted {{ color: #64748b; }}",
            ".numeric {{ font-variant-numeric: tabular-nums; }}",
            ".empty-state {{ color: #64748b; }}",
            ".code-panel {{ margin: 0; padding: 0.9rem; overflow-x: auto; border-radius: 0.75rem; background: #0f172a; color: #e2e8f0; }}",
            ".state-badge {{ display: inline-block; padding: 0.2rem 0.55rem; border-radius: 999px; font-size: 0.86rem; font-weight: 600; background: #e2e8f0; color: #1e293b; }}",
            ".state-active {{ background: #dcfce7; color: #166534; }}",
            ".state-warning {{ background: #fef3c7; color: #92400e; }}",
            ".state-danger {{ background: #fee2e2; color: #991b1b; }}",
            ".status-pill {{ display: inline-block; border-radius: 999px; padding: 0.2rem 0.6rem; font-size: 0.9rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; }}",
            ".status-live {{ background: #dcfce7; color: #166534; }}",
            ".status-stale {{ background: #fef3c7; color: #92400e; }}",
            ".status-offline {{ background: #fee2e2; color: #991b1b; }}",
            "</style>",
            "</head>",
            "<body>",
            "<main>",
            "<header>",
            "<h1>Symphony Dashboard</h1>",
            "<p>Current orchestration snapshot from in-memory runtime state.</p>",
            "<p><span class=\"status-pill {}\">{}</span></p>",
            "</header>",
            "<section aria-labelledby=\"metrics-heading\">",
            "<div class=\"section-header\">",
            "<div>",
            "<h2 id=\"metrics-heading\">Metrics</h2>",
            "<p class=\"section-copy\">Live issue counts, aggregate Codex token usage, runtime totals, and orchestration activity.</p>",
            "</div>",
            "</div>",
            "<div class=\"metric-grid\">",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Running</p>",
            "<p class=\"metric-value numeric\">{}</p>",
            "<p class=\"metric-detail\">Active issue sessions in the current runtime.</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Retrying</p>",
            "<p class=\"metric-value numeric\">{}</p>",
            "<p class=\"metric-detail\">Issues waiting for the next retry window.</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Total Tokens</p>",
            "<p class=\"metric-value numeric\">{}</p>",
            "<p class=\"metric-detail numeric\">In {} / Out {}</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Runtime</p>",
            "<p class=\"metric-value numeric\">{}</p>",
            "<p class=\"metric-detail\">Total Codex runtime across completed and active sessions.</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Poll Status</p>",
            "<p class=\"metric-value\">{}</p>",
            "<p class=\"metric-detail\">{}</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Last Activity</p>",
            "<p class=\"metric-value\">{}</p>",
            "<p class=\"metric-detail\">{}</p>",
            "</article>",
            "<article class=\"metric-card\">",
            "<p class=\"metric-label\">Throughput</p>",
            "<p class=\"metric-value numeric\">{}</p>",
            "<p class=\"metric-detail\">{} · In {} / Out {}</p>",
            "</article>",
            "</div>",
            "</section>",
        ),
        health_class,
        health_label,
        view.counts.running,
        view.counts.retrying,
        format_token_count(view.codex_totals.total_tokens),
        format_token_count(view.codex_totals.input_tokens),
        format_token_count(view.codex_totals.output_tokens),
        format_duration_seconds(total_runtime_seconds(&view, &now)),
        escape_html(&poll_status_value(&view)),
        escape_html(&poll_status_detail(&view, &now)),
        escape_html(&last_activity_value(&view, &now)),
        escape_html(&last_activity_detail(&view)),
        escape_html(&format_tps(
            view.activity.throughput.total_tokens_per_second
        )),
        escape_html(&format_throughput_window(
            view.activity.throughput.window_seconds
        )),
        escape_html(&format_tps(
            view.activity.throughput.input_tokens_per_second
        )),
        escape_html(&format_tps(
            view.activity.throughput.output_tokens_per_second
        )),
    );

    if let Some(error) = &envelope.error {
        let _ = write!(
            html,
            concat!(
                "<section aria-labelledby=\"health-heading\">",
                "<h2 id=\"health-heading\">Snapshot Health</h2>",
                "<dl>",
                "<dt>Code</dt><dd><code>{}</code></dd>",
                "<dt>Message</dt><dd>{}</dd>",
                "</dl>",
                "</section>"
            ),
            escape_html(error.code.api_code()),
            escape_html(&error.message),
        );
    }

    push_rate_limits_panel(&mut html, view.rate_limits.as_ref());
    push_running_sessions_table(&mut html, &view.running, &now);
    push_retry_queue_table(&mut html, &view.retrying, &now);

    html.push_str(concat!(
        "<section aria-labelledby=\"api-heading\">",
        "<div class=\"section-header\"><div><h2 id=\"api-heading\">API Endpoints</h2><p class=\"section-copy\">Machine-readable surfaces backed by the same snapshot.</p></div></div>",
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
    handle_request_with_workspace(method, path, snapshot, None)
}

pub fn handle_request_with_workspace(
    method: HttpMethod,
    path: &str,
    snapshot: &StateSnapshot,
    workspace_root: Option<&Path>,
) -> ApiResponse {
    handle_request_with_workspace_envelope(
        method,
        path,
        &StateSnapshotEnvelope::ready(snapshot.clone()),
        workspace_root,
    )
}

pub fn handle_request_with_workspace_envelope(
    method: HttpMethod,
    path: &str,
    envelope: &StateSnapshotEnvelope,
    workspace_root: Option<&Path>,
) -> ApiResponse {
    if path == DASHBOARD_ROUTE {
        return match method {
            HttpMethod::Get => ApiResponse::html(dashboard_envelope_to_html(envelope)),
            _ => ApiResponse::method_not_allowed(),
        };
    }

    if path == STATE_ROUTE {
        return match method {
            HttpMethod::Get => ApiResponse::ok(state_envelope_to_json(envelope)),
            _ => ApiResponse::method_not_allowed(),
        };
    }

    if path == REFRESH_ROUTE {
        return match method {
            HttpMethod::Post => ApiResponse::accepted(refresh_to_json()),
            _ => ApiResponse::method_not_allowed(),
        };
    }

    if let Some(issue_identifier) = issue_identifier_from_path(path) {
        return match method {
            HttpMethod::Get => envelope
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.issue_by_id_or_identifier(issue_identifier))
                .map_or_else(ApiResponse::issue_not_found, |issue| {
                    ApiResponse::ok(issue_detail_to_json_with_workspace(issue, workspace_root))
                }),
            _ => ApiResponse::method_not_allowed(),
        };
    }

    ApiResponse::not_found()
}

pub fn handle_get(path: &str, snapshot: &StateSnapshot) -> ApiResponse {
    handle_request(HttpMethod::Get, path, snapshot)
}

pub fn handle_get_with_workspace(
    path: &str,
    snapshot: &StateSnapshot,
    workspace_root: Option<&Path>,
) -> ApiResponse {
    handle_request_with_workspace(HttpMethod::Get, path, snapshot, workspace_root)
}

pub fn handle_get_with_workspace_envelope(
    path: &str,
    envelope: &StateSnapshotEnvelope,
    workspace_root: Option<&Path>,
) -> ApiResponse {
    handle_request_with_workspace_envelope(HttpMethod::Get, path, envelope, workspace_root)
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

fn push_rate_limits_panel(html: &mut String, rate_limits: Option<&Value>) {
    let _ = write!(
        html,
        concat!(
            "<section aria-labelledby=\"rate-limits-heading\">",
            "<div class=\"section-header\">",
            "<div>",
            "<h2 id=\"rate-limits-heading\">Rate limits</h2>",
            "<p class=\"section-copy\">Latest upstream rate-limit snapshot, when available.</p>",
            "</div>",
            "</div>",
            "<pre class=\"code-panel\">{}</pre>",
            "</section>"
        ),
        escape_html(&format_rate_limits(rate_limits)),
    );
}

fn push_running_sessions_table(
    html: &mut String,
    running: &[RunningIssueView],
    now: &DateTime<Utc>,
) {
    html.push_str(concat!(
        "<section aria-labelledby=\"running-heading\">",
        "<div class=\"section-header\">",
        "<div>",
        "<h2 id=\"running-heading\">Running sessions</h2>",
        "<p class=\"section-copy\">Active issues, last known agent activity, and token usage.</p>",
        "</div>",
        "</div>"
    ));

    if running.is_empty() {
        html.push_str("<p class=\"empty-state\">No active sessions.</p></section>");
        return;
    }

    html.push_str(concat!(
        "<div class=\"table-wrap\">",
        "<table>",
        "<thead>",
        "<tr>",
        "<th>Issue</th>",
        "<th>State</th>",
        "<th>Session</th>",
        "<th>Runtime / turns</th>",
        "<th>Codex update</th>",
        "<th>Tokens</th>",
        "</tr>",
        "</thead>",
        "<tbody>"
    ));

    for entry in running {
        let issue_href = format!("{API_V1_PREFIX}/{}", escape_html(&entry.issue_identifier));
        let session = entry.session_id.as_deref().unwrap_or("n/a");
        let last_update = entry
            .last_message
            .as_deref()
            .or(entry.last_event.as_deref())
            .unwrap_or("n/a");
        let last_event = entry.last_event.as_deref().unwrap_or("n/a");
        let last_event_at = entry.last_event_at.as_deref().unwrap_or("n/a");

        let _ = write!(
            html,
            concat!(
                "<tr>",
                "<td>",
                "<div class=\"issue-stack\">",
                "<span class=\"issue-id\">{}</span>",
                "<a class=\"issue-link\" href=\"{}\">JSON details</a>",
                "</div>",
                "</td>",
                "<td><span class=\"state-badge {}\">{}</span></td>",
                "<td class=\"mono\" title=\"{}\">{}</td>",
                "<td class=\"numeric\">{}</td>",
                "<td>",
                "<div class=\"detail-stack\">",
                "<span title=\"{}\">{}</span>",
                "<span class=\"muted\">{} · <span class=\"mono numeric\" title=\"{}\">{}</span></span>",
                "</div>",
                "</td>",
                "<td>",
                "<div class=\"token-stack numeric\">",
                "<span>Total: {}</span>",
                "<span class=\"muted\">In {} / Out {}</span>",
                "</div>",
                "</td>",
                "</tr>"
            ),
            escape_html(&entry.issue_identifier),
            issue_href,
            state_badge_class(&entry.state),
            escape_html(&entry.state),
            escape_html(session),
            escape_html(session),
            format_runtime_and_turns(entry.started_at.as_deref(), entry.turn_count, now),
            escape_html(last_update),
            escape_html(last_update),
            escape_html(last_event),
            escape_html(last_event_at),
            format_relative_time(entry.last_event_at.as_deref(), now),
            format_token_count(entry.tokens.total_tokens),
            format_token_count(entry.tokens.input_tokens),
            format_token_count(entry.tokens.output_tokens),
        );
    }

    html.push_str("</tbody></table></div></section>");
}

fn push_retry_queue_table(html: &mut String, retrying: &[RetryIssueView], now: &DateTime<Utc>) {
    html.push_str(concat!(
        "<section aria-labelledby=\"retrying-heading\">",
        "<div class=\"section-header\">",
        "<div>",
        "<h2 id=\"retrying-heading\">Retry queue</h2>",
        "<p class=\"section-copy\">Issues waiting for the next retry window.</p>",
        "</div>",
        "</div>"
    ));

    if retrying.is_empty() {
        html.push_str(
            "<p class=\"empty-state\">No issues are currently backing off.</p></section>",
        );
        return;
    }

    html.push_str(concat!(
        "<div class=\"table-wrap\">",
        "<table>",
        "<thead>",
        "<tr>",
        "<th>Issue</th>",
        "<th>Attempt</th>",
        "<th>Due at</th>",
        "<th>Error</th>",
        "</tr>",
        "</thead>",
        "<tbody>"
    ));

    for entry in retrying {
        let issue_href = format!("{API_V1_PREFIX}/{}", escape_html(&entry.issue_identifier));
        let due_at = entry.due_at.as_deref().unwrap_or("n/a");
        let _ = write!(
            html,
            concat!(
                "<tr>",
                "<td>",
                "<div class=\"issue-stack\">",
                "<span class=\"issue-id\">{}</span>",
                "<a class=\"issue-link\" href=\"{}\">JSON details</a>",
                "</div>",
                "</td>",
                "<td class=\"numeric\">{}</td>",
                "<td><div class=\"detail-stack\"><span>{}</span><span class=\"muted mono\" title=\"{}\">{}</span></div></td>",
                "<td>{}</td>",
                "</tr>"
            ),
            escape_html(&entry.issue_identifier),
            issue_href,
            entry.attempt,
            format_relative_time(entry.due_at.as_deref(), now),
            escape_html(due_at),
            escape_html(due_at),
            escape_html(entry.error.as_deref().unwrap_or("n/a")),
        );
    }

    html.push_str("</tbody></table></div></section>");
}

fn total_runtime_seconds(view: &StateApiView, now: &DateTime<Utc>) -> i64 {
    completed_runtime_seconds(view)
        + view
            .running
            .iter()
            .map(|entry| runtime_seconds_from_started_at(entry.started_at.as_deref(), now))
            .sum::<i64>()
}

fn completed_runtime_seconds(view: &StateApiView) -> i64 {
    view.codex_totals.seconds_running.max(0.0).trunc() as i64
}

fn poll_status_value(view: &StateApiView) -> String {
    if view.activity.poll_in_progress {
        "checking now".to_owned()
    } else if view.activity.next_poll_due_at.is_some() {
        "idle".to_owned()
    } else {
        "n/a".to_owned()
    }
}

fn poll_status_detail(view: &StateApiView, now: &DateTime<Utc>) -> String {
    let last_completed = format_relative_time(view.activity.last_poll_completed_at.as_deref(), now);
    let next_due = format_relative_time(view.activity.next_poll_due_at.as_deref(), now);

    if view.activity.poll_in_progress {
        let started = format_relative_time(view.activity.last_poll_started_at.as_deref(), now);
        return format!("started {started}");
    }

    match (
        view.activity.last_poll_completed_at.as_deref(),
        view.activity.next_poll_due_at.as_deref(),
    ) {
        (Some(_), Some(_)) => format!("last completed {last_completed} · next poll {next_due}"),
        (Some(_), None) => format!("last completed {last_completed}"),
        (None, Some(_)) => format!("next poll {next_due}"),
        (None, None) => "poll timing not available".to_owned(),
    }
}

fn last_activity_value(view: &StateApiView, now: &DateTime<Utc>) -> String {
    format_relative_time(view.activity.last_runtime_activity_at.as_deref(), now)
}

fn last_activity_detail(view: &StateApiView) -> String {
    view.activity
        .last_runtime_activity_at
        .clone()
        .unwrap_or_else(|| "waiting for runtime activity".to_owned())
}

fn format_tps(value: f64) -> String {
    format!("{value:.1} tps")
}

fn format_throughput_window(window_seconds: f64) -> String {
    format!("{window_seconds:.1}s window")
}

fn format_runtime_and_turns(
    started_at: Option<&str>,
    turn_count: u32,
    now: &DateTime<Utc>,
) -> String {
    let runtime = format_duration_seconds(runtime_seconds_from_started_at(started_at, now));
    if turn_count == 0 {
        runtime
    } else {
        let suffix = if turn_count == 1 { "turn" } else { "turns" };
        format!("{runtime} / {turn_count} {suffix}")
    }
}

fn runtime_seconds_from_started_at(started_at: Option<&str>, now: &DateTime<Utc>) -> i64 {
    parse_iso8601(started_at)
        .map(|started_at| now.signed_duration_since(started_at).num_seconds().max(0))
        .unwrap_or(0)
}

fn format_relative_time(timestamp: Option<&str>, now: &DateTime<Utc>) -> String {
    let Some(timestamp) = parse_iso8601(timestamp) else {
        return "n/a".to_owned();
    };
    let delta = timestamp.signed_duration_since(*now).num_seconds();
    if delta == 0 {
        return "just now".to_owned();
    }

    let label = format_compact_duration(delta.unsigned_abs() as i64);
    if delta.is_negative() {
        format!("{label} ago")
    } else {
        format!("in {label}")
    }
}

fn format_duration_seconds(seconds: i64) -> String {
    format_compact_duration(seconds.max(0))
}

fn format_compact_duration(seconds: i64) -> String {
    let total = seconds.max(0);
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;
    let secs = total % 60;

    if days > 0 {
        return format!("{days}d {hours}h");
    }
    if hours > 0 {
        return format!("{hours}h {minutes}m");
    }
    if minutes > 0 {
        return format!("{minutes}m {secs}s");
    }
    format!("{secs}s")
}

fn format_token_count(value: u64) -> String {
    let mut digits = value.to_string();
    let mut parts = Vec::new();
    while digits.len() > 3 {
        let chunk = digits.split_off(digits.len() - 3);
        parts.push(chunk);
    }
    parts.push(digits);
    parts.reverse();
    parts.join(",")
}

fn format_rate_limits(rate_limits: Option<&Value>) -> String {
    rate_limits
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| "n/a".to_owned())
}

fn parse_iso8601(timestamp: Option<&str>) -> Option<DateTime<Utc>> {
    let timestamp = timestamp?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn state_badge_class(state: &str) -> &'static str {
    let normalized = state.to_ascii_lowercase();
    if normalized.contains("progress")
        || normalized.contains("running")
        || normalized.contains("active")
    {
        "state-active"
    } else if normalized.contains("retry")
        || normalized.contains("pending")
        || normalized.contains("todo")
        || normalized.contains("queued")
    {
        "state-warning"
    } else if normalized.contains("error")
        || normalized.contains("failed")
        || normalized.contains("blocked")
    {
        "state-danger"
    } else {
        ""
    }
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{
        format_relative_time, format_runtime_and_turns, format_token_count,
        runtime_seconds_from_started_at,
    };

    #[test]
    fn formats_relative_times_for_past_future_and_missing_values() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 12, 0, 0).unwrap();

        assert_eq!(
            format_relative_time(Some("2026-03-06T11:58:30Z"), &now),
            "1m 30s ago"
        );
        assert_eq!(
            format_relative_time(Some("2026-03-06T12:02:00Z"), &now),
            "in 2m 0s"
        );
        assert_eq!(format_relative_time(None, &now), "n/a");
    }

    #[test]
    fn formats_runtime_and_turn_counts() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 12, 0, 0).unwrap();

        assert_eq!(
            format_runtime_and_turns(Some("2026-03-06T11:57:45Z"), 3, &now),
            "2m 15s / 3 turns"
        );
        assert_eq!(
            format_runtime_and_turns(Some("2026-03-06T11:59:59Z"), 1, &now),
            "1s / 1 turn"
        );
        assert_eq!(runtime_seconds_from_started_at(None, &now), 0);
    }

    #[test]
    fn formats_token_counts_with_grouping() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1_234_567), "1,234,567");
    }
}
