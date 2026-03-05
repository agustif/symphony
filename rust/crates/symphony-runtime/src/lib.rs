#![forbid(unsafe_code)]

use std::sync::Arc;

use symphony_observability::RuntimeSnapshot;
use symphony_tracker::TrackerClient;

pub struct Runtime<T: TrackerClient> {
    tracker: Arc<T>,
}

impl<T: TrackerClient> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        Self { tracker }
    }

    pub async fn snapshot(&self) -> RuntimeSnapshot {
        let running = self
            .tracker
            .fetch_candidates()
            .await
            .map_or(0, |issues| issues.len());
        RuntimeSnapshot {
            running,
            retrying: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue};

    use super::*;

    struct StaticTracker;

    #[async_trait]
    impl TrackerClient for StaticTracker {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn runtime_reports_zero_when_no_candidates() {
        let runtime = Runtime::new(Arc::new(StaticTracker));
        let snapshot = runtime.snapshot().await;
        assert_eq!(snapshot.running, 0);
    }
}
