use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub running: usize,
    pub retrying: usize,
}
