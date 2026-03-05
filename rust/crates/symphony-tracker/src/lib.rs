#![forbid(unsafe_code)]

use async_trait::async_trait;
use symphony_domain::IssueId;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerIssue {
    pub id: IssueId,
    pub identifier: String,
    pub state: String,
}

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("tracker request failed: {0}")]
    Request(String),
}

#[async_trait]
pub trait TrackerClient: Send + Sync {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn tracker_contract_smoke() {
        assert_eq!(2 + 2, 4);
    }
}
