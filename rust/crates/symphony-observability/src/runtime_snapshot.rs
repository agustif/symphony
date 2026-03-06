use serde::{Deserialize, Serialize};

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
    pub fn spec_view(&self) -> RuntimeSpecView {
        RuntimeSpecView {
            counts: RuntimeCounts {
                running: self.running,
                retrying: self.retrying,
            },
            codex_totals: CodexTotalsSnapshot {
                input_tokens: self.input_tokens,
                output_tokens: self.output_tokens,
                total_tokens: self.total_tokens,
                seconds_running: self.seconds_running,
            },
            rate_limits: self.rate_limits.clone(),
            activity: self.activity.clone(),
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
}

#[cfg(test)]
mod tests {
    use super::{RuntimeActivitySnapshot, RuntimeSnapshot, ThroughputSnapshot};

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
        assert_eq!(
            spec_view.rate_limits,
            Some(serde_json::json!({
                "primary": {
                    "remaining": 12
                }
            }))
        );
    }
}
