#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub running: usize,
    pub retrying: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_smoke() {
        let snap = RuntimeSnapshot {
            running: 0,
            retrying: 0,
        };
        assert_eq!(snap.running + snap.retrying, 0);
    }
}
