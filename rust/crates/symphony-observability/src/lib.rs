#![forbid(unsafe_code)]

mod issue_snapshot;
mod runtime_snapshot;
mod sanitization;
mod state_snapshot;

pub use issue_snapshot::IssueSnapshot;
pub use issue_snapshot::IssueTaskMapKind;
pub use runtime_snapshot::{
    CodexTotalsSnapshot, RuntimeActivitySnapshot, RuntimeCounts, RuntimeSnapshot, RuntimeSpecView,
    ThroughputSnapshot,
};
pub use sanitization::{sanitize_event_text, sanitize_message_text, strip_control_bytes};
pub use state_snapshot::{
    IssueStatusTotalsSnapshot, SnapshotErrorCode, SnapshotErrorView, SnapshotStatus, StateSnapshot,
    StateSnapshotEnvelope, StateSpecView, TaskMapSnapshot,
};

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_domain::IssueId;

    #[test]
    fn runtime_snapshot_smoke() {
        let snapshot = RuntimeSnapshot {
            running: 0,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: 0.0,
            rate_limits: None,
            activity: RuntimeActivitySnapshot::default(),
        };
        assert_eq!(snapshot.running + snapshot.retrying, 0);
        assert_eq!(snapshot.spec_view().counts.running, 0);
        assert_eq!(
            snapshot
                .spec_view()
                .activity
                .throughput
                .total_tokens_per_second,
            0.0
        );
    }

    #[test]
    fn state_snapshot_resolves_issue_by_id() {
        let snapshot = StateSnapshot {
            runtime: RuntimeSnapshot {
                running: 1,
                retrying: 0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                seconds_running: 0.0,
                rate_limits: None,
                activity: RuntimeActivitySnapshot::default(),
            },
            issues: vec![IssueSnapshot {
                id: IssueId("SYM-42".to_owned()),
                identifier: "SYM-42".to_owned(),
                state: "Running".to_owned(),
                retry_attempts: 2,
                workspace_path: None,
                session_id: None,
                turn_count: 0,
                last_event: None,
                last_message: None,
                started_at: None,
                last_event_at: None,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                retry_due_at: None,
                retry_error: None,
            }],
        };

        let issue = snapshot.issue("SYM-42").expect("issue should exist");
        assert_eq!(issue.retry_attempts, 2);
        let by_identifier = snapshot
            .issue_by_id_or_identifier("SYM-42")
            .expect("issue identifier should resolve");
        assert_eq!(by_identifier.identifier, "SYM-42");
        assert_eq!(snapshot.spec_view().issue_totals.total, 1);
    }

    #[test]
    fn sanitization_helpers_strip_controls() {
        let raw = "msg\u{0000}\u{001b}[31m text";
        assert_eq!(sanitize_event_text(raw), "msg text");
        assert_eq!(sanitize_message_text(raw), "msg text");
    }
}
