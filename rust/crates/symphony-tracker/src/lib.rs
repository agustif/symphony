#![forbid(unsafe_code)]

mod error;
mod issue;
mod state;
mod tracker_client;

pub use error::TrackerError;
pub use issue::TrackerIssue;
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
