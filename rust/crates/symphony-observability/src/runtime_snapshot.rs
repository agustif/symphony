use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub running: usize,
    pub retrying: usize,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
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
                seconds_running: 0.0,
            },
            rate_limits: None,
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
    pub seconds_running: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitSnapshot {
    #[serde(default)]
    pub requests_remaining: Option<u64>,
    #[serde(default)]
    pub tokens_remaining: Option<u64>,
    #[serde(default)]
    pub reset_seconds: Option<u64>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSpecView {
    pub counts: RuntimeCounts,
    pub codex_totals: CodexTotalsSnapshot,
    #[serde(default)]
    pub rate_limits: Option<RateLimitSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::RuntimeSnapshot;

    #[test]
    fn runtime_spec_view_uses_counts_and_default_placeholders() {
        let snapshot = RuntimeSnapshot {
            running: 2,
            retrying: 1,
            input_tokens: 10,
            output_tokens: 20,
            total_tokens: 30,
        };

        let spec_view = snapshot.spec_view();
        assert_eq!(spec_view.counts.running, 2);
        assert_eq!(spec_view.counts.retrying, 1);
        assert_eq!(spec_view.codex_totals.input_tokens, 10);
        assert_eq!(spec_view.codex_totals.output_tokens, 20);
        assert_eq!(spec_view.codex_totals.total_tokens, 30);
        assert_eq!(spec_view.codex_totals.seconds_running, 0.0);
        assert_eq!(spec_view.rate_limits, None);
    }
}
