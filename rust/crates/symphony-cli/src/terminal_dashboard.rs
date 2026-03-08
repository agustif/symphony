use std::io::{self, IsTerminal, Write};
use std::net::IpAddr;

use std::cmp::max;

use serde_json::Value;
use symphony_http::{StateApiView, summarize_codex_message};
use symphony_observability::StateSnapshot;
use terminal_size::{Width, terminal_size};

const DEFAULT_MAX_AGENTS: u32 = 10;
const DEFAULT_TERMINAL_COLUMNS: usize = 115;
const RUNNING_ID_WIDTH: usize = 8;
const RUNNING_STAGE_WIDTH: usize = 14;
const RUNNING_PID_WIDTH: usize = 8;
const RUNNING_AGE_WIDTH: usize = 12;
const RUNNING_TOKENS_WIDTH: usize = 10;
const RUNNING_SESSION_WIDTH: usize = 14;
const RUNNING_EVENT_MIN_WIDTH: usize = 12;
const RUNNING_ROW_CHROME_WIDTH: usize = 10;

const ANSI_RESET: &str = "\u{1b}[0m";
const ANSI_BOLD: &str = "\u{1b}[1m";
const ANSI_BLUE: &str = "\u{1b}[34m";
const ANSI_CYAN: &str = "\u{1b}[36m";
const ANSI_DIM: &str = "\u{1b}[2m";
const ANSI_GREEN: &str = "\u{1b}[32m";
const ANSI_GRAY: &str = "\u{1b}[90m";
const ANSI_MAGENTA: &str = "\u{1b}[35m";
const ANSI_RED: &str = "\u{1b}[31m";
const ANSI_YELLOW: &str = "\u{1b}[33m";
const ANSI_HOME: &str = "\u{1b}[H";
const ANSI_CLEAR: &str = "\u{1b}[2J";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DashboardCodexTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DashboardRunningEntry {
    pub identifier: String,
    pub state: String,
    pub session_id: Option<String>,
    pub codex_app_server_pid: Option<u32>,
    pub codex_total_tokens: u64,
    pub runtime_seconds: u64,
    pub turn_count: u32,
    pub last_codex_event: Option<String>,
    pub last_codex_message: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DashboardRetryEntry {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_in_ms: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DashboardPollingStatus {
    pub checking: bool,
    pub next_poll_in_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalDashboardSnapshot {
    pub running: Vec<DashboardRunningEntry>,
    pub retrying: Vec<DashboardRetryEntry>,
    pub codex_totals: DashboardCodexTotals,
    pub rate_limits: Option<Value>,
    pub polling: Option<DashboardPollingStatus>,
}

impl TerminalDashboardSnapshot {
    #[must_use]
    pub fn from_state_view(view: &StateApiView) -> Self {
        let running = view
            .running
            .iter()
            .map(|issue| DashboardRunningEntry {
                identifier: issue.issue_identifier.clone(),
                state: issue.state.clone(),
                session_id: issue.session_id.clone(),
                codex_app_server_pid: issue.codex_app_server_pid,
                codex_total_tokens: issue.tokens.total_tokens,
                runtime_seconds: issue.runtime_seconds,
                turn_count: issue.turn_count,
                last_codex_event: issue.last_event.clone(),
                last_codex_message: issue.last_message.clone(),
            })
            .collect();
        let retrying = view
            .retrying
            .iter()
            .map(|issue| DashboardRetryEntry {
                issue_id: issue.issue_id.clone(),
                identifier: issue.issue_identifier.clone(),
                attempt: issue.attempt,
                due_in_ms: issue.due_in_ms,
                error: issue.error.clone(),
            })
            .collect();
        let polling = view.polling.as_ref().map(|polling| DashboardPollingStatus {
            checking: polling.checking,
            next_poll_in_ms: polling.next_poll_in_ms,
        });

        Self {
            running,
            retrying,
            codex_totals: DashboardCodexTotals {
                input_tokens: view.codex_totals.input_tokens,
                output_tokens: view.codex_totals.output_tokens,
                total_tokens: view.codex_totals.total_tokens,
                seconds_running: view.codex_totals.seconds_running.trunc() as u64,
            },
            rate_limits: view.rate_limits.clone(),
            polling,
        }
    }

    #[must_use]
    pub fn from_state_snapshot(snapshot: &StateSnapshot, now_unix_seconds: u64) -> Self {
        let view = StateApiView::from_snapshot_with_now_unix_seconds(snapshot, now_unix_seconds);
        Self::from_state_view(&view)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderContext {
    pub max_agents: u32,
    pub project_slug: Option<String>,
    pub dashboard_url: Option<String>,
    pub terminal_columns: Option<usize>,
    pub now_unix_seconds: u64,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            max_agents: DEFAULT_MAX_AGENTS,
            project_slug: None,
            dashboard_url: None,
            terminal_columns: None,
            now_unix_seconds: 0,
        }
    }
}

#[must_use]
pub fn render_snapshot_content_for_test(
    snapshot: &TerminalDashboardSnapshot,
    tps: f64,
    context: &RenderContext,
) -> String {
    let running_event_width = running_event_width(context.terminal_columns);
    let running_rows = format_running_rows(&snapshot.running, running_event_width);
    let backoff_rows = format_retry_rows(&snapshot.retrying);
    let project_link_lines = format_project_link_lines(
        context.project_slug.as_deref(),
        context.dashboard_url.as_deref(),
    );
    let mut lines = vec![
        colorize("╭─ SYMPHONY STATUS", ANSI_BOLD),
        format!(
            "{}{}{}{}{}",
            colorize("│ Agents: ", ANSI_BOLD),
            colorize(&snapshot.running.len().to_string(), ANSI_GREEN),
            colorize("/", ANSI_GRAY),
            colorize(&context.max_agents.to_string(), ANSI_GRAY),
            ""
        ),
        format!(
            "{}{}",
            colorize("│ Throughput: ", ANSI_BOLD),
            colorize(&format!("{} tps", format_tps(tps)), ANSI_CYAN)
        ),
        format!(
            "{}{}",
            colorize("│ Runtime: ", ANSI_BOLD),
            colorize(
                &format_runtime_seconds(snapshot.codex_totals.seconds_running),
                ANSI_MAGENTA,
            )
        ),
        format!(
            "{}{}{}{}{}{}{}",
            colorize("│ Tokens: ", ANSI_BOLD),
            colorize(
                &format!("in {}", format_count(snapshot.codex_totals.input_tokens)),
                ANSI_YELLOW,
            ),
            colorize(" | ", ANSI_GRAY),
            colorize(
                &format!("out {}", format_count(snapshot.codex_totals.output_tokens)),
                ANSI_YELLOW,
            ),
            colorize(" | ", ANSI_GRAY),
            colorize(
                &format!("total {}", format_count(snapshot.codex_totals.total_tokens)),
                ANSI_YELLOW,
            ),
            ""
        ),
        format!(
            "{}{}",
            colorize("│ Rate Limits: ", ANSI_BOLD),
            format_rate_limits(snapshot.rate_limits.as_ref())
        ),
    ];
    lines.extend(project_link_lines);
    lines.push(format_project_refresh_line(snapshot.polling.as_ref()));
    lines.push(colorize("├─ Running", ANSI_BOLD));
    lines.push("│".to_owned());
    lines.push(running_table_header_row(running_event_width));
    lines.push(running_table_separator_row(running_event_width));
    lines.extend(running_rows);
    if !snapshot.running.is_empty() {
        lines.push("│".to_owned());
    }
    lines.push(colorize("├─ Backoff queue", ANSI_BOLD));
    lines.push("│".to_owned());
    lines.extend(backoff_rows);
    lines.push("╰─".to_owned());
    lines.join("\n")
}

#[must_use]
pub fn render_state_snapshot_content_for_test(
    snapshot: &StateSnapshot,
    tps: f64,
    context: &RenderContext,
) -> String {
    render_snapshot_content_for_test(
        &TerminalDashboardSnapshot::from_state_snapshot(snapshot, context.now_unix_seconds),
        tps,
        context,
    )
}

#[must_use]
pub fn stdout_supports_dashboard() -> bool {
    std::io::stdout().is_terminal()
        && std::env::var("TERM")
            .map(|term| term.trim() != "dumb")
            .unwrap_or(true)
}

pub fn render_content_to_terminal(content: &str) -> io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{ANSI_HOME}{ANSI_CLEAR}{content}\n")?;
    stdout.flush()
}

pub fn render_offline_status_to_terminal() -> io::Result<()> {
    let content = [
        colorize("╭─ SYMPHONY STATUS", ANSI_BOLD),
        colorize("│ app_status=offline", ANSI_RED),
        "╰─".to_owned(),
    ]
    .join("\n");
    render_content_to_terminal(&content)
}

#[must_use]
pub fn dashboard_url(
    host: IpAddr,
    configured_port: Option<u16>,
    bound_port: Option<u16>,
) -> Option<String> {
    let port = bound_port.or(configured_port).filter(|port| *port > 0)?;
    Some(format!("http://{}:{port}/", dashboard_url_host(host)))
}

fn format_project_link_lines(
    project_slug: Option<&str>,
    dashboard_url: Option<&str>,
) -> Vec<String> {
    let project = match project_slug.filter(|project_slug| !project_slug.is_empty()) {
        Some(project_slug) => colorize(&project_url(project_slug), ANSI_CYAN),
        None => colorize("n/a", ANSI_GRAY),
    };
    let mut lines = vec![format!("{}{}", colorize("│ Project: ", ANSI_BOLD), project)];

    if let Some(dashboard_url) = dashboard_url {
        lines.push(format!(
            "{}{}",
            colorize("│ Dashboard: ", ANSI_BOLD),
            colorize(dashboard_url, ANSI_CYAN)
        ));
    }

    lines
}

fn format_project_refresh_line(polling: Option<&DashboardPollingStatus>) -> String {
    match polling {
        Some(DashboardPollingStatus { checking: true, .. }) => format!(
            "{}{}",
            colorize("│ Next refresh: ", ANSI_BOLD),
            colorize("checking now…", ANSI_CYAN)
        ),
        Some(DashboardPollingStatus {
            next_poll_in_ms: Some(next_poll_in_ms),
            ..
        }) => {
            let seconds = (next_poll_in_ms.saturating_add(999)) / 1000;
            format!(
                "{}{}",
                colorize("│ Next refresh: ", ANSI_BOLD),
                colorize(&format!("{seconds}s"), ANSI_CYAN)
            )
        }
        _ => format!(
            "{}{}",
            colorize("│ Next refresh: ", ANSI_BOLD),
            colorize("n/a", ANSI_GRAY)
        ),
    }
}

fn format_running_rows(
    running: &[DashboardRunningEntry],
    running_event_width: usize,
) -> Vec<String> {
    if running.is_empty() {
        return vec![
            format!("│  {}", colorize("No active agents", ANSI_GRAY)),
            "│".to_owned(),
        ];
    }

    let mut running = running.to_vec();
    running.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    running
        .iter()
        .map(|entry| format_running_summary(entry, running_event_width))
        .collect()
}

fn format_running_summary(
    running_entry: &DashboardRunningEntry,
    running_event_width: usize,
) -> String {
    let issue = format_cell(&running_entry.identifier, RUNNING_ID_WIDTH, Align::Left);
    let state = format_cell(&running_entry.state, RUNNING_STAGE_WIDTH, Align::Left);
    let pid = format_cell(
        &running_entry
            .codex_app_server_pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "n/a".to_owned()),
        RUNNING_PID_WIDTH,
        Align::Left,
    );
    let age = format_cell(
        &format_runtime_and_turns(running_entry.runtime_seconds, running_entry.turn_count),
        RUNNING_AGE_WIDTH,
        Align::Left,
    );
    let tokens = format_cell(
        &format_count(running_entry.codex_total_tokens),
        RUNNING_TOKENS_WIDTH,
        Align::Right,
    );
    let session = format_cell(
        &compact_session_id(running_entry.session_id.as_deref()),
        RUNNING_SESSION_WIDTH,
        Align::Left,
    );
    let event = format_cell(
        &summarize_message(
            running_entry.last_codex_event.as_deref(),
            running_entry.last_codex_message.as_deref(),
        ),
        running_event_width,
        Align::Left,
    );
    let status_color = status_color(running_entry.last_codex_event.as_deref());

    [
        "│ ".to_owned(),
        colorize("●", status_color),
        " ".to_owned(),
        colorize(&issue, ANSI_CYAN),
        " ".to_owned(),
        colorize(&state, status_color),
        " ".to_owned(),
        colorize(&pid, ANSI_YELLOW),
        " ".to_owned(),
        colorize(&age, ANSI_MAGENTA),
        " ".to_owned(),
        colorize(&tokens, ANSI_YELLOW),
        " ".to_owned(),
        colorize(&session, ANSI_CYAN),
        " ".to_owned(),
        colorize(&event, status_color),
    ]
    .join("")
}

fn format_retry_rows(retrying: &[DashboardRetryEntry]) -> Vec<String> {
    if retrying.is_empty() {
        return vec![format!("│  {}", colorize("No queued retries", ANSI_GRAY))];
    }

    let mut retrying = retrying.to_vec();
    retrying.sort_by_key(|entry| entry.due_in_ms);
    retrying.iter().map(format_retry_summary).collect()
}

fn format_retry_summary(retry_entry: &DashboardRetryEntry) -> String {
    let identifier = if retry_entry.identifier.is_empty() {
        retry_entry.issue_id.as_str()
    } else {
        retry_entry.identifier.as_str()
    };
    format!(
        "│  {} {} {}{}{}{}",
        colorize("↻", ANSI_YELLOW),
        colorize(identifier, ANSI_RED),
        colorize(&format!("attempt={}", retry_entry.attempt), ANSI_YELLOW),
        colorize(" in ", ANSI_DIM),
        colorize(&next_in_words(retry_entry.due_in_ms), ANSI_CYAN),
        format_retry_error(retry_entry.error.as_deref())
    )
}

fn running_table_header_row(running_event_width: usize) -> String {
    let header = [
        format_cell("ID", RUNNING_ID_WIDTH, Align::Left),
        format_cell("STAGE", RUNNING_STAGE_WIDTH, Align::Left),
        format_cell("PID", RUNNING_PID_WIDTH, Align::Left),
        format_cell("AGE / TURN", RUNNING_AGE_WIDTH, Align::Left),
        format_cell("TOKENS", RUNNING_TOKENS_WIDTH, Align::Left),
        format_cell("SESSION", RUNNING_SESSION_WIDTH, Align::Left),
        format_cell("EVENT", running_event_width, Align::Left),
    ]
    .join(" ");

    format!("│   {}", colorize(&header, ANSI_GRAY))
}

fn running_table_separator_row(running_event_width: usize) -> String {
    let separator_width = RUNNING_ID_WIDTH
        + RUNNING_STAGE_WIDTH
        + RUNNING_PID_WIDTH
        + RUNNING_AGE_WIDTH
        + RUNNING_TOKENS_WIDTH
        + RUNNING_SESSION_WIDTH
        + running_event_width
        + 6;

    format!("│   {}", colorize(&"─".repeat(separator_width), ANSI_GRAY))
}

fn running_event_width(terminal_columns_override: Option<usize>) -> usize {
    let terminal_columns = terminal_columns_override.unwrap_or_else(terminal_columns);
    max(
        RUNNING_EVENT_MIN_WIDTH,
        terminal_columns.saturating_sub(fixed_running_width() + RUNNING_ROW_CHROME_WIDTH),
    )
}

fn fixed_running_width() -> usize {
    RUNNING_ID_WIDTH
        + RUNNING_STAGE_WIDTH
        + RUNNING_PID_WIDTH
        + RUNNING_AGE_WIDTH
        + RUNNING_TOKENS_WIDTH
        + RUNNING_SESSION_WIDTH
}

fn terminal_columns() -> usize {
    match terminal_size() {
        Some((Width(columns), _)) if columns > 0 => columns as usize,
        _ => DEFAULT_TERMINAL_COLUMNS,
    }
}

fn format_tps(value: f64) -> String {
    format_count(value.trunc().max(0.0) as u64)
}

fn format_runtime_and_turns(seconds: u64, turn_count: u32) -> String {
    if turn_count > 0 {
        format!("{} / {}", format_runtime_seconds(seconds), turn_count)
    } else {
        format_runtime_seconds(seconds)
    }
}

fn format_runtime_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}m {seconds}s")
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    grouped.chars().rev().collect()
}

