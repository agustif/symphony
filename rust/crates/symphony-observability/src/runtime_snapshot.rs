use serde::{Deserialize, Serialize};

use crate::{format_json_compact_sorted, sanitize_json_value};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThroughputSnapshot {
    #[serde(default)]
    pub window_seconds: f64,
    #[serde(default)]
    pub input_tokens_per_second: f64,
    #[serde(default)]
    pub output_tokens_per_second: f64,
    #[serde(default)]
    pub total_tokens_per_second: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeActivitySnapshot {
    #[serde(default)]
    pub poll_in_progress: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_poll_started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_poll_completed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_poll_due_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_runtime_activity_at: Option<u64>,
    #[serde(default)]
    pub throughput: ThroughputSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub running: usize,
    pub retrying: usize,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub seconds_running: f64,
    #[serde(default)]
    pub rate_limits: Option<serde_json::Value>,
    #[serde(default)]
    pub activity: RuntimeActivitySnapshot,
}

impl RuntimeSnapshot {
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            running: self.running,
            retrying: self.retrying,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            total_tokens: self.total_tokens,
            seconds_running: normalize_display_float(self.seconds_running),
            rate_limits: self.rate_limits.as_ref().map(sanitize_json_value),
            activity: self.activity.normalized(),
        }
    }

    #[must_use]
    pub fn health(&self) -> RuntimeHealthView {
        let snapshot = self.normalized();
        let status = if snapshot.activity.poll_in_progress {
            RuntimeHealthStatus::Polling
        } else if snapshot.running > 0 || snapshot.retrying > 0 {
            RuntimeHealthStatus::Busy
        } else if snapshot.activity.last_poll_completed_at.is_some()
            || snapshot.activity.next_poll_due_at.is_some()
            || snapshot.activity.last_runtime_activity_at.is_some()
        {
            RuntimeHealthStatus::Idle
        } else {
            RuntimeHealthStatus::Unknown
        };

        RuntimeHealthView {
            status,
            has_running_work: snapshot.running > 0,
            has_retry_backlog: snapshot.retrying > 0,
            poll_in_progress: snapshot.activity.poll_in_progress,
            has_rate_limits: snapshot.rate_limits.is_some(),
            last_poll_started_at: snapshot.activity.last_poll_started_at,
            last_poll_completed_at: snapshot.activity.last_poll_completed_at,
            next_poll_due_at: snapshot.activity.next_poll_due_at,
            last_runtime_activity_at: snapshot.activity.last_runtime_activity_at,
        }
    }

    #[must_use]
    pub fn summary(&self) -> RuntimeSummaryView {
        let snapshot = self.normalized();
        RuntimeSummaryView {
            counts: format!(
                "running={} retrying={} total_active={}",
                snapshot.running,
                snapshot.retrying,
                snapshot.running.saturating_add(snapshot.retrying)
            ),
            tokens: format!(
                "input_tokens={} output_tokens={} total_tokens={} seconds_running={}",
                snapshot.input_tokens,
                snapshot.output_tokens,
                snapshot.total_tokens,
                format_one_decimal(snapshot.seconds_running)
            ),
            activity: format!(
                "poll_in_progress={} last_poll_started_at={} last_poll_completed_at={} next_poll_due_at={} last_runtime_activity_at={}",
                snapshot.activity.poll_in_progress,
                format_optional_u64(snapshot.activity.last_poll_started_at),
                format_optional_u64(snapshot.activity.last_poll_completed_at),
                format_optional_u64(snapshot.activity.next_poll_due_at),
                format_optional_u64(snapshot.activity.last_runtime_activity_at),
            ),
            throughput: format!(
                "window_seconds={} input_tps={} output_tps={} total_tps={}",
                format_one_decimal(snapshot.activity.throughput.window_seconds),
                format_one_decimal(snapshot.activity.throughput.input_tokens_per_second),
                format_one_decimal(snapshot.activity.throughput.output_tokens_per_second),
                format_one_decimal(snapshot.activity.throughput.total_tokens_per_second),
            ),
            rate_limits: snapshot
                .rate_limits
                .as_ref()
                .map(format_json_compact_sorted),
        }
    }

    #[must_use]
    pub fn spec_view(&self) -> RuntimeSpecView {
        let snapshot = self.normalized();
        let health = snapshot.health();
        let summary = snapshot.summary();
        RuntimeSpecView {
            counts: RuntimeCounts {
                running: snapshot.running,
                retrying: snapshot.retrying,
            },
            codex_totals: CodexTotalsSnapshot {
                input_tokens: snapshot.input_tokens,
                output_tokens: snapshot.output_tokens,
                total_tokens: snapshot.total_tokens,
                seconds_running: snapshot.seconds_running,
            },
            rate_limits: snapshot.rate_limits,
            activity: snapshot.activity,
            health,
            summary,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCounts {
    pub running: usize,
    pub retrying: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CodexTotalsSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub seconds_running: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSpecView {
    pub counts: RuntimeCounts,
    pub codex_totals: CodexTotalsSnapshot,
    #[serde(default)]
    pub rate_limits: Option<serde_json::Value>,
    #[serde(default)]
    pub activity: RuntimeActivitySnapshot,
    #[serde(default)]
    pub health: RuntimeHealthView,
    #[serde(default)]
    pub summary: RuntimeSummaryView,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHealthStatus {
    Polling,
    Busy,
    Idle,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealthView {
    pub status: RuntimeHealthStatus,
    #[serde(default)]
    pub has_running_work: bool,
    #[serde(default)]
    pub has_retry_backlog: bool,
    #[serde(default)]
    pub poll_in_progress: bool,
    #[serde(default)]
    pub has_rate_limits: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_poll_started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_poll_completed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_poll_due_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_runtime_activity_at: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSummaryView {
    pub counts: String,
    pub tokens: String,
    pub activity: String,
    pub throughput: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limits: Option<String>,
}

impl RuntimeActivitySnapshot {
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            poll_in_progress: self.poll_in_progress,
            last_poll_started_at: self.last_poll_started_at,
            last_poll_completed_at: self.last_poll_completed_at,
            next_poll_due_at: self.next_poll_due_at,
            last_runtime_activity_at: self.last_runtime_activity_at,
            throughput: self.throughput.normalized(),
        }
    }
}

impl ThroughputSnapshot {
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            window_seconds: normalize_display_float(self.window_seconds),
            input_tokens_per_second: normalize_display_float(self.input_tokens_per_second),
            output_tokens_per_second: normalize_display_float(self.output_tokens_per_second),
            total_tokens_per_second: normalize_display_float(self.total_tokens_per_second),
        }
    }
}

fn normalize_display_float(value: f64) -> f64 {
    if value == 0.0 || value.abs() < f64::EPSILON {
        0.0
    } else {
        value
    }
}

fn format_one_decimal(value: f64) -> String {
    format!("{:.1}", normalize_display_float(value))
}

fn format_optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeActivitySnapshot, RuntimeHealthStatus, RuntimeSnapshot, ThroughputSnapshot,
    };

    #[test]
    fn runtime_spec_view_uses_counts_rate_limits_and_activity_payload() {
        let snapshot = RuntimeSnapshot {
            running: 2,
            retrying: 1,
            input_tokens: 10,
            output_tokens: 20,
            total_tokens: 30,
            seconds_running: 42.5,
            rate_limits: Some(serde_json::json!({
                "primary": {
                    "remaining": 12
                }
            })),
            activity: RuntimeActivitySnapshot {
                poll_in_progress: true,
                last_poll_started_at: Some(1_700_000_000),
                last_poll_completed_at: Some(1_700_000_001),
                next_poll_due_at: Some(1_700_000_060),
                last_runtime_activity_at: Some(1_700_000_002),
                throughput: ThroughputSnapshot {
                    window_seconds: 5.0,
                    input_tokens_per_second: 1.2,
                    output_tokens_per_second: 2.4,
                    total_tokens_per_second: 3.6,
                },
            },
        };

        let spec_view = snapshot.spec_view();
        assert_eq!(spec_view.counts.running, 2);
        assert_eq!(spec_view.counts.retrying, 1);
        assert_eq!(spec_view.codex_totals.input_tokens, 10);
        assert_eq!(spec_view.codex_totals.output_tokens, 20);
        assert_eq!(spec_view.codex_totals.total_tokens, 30);
        assert_eq!(spec_view.codex_totals.seconds_running, 42.5);
        assert!(spec_view.activity.poll_in_progress);
        assert_eq!(spec_view.activity.last_poll_started_at, Some(1_700_000_000));
        assert_eq!(
            spec_view.activity.last_poll_completed_at,
            Some(1_700_000_001)
        );
        assert_eq!(spec_view.activity.next_poll_due_at, Some(1_700_000_060));
        assert_eq!(
            spec_view.activity.last_runtime_activity_at,
            Some(1_700_000_002)
        );
        assert_eq!(spec_view.activity.throughput.window_seconds, 5.0);
        assert_eq!(spec_view.activity.throughput.total_tokens_per_second, 3.6);
        assert_eq!(spec_view.health.status, RuntimeHealthStatus::Polling);
        assert_eq!(
            spec_view.summary.counts,
            "running=2 retrying=1 total_active=3"
        );
        assert_eq!(
            spec_view.summary.tokens,
            "input_tokens=10 output_tokens=20 total_tokens=30 seconds_running=42.5"
        );
        assert_eq!(
            spec_view.summary.activity,
            "poll_in_progress=true last_poll_started_at=1700000000 last_poll_completed_at=1700000001 next_poll_due_at=1700000060 last_runtime_activity_at=1700000002"
        );
        assert_eq!(
            spec_view.summary.throughput,
            "window_seconds=5.0 input_tps=1.2 output_tps=2.4 total_tps=3.6"
        );
        assert_eq!(
            spec_view.summary.rate_limits.as_deref(),
            Some(r#"{"primary":{"remaining":12}}"#)
        );
        assert_eq!(
            spec_view.rate_limits,
            Some(serde_json::json!({
                "primary": {
                    "remaining": 12
                }
            }))
        );
    }

    #[test]
    fn runtime_snapshot_normalizes_negative_zero_values() {
        let snapshot = RuntimeSnapshot {
            running: 0,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: -0.0,
            rate_limits: None,
            activity: RuntimeActivitySnapshot {
                poll_in_progress: false,
                last_poll_started_at: None,
                last_poll_completed_at: None,
                next_poll_due_at: None,
                last_runtime_activity_at: None,
                throughput: ThroughputSnapshot {
                    window_seconds: -0.0,
                    input_tokens_per_second: -0.0,
                    output_tokens_per_second: -0.0,
                    total_tokens_per_second: -0.0,
                },
            },
        };

        let normalized = snapshot.normalized();
        assert_eq!(normalized.seconds_running, 0.0);
        assert_eq!(normalized.activity.throughput.window_seconds, 0.0);
        assert_eq!(normalized.activity.throughput.total_tokens_per_second, 0.0);
        assert_eq!(
            normalized.summary().tokens,
            "input_tokens=0 output_tokens=0 total_tokens=0 seconds_running=0.0"
        );
        assert_eq!(
            normalized.summary().throughput,
            "window_seconds=0.0 input_tps=0.0 output_tps=0.0 total_tps=0.0"
        );
    }

    #[test]
    fn runtime_snapshot_sanitizes_rate_limits_payload() {
        let snapshot = RuntimeSnapshot {
            running: 1,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: 0.0,
            rate_limits: Some(serde_json::json!({
                "zeta": 1,
                "authorization": "Bearer top-secret",
                "nested": {
                    "api_key": "secret-key"
                }
            })),
            activity: RuntimeActivitySnapshot::default(),
        };

        let spec_view = snapshot.spec_view();
        assert_eq!(
            spec_view.rate_limits,
            Some(serde_json::json!({
                "zeta": 1,
                "authorization": "[REDACTED]",
                "nested": {
                    "api_key": "[REDACTED]"
                }
            }))
        );
        assert_eq!(
            spec_view.summary.rate_limits.as_deref(),
            Some(r#"{"authorization":"[REDACTED]","nested":{"api_key":"[REDACTED]"},"zeta":1}"#)
        );
    }

    #[test]
    fn runtime_health_marks_idle_runtime_without_work() {
        let snapshot = RuntimeSnapshot {
            running: 0,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: 0.0,
            rate_limits: None,
            activity: RuntimeActivitySnapshot {
                poll_in_progress: false,
                last_poll_started_at: Some(1_700_000_000),
                last_poll_completed_at: Some(1_700_000_001),
                next_poll_due_at: Some(1_700_000_060),
                last_runtime_activity_at: Some(1_700_000_002),
                throughput: ThroughputSnapshot::default(),
            },
        };

        let health = snapshot.health();
        assert_eq!(health.status, RuntimeHealthStatus::Idle);
        assert!(!health.has_running_work);
        assert!(!health.has_retry_backlog);
        assert!(!health.poll_in_progress);
    }
}
