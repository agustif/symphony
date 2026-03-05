#![forbid(unsafe_code)]

mod issue_snapshot;
mod runtime_snapshot;
mod state_snapshot;

pub use issue_snapshot::IssueSnapshot;
pub use runtime_snapshot::RuntimeSnapshot;
pub use state_snapshot::StateSnapshot;

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_domain::IssueId;

    #[test]
    fn runtime_snapshot_smoke() {
        let snapshot = RuntimeSnapshot {
            running: 0,
            retrying: 0,
        };
        assert_eq!(snapshot.running + snapshot.retrying, 0);
    }

    #[test]
    fn state_snapshot_resolves_issue_by_id() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 1,
                retrying: 0,
            },
            issues: vec![IssueSnapshot {
                id: IssueId("SYM-42".to_owned()),
                identifier: "SYM-42".to_owned(),
                state: "Running".to_owned(),
                retry_attempts: 2,
            }],
        };

        let issue = snapshot.issue("SYM-42").expect("issue should exist");
        assert_eq!(issue.retry_attempts, 2);
    }
}