fn next_in_words(due_in_ms: u64) -> String {
    let seconds = due_in_ms / 1000;
    let millis = due_in_ms % 1000;
    format!("{seconds}.{millis:03}s")
}

fn format_retry_error(error: Option<&str>) -> String {
    let Some(error) = error else {
        return String::new();
    };

    let sanitized = error
        .replace("\\r\\n", " ")
        .replace("\\r", " ")
        .replace("\\n", " ")
        .replace("\r\n", " ")
        .replace('\r', " ")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if sanitized.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            colorize(
                &format!("error={}", truncate_plain(&sanitized, 96)),
                ANSI_DIM
            )
        )
    }
}

fn format_rate_limits(rate_limits: Option<&Value>) -> String {
    match rate_limits {
        None => colorize("unavailable", ANSI_GRAY),
        Some(Value::Object(_)) => {
            let limit_id = extract_first_string_key(rate_limits, &["limit_id", "limit_name"])
                .unwrap_or_else(|| "unknown".to_owned());
            let primary = format_rate_limit_bucket(extract_value(rate_limits, &["primary"]));
            let secondary = format_rate_limit_bucket(extract_value(rate_limits, &["secondary"]));
            let credits = format_rate_limit_credits(extract_value(rate_limits, &["credits"]));

            [
                colorize(&limit_id, ANSI_YELLOW),
                colorize(" | ", ANSI_GRAY),
                colorize(&format!("primary {primary}"), ANSI_CYAN),
                colorize(" | ", ANSI_GRAY),
                colorize(&format!("secondary {secondary}"), ANSI_CYAN),
                colorize(" | ", ANSI_GRAY),
                colorize(&credits, ANSI_GREEN),
            ]
            .join("")
        }
        Some(other) => colorize(&truncate_plain(&other.to_string(), 80), ANSI_GRAY),
    }
}

