//! Fake data generators for testing using the `fake` crate.

use fake::{Fake, faker::lorem::en::Sentence};
use symphony_domain::IssueId;
use symphony_tracker::{TrackerBlockerRef, TrackerIssue, TrackerState};

/// Generate a fake IssueId
pub fn fake_issue_id() -> IssueId {
    IssueId(format!("lin_{}", uuid::Uuid::new_v4().simple()))
}

/// Generate a fake issue identifier (e.g., "SYM-123")
pub fn fake_issue_identifier() -> String {
    let num: u16 = (1..999).fake();
    format!("SYM-{}", num)
}

/// Generate a fake TrackerIssue with realistic data
pub fn fake_tracker_issue() -> TrackerIssue {
    fake_tracker_issue_with_state("Todo")
}

/// Generate a fake TrackerIssue in a specific state
pub fn fake_tracker_issue_with_state(state: &str) -> TrackerIssue {
    let identifier = fake_issue_identifier();
    let title: String = Sentence(3..8).fake();

    TrackerIssue::new(fake_issue_id(), identifier, TrackerState::new(state))
        .with_title(title)
        .with_description(Sentence(10..20).fake::<String>())
}

/// Generate multiple fake issues
pub fn fake_tracker_issues(count: usize) -> Vec<TrackerIssue> {
    (0..count).map(|_| fake_tracker_issue()).collect()
}

/// Generate a fake blocker reference
pub fn fake_blocker_ref() -> TrackerBlockerRef {
    TrackerBlockerRef {
        id: Some(fake_issue_id().0),
        identifier: Some(fake_issue_identifier()),
        state: Some("In Progress".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_issue_id_generates_unique_ids() {
        let id1 = fake_issue_id();
        let id2 = fake_issue_id();
        assert_ne!(id1.0, id2.0);
        assert!(id1.0.starts_with("lin_"));
    }

    #[test]
    fn fake_tracker_issue_generates_realistic_data() {
        let issue = fake_tracker_issue();
        assert!(!issue.identifier.is_empty());
        assert!(issue.identifier.starts_with("SYM-"));
        assert!(!issue.title.is_empty());
        assert!(issue.description.is_some());
    }

    #[test]
    fn fake_tracker_issues_generates_multiple() {
        let issues = fake_tracker_issues(5);
        assert_eq!(issues.len(), 5);
        // All should have unique IDs
        let ids: std::collections::HashSet<_> = issues.iter().map(|i| &i.id.0).collect();
        assert_eq!(ids.len(), 5);
    }
}
