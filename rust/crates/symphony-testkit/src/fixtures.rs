use symphony_domain::{IssueId, OrchestratorState, RetryEntry};

pub fn issue_id(value: &str) -> IssueId {
    IssueId(value.to_owned())
}

pub fn issue_ids(prefix: &str, count: usize) -> Vec<IssueId> {
    (1..=count)
        .map(|index| issue_id(&format!("{prefix}-{index}")))
        .collect()
}

pub fn orchestrator_state_from_labels(
    claimed: &[&str],
    running: &[&str],
    retry_attempts: &[(&str, u32)],
) -> OrchestratorState {
    let mut state = OrchestratorState::default();

    for label in claimed {
        state.claimed.insert(issue_id(label));
    }

    for label in running {
        let id = issue_id(label);
        state.running.insert(id.clone());
        state.claimed.insert(id);
    }

    for (label, attempt) in retry_attempts {
        let id = issue_id(label);
        state.running.remove(&id);
        state.claimed.insert(id.clone());
        state
            .retry_attempts
            .insert(id, RetryEntry { attempt: *attempt });
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_domain::validate_invariants;

    #[test]
    fn seeded_state_builder_normalizes_running_into_claimed() {
        let state = orchestrator_state_from_labels(&["SYM-1"], &["SYM-2"], &[("SYM-3", 1)]);
        assert_eq!(state.claimed.len(), 3);
        assert_eq!(state.running.len(), 1);
        assert_eq!(state.retry_attempts.len(), 1);
        assert_eq!(validate_invariants(&state), Ok(()));
    }
}