fn format_rate_limit_bucket(bucket: Option<&Value>) -> String {
    let Some(bucket) = bucket else {
        return "n/a".to_owned();
    };

    let remaining = extract_first_integer_key(bucket, &["remaining"]);
    let limit = extract_first_integer_key(bucket, &["limit"]);
    let reset_value = extract_first_scalar_key(
        bucket,
        &[
            "reset_in_seconds",
            "resetInSeconds",
            "reset_at",
            "resetAt",
            "resets_at",
            "resetsAt",
        ],
    );

    let base = match (remaining, limit) {
        (Some(remaining), Some(limit)) => {
            format!("{}/{}", format_count(remaining), format_count(limit))
        }
        (Some(remaining), None) => format!("remaining {}", format_count(remaining)),
        (None, Some(limit)) => format!("limit {}", format_count(limit)),
        (None, None) if bucket.as_object().is_some_and(|object| object.is_empty()) => {
            "n/a".to_owned()
        }
        (None, None) => truncate_plain(&bucket.to_string(), 40),
    };

    match reset_value {
        Some(reset_value) => format!("{base} reset {}", format_reset_value(&reset_value)),
        None => base,
    }
}

fn format_rate_limit_credits(credits: Option<&Value>) -> String {
    let Some(credits) = credits else {
        return "credits n/a".to_owned();
    };

    let unlimited = extract_first_bool_key(credits, &["unlimited"]).unwrap_or(false);
    let has_credits = extract_first_bool_key(credits, &["has_credits"]).unwrap_or(false);
    let balance = extract_first_f64_key(credits, &["balance"]);

    if unlimited {
        "credits unlimited".to_owned()
    } else if has_credits {
        match balance {
            Some(balance) => format!("credits {:.2}", balance),
            None => "credits available".to_owned(),
        }
    } else {
        "credits none".to_owned()
    }
}

