use serde::{Deserialize, Serialize};
use symphony_domain::IssueId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSnapshot {
    pub id: IssueId,
    pub identifier: String,
    pub state: String,
    pub retry_attempts: u32,
}
