//! # symphony-tracker
//!
//! Tracker client trait and domain types for issue tracker integration.
//!
//! ## Core Contract Fields
//!
//! The `TrackerIssue` struct provides fields used by the orchestrator for
//! routing and orchestration decisions:
//!
//! - `id`: Unique issue identifier (e.g., Linear UUID)
//! - `identifier`: Human-readable key (e.g., "SYM-123")
//! - `title`: Issue title for display and logging
//! - `state`: Current workflow state (e.g., "Todo", "In Progress")
//! - `priority`: Numeric priority for ordering
//! - `labels`: Tags for categorization and filtering
//! - `blocked_by`: References to blocking issues
//! - `created_at`/`updated_at`: Timestamps for staleness detection
//!
//! ## Routing-Only Fields
//!
//! The following fields are provided for external routing (e.g., branch name
//! derivation, URL construction) but are NOT used by the orchestrator for
//! state transitions or orchestration decisions:
//!
//! - `url`: Issue URL in the tracker (for display/links only)
//! - `branch_name`: Suggested branch name (for external tooling)
//! - `description`: Issue body/description (for display only)
//!
//! ## Read-Only Contract
//!
//! All fields in `TrackerIssue` are read-only from the orchestrator's
//! perspective. State mutations are performed via the tracker client's
//! state transition methods, not by modifying `TrackerIssue` directly.

#![forbid(unsafe_code)]

mod error;
mod issue;
mod state;
mod tracker_client;

pub use error::TrackerError;
pub use issue::{TrackerBlockerRef, TrackerIssue};
pub use state::TrackerState;
pub use tracker_client::TrackerClient;

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use symphony_domain::IssueId;

    use crate::{TrackerClient, TrackerError, TrackerIssue, TrackerState};

    struct StaticTracker {
        issues: Vec<TrackerIssue>,
    }

    #[async_trait]
    impl TrackerClient for StaticTracker {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(self.issues.clone())
        }
    }

    #[tokio::test]
    async fn fetch_candidates_by_states_filters_default_candidates() {
        let tracker = StaticTracker {
            issues: vec![
                TrackerIssue::new(
                    IssueId("lin_1".to_owned()),
                    "SYM-1",
                    TrackerState::new("Todo"),
                ),
                TrackerIssue::new(
                    IssueId("lin_2".to_owned()),
                    "SYM-2",
                    TrackerState::new("Done"),
                ),
            ],
        };
        let issues = tracker
            .fetch_candidates_by_states(&[TrackerState::new("Todo")])
            .await
            .expect("state-filtered query should succeed");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].identifier, "SYM-1");
    }

    #[tokio::test]
    async fn fetch_candidates_by_states_uses_normalized_state_matching() {
        let tracker = StaticTracker {
            issues: vec![TrackerIssue::new(
                IssueId("lin_1".to_owned()),
                "SYM-1",
                TrackerState::new("  In Progress  "),
            )],
        };
        let issues = tracker
            .fetch_candidates_by_states(&[TrackerState::new("in progress")])
            .await
            .expect("state-filtered query should normalize and match state names");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].identifier, "SYM-1");
    }

    #[tokio::test]
    async fn fetch_states_by_ids_returns_subset_for_requested_ids() {
        let tracker = StaticTracker {
            issues: vec![
                TrackerIssue::new(
                    IssueId("lin_1".to_owned()),
                    "SYM-1",
                    TrackerState::new("Todo"),
                ),
                TrackerIssue::new(
                    IssueId("lin_2".to_owned()),
                    "SYM-2",
                    TrackerState::new("In Progress"),
                ),
            ],
        };
        let states = tracker
            .fetch_states_by_ids(&[IssueId("lin_2".to_owned())])
            .await
            .expect("state map query should succeed");
        assert_eq!(states.len(), 1);
        assert_eq!(
            states.get(&IssueId("lin_2".to_owned())),
            Some(&TrackerState::new("In Progress"))
        );
    }

    #[tokio::test]
    async fn fetch_terminal_candidates_filters_by_terminal_states() {
        let tracker = StaticTracker {
            issues: vec![
                TrackerIssue::new(
                    IssueId("lin_1".to_owned()),
                    "SYM-1",
                    TrackerState::new("Todo"),
                ),
                TrackerIssue::new(
                    IssueId("lin_2".to_owned()),
                    "SYM-2",
                    TrackerState::new("Done"),
                ),
            ],
        };
        let terminal = tracker
            .fetch_terminal_candidates(&[TrackerState::new(" done ")])
            .await
            .expect("terminal state lookup should succeed");
        assert_eq!(terminal.len(), 1);
        assert_eq!(terminal[0].id, IssueId("lin_2".to_owned()));
    }

    #[tokio::test]
    async fn fetch_terminal_states_by_ids_returns_only_terminal_matches() {
        let tracker = StaticTracker {
            issues: vec![
                TrackerIssue::new(
                    IssueId("lin_1".to_owned()),
                    "SYM-1",
                    TrackerState::new("Todo"),
                ),
                TrackerIssue::new(
                    IssueId("lin_2".to_owned()),
                    "SYM-2",
                    TrackerState::new("Done"),
                ),
            ],
        };
        let states = tracker
            .fetch_terminal_states_by_ids(
                &[IssueId("lin_1".to_owned()), IssueId("lin_2".to_owned())],
                &[TrackerState::new("done")],
            )
            .await
            .expect("terminal state refresh should succeed");
        assert_eq!(states.len(), 1);
        assert_eq!(
            states.get(&IssueId("lin_2".to_owned())),
            Some(&TrackerState::new("Done"))
        );
    }
}

#[cfg(test)]
mod error_mapping {
    use crate::TrackerError;

    #[test]
    fn transport_error_constructor_and_display() {
        let err = TrackerError::transport("connection refused");
        let display = format!("{}", err);
        assert!(display.contains("connection refused"));
        assert!(display.contains("transport error"));
    }

    #[test]
    fn status_error_constructor_and_display() {
        let err = TrackerError::status(404, "not found");
        let display = format!("{}", err);
        assert!(display.contains("404"));
        assert!(display.contains("not found"));
    }

    #[test]
    fn graphql_error_constructor_and_display() {
        let err = TrackerError::graphql(vec!["field X is required".to_string()]);
        let display = format!("{}", err);
        assert!(display.contains("field X is required"));
    }

    #[test]
    fn payload_error_constructor_and_display() {
        let err = TrackerError::payload("unexpected JSON structure");
        let display = format!("{}", err);
        assert!(display.contains("unexpected JSON structure"));
        assert!(display.contains("payload error"));
    }

    #[test]
    fn error_equality() {
        let err1 = TrackerError::transport("msg");
        let err2 = TrackerError::transport("msg");
        let err3 = TrackerError::transport("other");
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