fn format_reset_value(value: &str) -> String {
    match value.parse::<u64>() {
        Ok(value) => format!("{}s", format_count(value)),
        Err(_) => value.to_owned(),
    }
}

fn summarize_message(event: Option<&str>, message: Option<&str>) -> String {
    summarize_codex_message(event, message).unwrap_or_else(|| "no codex message yet".to_owned())
}

fn compact_session_id(session_id: Option<&str>) -> String {
    let Some(session_id) = session_id else {
        return "n/a".to_owned();
    };
    if session_id.len() > 10 {
        format!(
            "{}...{}",
            &session_id[..4],
            &session_id[session_id.len().saturating_sub(6)..]
        )
    } else {
        session_id.to_owned()
    }
}

fn project_url(project_slug: &str) -> String {
    format!("https://linear.app/project/{project_slug}/issues")
}

fn dashboard_url_host(host: IpAddr) -> String {
    let host = host.to_string();
    match host.as_str() {
        "0.0.0.0" | "::" | "[::]" | "" => "127.0.0.1".to_owned(),
        _ if host.contains(':') => format!("[{host}]"),
        _ => host,
    }
}

fn status_color(event: Option<&str>) -> &'static str {
    match event {
        None => ANSI_RED,
        Some("codex/event/token_count") => ANSI_YELLOW,
        Some("codex/event/task_started") => ANSI_GREEN,
        Some("turn_completed" | "turn/completed") => ANSI_MAGENTA,
        Some(_) => ANSI_BLUE,
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

fn format_cell(value: &str, width: usize, align: Align) -> String {
    let truncated = truncate_plain(&sanitize_cell_text(value), width);
    match align {
        Align::Left => format!("{truncated:<width$}"),
        Align::Right => format!("{truncated:>width$}"),
    }
}

fn sanitize_cell_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_plain(value: &str, width: usize) -> String {
    if value.len() <= width {
        return value.to_owned();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut truncated = String::new();
    for character in value.chars().take(width - 3) {
        truncated.push(character);
    }
    truncated.push_str("...");
    truncated
}

fn colorize(value: &str, code: &str) -> String {
    format!("{code}{value}{ANSI_RESET}")
}

fn extract_value<'a>(value: Option<&'a Value>, keys: &[&str]) -> Option<&'a Value> {
    let mut current = value?;
    for key in keys {
        current = current.get(*key)?;
    }
    Some(current)
}

