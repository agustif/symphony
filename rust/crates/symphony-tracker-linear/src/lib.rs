#![forbid(unsafe_code)]

use async_trait::async_trait;
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue};

#[derive(Clone, Debug)]
pub struct LinearTracker;

#[async_trait]
impl TrackerClient for LinearTracker {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn linear_tracker_returns_empty_candidates_by_default() {
        use symphony_tracker::TrackerClient;

        let client = super::LinearTracker;
        let issues = client
            .fetch_candidates()
            .await
            .expect("tracker call should succeed");
        assert!(issues.is_empty());
    }
}
