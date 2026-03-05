#![forbid(unsafe_code)]

use symphony_observability::RuntimeSnapshot;

pub fn snapshot_to_json(snapshot: &RuntimeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "running": snapshot.running,
        "retrying": snapshot.retrying,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snapshot_shape() {
        let value = snapshot_to_json(&RuntimeSnapshot {
            running: 3,
            retrying: 1,
        });
        assert_eq!(value["running"], 3);
        assert_eq!(value["retrying"], 1);
    }
}
