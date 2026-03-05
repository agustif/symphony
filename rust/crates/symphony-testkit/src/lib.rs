#![forbid(unsafe_code)]

mod issue_builders;
mod observability_builders;
mod path_builders;
mod protocol_builders;

pub use issue_builders::issue_id;
pub use observability_builders::{issue_snapshot, runtime_snapshot, state_snapshot};
pub use path_builders::test_cwd;
pub use protocol_builders::{protocol_stderr_line, protocol_stdout_line};

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_observability::StateSnapshot;

    #[test]
    fn helper_builds_issue_id() {
        let id = issue_id("SYM-7");
        assert_eq!(id.0, "SYM-7");
    }

    #[test]
    fn helper_builds_state_snapshot() {
        let snapshot = state_snapshot(
            runtime_snapshot(1, 0),
            vec![issue_snapshot("SYM-8", "SYM-8", "Running", 0)],
        );
        assert_eq!(snapshot.runtime.running, 1);
        assert_eq!(snapshot.issues.len(), 1);
    }

    #[test]
    fn helper_builds_protocol_stdout_line() {
        let line = protocol_stdout_line("turn.start");
        assert!(line.contains("\"method\":\"turn.start\""));
    }

    #[test]
    fn helper_builds_test_cwd() {
        let cwd = test_cwd("/tmp/symphony");
        assert_eq!(cwd, std::path::PathBuf::from("/tmp/symphony"));
    }

    #[test]
    fn state_snapshot_default_roundtrip() {
        let snapshot = StateSnapshot::default();
        assert_eq!(snapshot.runtime.running, 0);
    }
}