fn extract_string(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    match extract_value(value, keys)? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn extract_first_string_key(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| extract_string(value, &[*key]))
}

fn extract_first_scalar_key(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(boolean) => Some(boolean.to_string()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        })
}

fn extract_first_integer_key(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse::<u64>().ok(),
            Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
        })
}

fn extract_first_bool_key(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(Value::as_bool)
}

fn extract_first_f64_key(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.trim().parse::<f64>().ok(),
            Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        path::PathBuf,
    };

    use serde_json::json;
    use symphony_testkit::{runtime_snapshot, state_snapshot};

    use super::{
        DashboardCodexTotals, DashboardRetryEntry, DashboardRunningEntry, RenderContext,
        TerminalDashboardSnapshot, dashboard_url, render_snapshot_content_for_test,
        render_state_snapshot_content_for_test,
    };

    #[test]
    fn matches_elixir_idle_snapshot_fixture() {
        let rendered = render_snapshot_content_for_test(
            &TerminalDashboardSnapshot::default(),
            0.0,
            &render_context(),
        );

        assert_dashboard_snapshot("idle", &rendered);
    }

    #[test]
    fn matches_elixir_idle_with_dashboard_url_snapshot_fixture() {
        let rendered = render_snapshot_content_for_test(
            &TerminalDashboardSnapshot::default(),
            0.0,
            &RenderContext {
                dashboard_url: Some("http://127.0.0.1:4000/".to_owned()),
                ..render_context()
            },
        );

        assert_dashboard_snapshot("idle_with_dashboard_url", &rendered);
    }

    #[test]
    fn matches_elixir_super_busy_snapshot_fixture() {
        let rendered = render_snapshot_content_for_test(
            &TerminalDashboardSnapshot {
                running: vec![
                    DashboardRunningEntry {
                        identifier: "MT-101".to_owned(),
                        state: "running".to_owned(),
                        session_id: Some("thread-1234567890".to_owned()),
                        codex_app_server_pid: Some(4242),
                        codex_total_tokens: 120_450,
                        runtime_seconds: 785,
                        turn_count: 11,
                        last_codex_event: Some("turn_completed".to_owned()),
                        last_codex_message: Some(
                            r#"{"method":"turn/completed","params":{"turn":{"status":"completed"}}}"#
                                .to_owned(),
                        ),
                    },
                    DashboardRunningEntry {
                        identifier: "MT-102".to_owned(),
                        state: "running".to_owned(),
                        session_id: Some("thread-abcdef1234567890".to_owned()),
                        codex_app_server_pid: Some(5252),
                        codex_total_tokens: 89_200,
                        runtime_seconds: 412,
                        turn_count: 4,
                        last_codex_event: Some("codex/event/task_started".to_owned()),
                        last_codex_message: Some(
                            r#"{"method":"codex/event/exec_command_begin","params":{"msg":{"command":"mix test --cover"}}}"#
                                .to_owned(),
                        ),
                    },
                ],
                retrying: vec![],
                codex_totals: DashboardCodexTotals {
                    input_tokens: 250_000,
                    output_tokens: 18_500,
                    total_tokens: 268_500,
                    seconds_running: 4_321,
                },
                rate_limits: Some(json!({
                    "limit_id": "gpt-5",
                    "primary": {"remaining": 12_345, "limit": 20_000, "reset_in_seconds": 30},
                    "secondary": {"remaining": 45, "limit": 60, "reset_in_seconds": 12},
                    "credits": {"has_credits": true, "balance": 9_876.5}
                })),
                polling: None,
            },
            1_842.7,
            &render_context(),
        );

        assert_dashboard_snapshot("super_busy", &rendered);
    }

    #[test]
    fn matches_elixir_backoff_queue_snapshot_fixture() {
        let rendered = render_snapshot_content_for_test(
            &TerminalDashboardSnapshot {
                running: vec![DashboardRunningEntry {
                    identifier: "MT-638".to_owned(),
                    state: "retrying".to_owned(),
                    session_id: Some("thread-1234567890".to_owned()),
                    codex_app_server_pid: Some(4242),
                    codex_total_tokens: 14_200,
                    runtime_seconds: 1_225,
                    turn_count: 7,
                    last_codex_event: Some("notification".to_owned()),
                    last_codex_message: Some(
                        r#"{"method":"item/agentMessage/delta","params":{"msg":{"payload":{"delta":"waiting on rate-limit backoff window"}}}}"#
                            .to_owned(),
                    ),
                }],
                retrying: vec![
                    DashboardRetryEntry {
                        issue_id: "issue-1".to_owned(),
                        identifier: "MT-450".to_owned(),
                        attempt: 4,
                        due_in_ms: 1_250,
                        error: Some("rate limit exhausted".to_owned()),
                    },
                    DashboardRetryEntry {
                        issue_id: "issue-2".to_owned(),
                        identifier: "MT-451".to_owned(),
                        attempt: 2,
                        due_in_ms: 3_900,
                        error: Some("retrying after API timeout with jitter".to_owned()),
                    },
                    DashboardRetryEntry {
                        issue_id: "issue-3".to_owned(),
                        identifier: "MT-452".to_owned(),
                        attempt: 6,
                        due_in_ms: 8_100,
                        error: Some("worker crashed\nrestarting cleanly".to_owned()),
                    },
                    DashboardRetryEntry {
                        issue_id: "issue-4".to_owned(),
                        identifier: "MT-453".to_owned(),
                        attempt: 1,
                        due_in_ms: 11_000,
                        error: Some(
                            "fourth queued retry should also render after removing the top-three limit"
                                .to_owned(),
                        ),
                    },
                ],
                codex_totals: DashboardCodexTotals {
                    input_tokens: 18_000,
                    output_tokens: 2_200,
                    total_tokens: 20_200,
                    seconds_running: 2_700,
                },
                rate_limits: Some(json!({
                    "limit_id": "gpt-5",
                    "primary": {"remaining": 0, "limit": 20_000, "reset_in_seconds": 95},
                    "secondary": {"remaining": 0, "limit": 60, "reset_in_seconds": 45},
                    "credits": {"has_credits": false}
                })),
                polling: None,
            },
            15.4,
            &render_context(),
        );

        assert_dashboard_snapshot("backoff_queue", &rendered);
    }

    #[test]
    fn matches_elixir_unlimited_credits_snapshot_fixture() {
        let rendered = render_snapshot_content_for_test(
            &TerminalDashboardSnapshot {
                running: vec![DashboardRunningEntry {
                    identifier: "MT-777".to_owned(),
                    state: "running".to_owned(),
                    session_id: Some("thread-1234567890".to_owned()),
                    codex_app_server_pid: Some(4242),
                    codex_total_tokens: 3_200,
                    runtime_seconds: 75,
                    turn_count: 7,
                    last_codex_event: Some("codex/event/token_count".to_owned()),
                    last_codex_message: Some(
                        r#"{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"total":{"inputTokens":90,"outputTokens":12,"totalTokens":102}}}}"#
                            .to_owned(),
                    ),
                }],
                retrying: vec![],
                codex_totals: DashboardCodexTotals {
                    input_tokens: 90,
                    output_tokens: 12,
                    total_tokens: 102,
                    seconds_running: 75,
                },
                rate_limits: Some(json!({
                    "limit_id": "priority-tier",
                    "primary": {"remaining": 100, "limit": 100, "reset_in_seconds": 1},
                    "secondary": {"remaining": 500, "limit": 500, "reset_in_seconds": 1},
                    "credits": {"unlimited": true}
                })),
                polling: None,
            },
            42.0,
            &render_context(),
        );

        assert_dashboard_snapshot("credits_unlimited", &rendered);
    }

    #[test]
    fn state_snapshot_projection_uses_runtime_timestamps_for_live_rendering() {
        let mut runtime = runtime_snapshot(1, 1);
        runtime.input_tokens = 10;
        runtime.output_tokens = 5;
        runtime.total_tokens = 15;
        runtime.seconds_running = 30.0;
        runtime.activity.next_poll_due_at = Some(205);

        let mut running = symphony_testkit::issue_snapshot("issue-1", "MT-1", "running", 0);
        running.started_at = Some(175);
        running.session_id = Some("thread-1234567890".to_owned());
        running.codex_app_server_pid = Some(4242);
        running.turn_count = 2;

        let mut retrying = symphony_testkit::issue_snapshot("issue-2", "MT-2", "Todo", 2);
        retrying.retry_due_at = Some(205);
        retrying.retry_error = Some("response timeout".to_owned());

        let rendered = render_state_snapshot_content_for_test(
            &state_snapshot(runtime, vec![running, retrying]),
            3.0,
            &RenderContext {
                now_unix_seconds: 200,
                ..render_context()
            },
        );

        assert!(rendered.contains("0m 25s / 2"));
        assert!(rendered.contains("5.000s"));
    }

    #[test]
    fn dashboard_url_prefers_bound_port_and_normalizes_unspecified_hosts() {
        assert_eq!(
            dashboard_url(IpAddr::V4(Ipv4Addr::UNSPECIFIED), Some(4000), Some(4012),).as_deref(),
            Some("http://127.0.0.1:4012/")
        );
        assert_eq!(
            dashboard_url(IpAddr::V6(Ipv6Addr::LOCALHOST), Some(4000), None).as_deref(),
            Some("http://[::1]:4000/")
        );
        assert_eq!(
            dashboard_url(IpAddr::V4(Ipv4Addr::LOCALHOST), Some(0), None),
            None
        );
    }

    fn render_context() -> RenderContext {
        RenderContext {
            max_agents: 10,
            project_slug: Some("project".to_owned()),
            dashboard_url: None,
            terminal_columns: Some(115),
            now_unix_seconds: 0,
        }
    }

    fn assert_dashboard_snapshot(name: &str, rendered: &str) {
        let expected = fs::read_to_string(fixture_path(name)).expect("fixture should exist");
        assert_eq!(escape_ansi(rendered), expected);
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../elixir/test/fixtures/status_dashboard_snapshots")
            .join(format!("{name}.snapshot.txt"))
    }

    fn escape_ansi(content: &str) -> String {
        let escaped = content.replace('\u{1b}', "\\e");
        if escaped.ends_with('\n') {
            escaped
        } else {
            format!("{escaped}\n")
        }
    }
}
