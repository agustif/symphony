use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use symphony_domain::{IssueId, OrchestratorState, RetryEntry, RunningEntry};

/// Snapshot of orchestrator state for testing and comparison
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateSnapshot {
    pub claimed: Vec<String>,
    pub running: Vec<String>,
    pub retry_attempts: Vec<(String, u32)>,
}

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
        state.running.insert(id.clone(), RunningEntry::default());
        state.claimed.insert(id);
    }

    for (label, attempt) in retry_attempts {
        let id = issue_id(label);
        state.running.remove(&id);
        state.claimed.insert(id.clone());
        state
            .retry_attempts
            .insert(id.clone(), RetryEntry { attempt: *attempt });
    }

    state
}

/// Load a fixture file from fixtures directory
pub fn load_fixture<P: AsRef<Path>>(relative_path: P) -> Result<String, FixtureError> {
    let path = resolve_fixture_path(relative_path)?;
    fs::read_to_string(&path).map_err(|e| FixtureError::IoError {
        path: path.clone(),
        source: e,
    })
}

/// Load a fixture file and apply variable substitution
pub fn load_fixture_with_vars<P: AsRef<Path>>(
    relative_path: P,
    variables: &HashMap<String, String>,
) -> Result<String, FixtureError> {
    let content = load_fixture(relative_path)?;
    let mut result = content;

    for (key, value) in variables {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }

    Ok(result)
}

/// Load JSON fixture and deserialize
pub fn load_json_fixture<P: AsRef<Path>, T: serde::de::DeserializeOwned>(
    relative_path: P,
) -> Result<T, FixtureError> {
    let content = load_fixture(relative_path)?;
    serde_json::from_str(&content).map_err(FixtureError::JsonError)
}

/// Load JSON fixture with variable substitution
pub fn load_json_fixture_with_vars<P: AsRef<Path>, T: serde::de::DeserializeOwned>(
    relative_path: P,
    variables: &HashMap<String, String>,
) -> Result<T, FixtureError> {
    let content = load_fixture_with_vars(relative_path, variables)?;
    serde_json::from_str(&content).map_err(FixtureError::JsonError)
}

/// Resolve a fixture path relative to the fixtures directory
pub fn resolve_fixture_path<P: AsRef<Path>>(relative_path: P) -> Result<PathBuf, FixtureError> {
    // Start from the crate root and look for fixtures
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.push("fixtures");
    base.push(relative_path);

    if !base.exists() {
        return Err(FixtureError::NotFound(base));
    }

    Ok(base)
}

/// Compare two snapshots and report differences
pub fn compare_snapshots(expected: &StateSnapshot, actual: &StateSnapshot) -> SnapshotComparison {
    let mut differences = Vec::new();

    // Compare claimed sets
    let expected_claimed: HashSet<_> = expected.claimed.iter().collect();
    let actual_claimed: HashSet<_> = actual.claimed.iter().collect();

    for missing in expected_claimed.difference(&actual_claimed) {
        differences.push(SnapshotDifference::MissingClaimed((*missing).clone()));
    }
    for extra in actual_claimed.difference(&expected_claimed) {
        differences.push(SnapshotDifference::ExtraClaimed((*extra).clone()));
    }

    // Compare running sets
    let expected_running: HashSet<_> = expected.running.iter().collect();
    let actual_running: HashSet<_> = actual.running.iter().collect();

    for missing in expected_running.difference(&actual_running) {
        differences.push(SnapshotDifference::MissingRunning((*missing).clone()));
    }
    for extra in actual_running.difference(&expected_running) {
        differences.push(SnapshotDifference::ExtraRunning((*extra).clone()));
    }

    // Compare retry attempts
    let expected_attempts: HashMap<_, _> = expected
        .retry_attempts
        .iter()
        .map(|(id, attempt)| (id.clone(), *attempt))
        .collect();
    let actual_attempts: HashMap<_, _> = actual
        .retry_attempts
        .iter()
        .map(|(id, attempt)| (id.clone(), *attempt))
        .collect();

    for (id, expected_attempt) in &expected_attempts {
        match actual_attempts.get(id) {
            Some(actual_attempt) if actual_attempt != expected_attempt => {
                differences.push(SnapshotDifference::MismatchedRetryAttempt {
                    id: id.clone(),
                    expected: *expected_attempt,
                    actual: *actual_attempt,
                });
            }
            None => {
                differences.push(SnapshotDifference::MissingRetryAttempt {
                    id: id.clone(),
                    attempt: *expected_attempt,
                });
            }
            _ => {}
        }
    }

    for (id, actual_attempt) in &actual_attempts {
        if !expected_attempts.contains_key(id) {
            differences.push(SnapshotDifference::ExtraRetryAttempt {
                id: id.clone(),
                attempt: *actual_attempt,
            });
        }
    }

    SnapshotComparison {
        matches: differences.is_empty(),
        differences,
    }
}

/// Result of snapshot comparison
#[derive(Debug, PartialEq, Eq)]
pub struct SnapshotComparison {
    pub matches: bool,
    pub differences: Vec<SnapshotDifference>,
}

/// Individual differences in snapshot comparison
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotDifference {
    MissingClaimed(String),
    ExtraClaimed(String),
    MissingRunning(String),
    ExtraRunning(String),
    MissingRetryAttempt {
        id: String,
        attempt: u32,
    },
    ExtraRetryAttempt {
        id: String,
        attempt: u32,
    },
    MismatchedRetryAttempt {
        id: String,
        expected: u32,
        actual: u32,
    },
}

impl SnapshotDifference {
    pub fn description(&self) -> String {
        match self {
            SnapshotDifference::MissingClaimed(id) => format!("Missing claimed issue: {}", id),
            SnapshotDifference::ExtraClaimed(id) => format!("Extra claimed issue: {}", id),
            SnapshotDifference::MissingRunning(id) => format!("Missing running issue: {}", id),
            SnapshotDifference::ExtraRunning(id) => format!("Extra running issue: {}", id),
            SnapshotDifference::MissingRetryAttempt { id, attempt } => {
                format!("Missing retry attempt for {}: attempt {}", id, attempt)
            }
            SnapshotDifference::ExtraRetryAttempt { id, attempt } => {
                format!("Extra retry attempt for {}: attempt {}", id, attempt)
            }
            SnapshotDifference::MismatchedRetryAttempt {
                id,
                expected,
                actual,
            } => {
                format!(
                    "Mismatched retry attempt for {}: expected {}, got {}",
                    id, expected, actual
                )
            }
        }
    }
}

/// Errors that can occur when loading fixtures
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("Fixture not found: {0}")]
    NotFound(PathBuf),

    #[error("IO error reading fixture at {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error parsing fixture: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Variable substitution error: {0}")]
    SubstitutionError(String),
}

/// Helper for building fixture test data
pub struct FixtureBuilder {
    base_path: PathBuf,
    variables: HashMap<String, String>,
}

impl FixtureBuilder {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            variables: HashMap::new(),
        }
    }

    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    pub fn with_variables(mut self, vars: HashMap<String, String>) -> Self {
        self.variables.extend(vars);
        self
    }

    pub fn load(&self, relative_path: impl AsRef<Path>) -> Result<String, FixtureError> {
        let full_path = self.base_path.join(relative_path);
        load_fixture(&full_path)
    }

    pub fn load_with_vars(&self, relative_path: impl AsRef<Path>) -> Result<String, FixtureError> {
        let full_path = self.base_path.join(relative_path);
        load_fixture_with_vars(&full_path, &self.variables)
    }

    pub fn load_json<T: serde::de::DeserializeOwned>(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<T, FixtureError> {
        let full_path = self.base_path.join(relative_path);
        load_json_fixture(&full_path)
    }

    pub fn load_json_with_vars<T: serde::de::DeserializeOwned>(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<T, FixtureError> {
        let full_path = self.base_path.join(relative_path);
        load_json_fixture_with_vars(&full_path, &self.variables)
    }
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self::new("fixtures")
    }
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

    #[test]
    fn snapshot_comparison_detects_differences() {
        let expected = StateSnapshot {
            claimed: vec!["SYM-1".to_string(), "SYM-2".to_string()],
            running: vec!["SYM-1".to_string()],
            retry_attempts: vec![("SYM-2".to_string(), 1)],
        };

        let actual = StateSnapshot {
            claimed: vec!["SYM-1".to_string(), "SYM-3".to_string()],
            running: vec!["SYM-1".to_string()],
            retry_attempts: vec![("SYM-2".to_string(), 2)],
        };

        let comparison = compare_snapshots(&expected, &actual);

        assert!(!comparison.matches);
        assert_eq!(comparison.differences.len(), 3);
    }

    #[test]
    fn snapshot_comparison_matches_identical_snapshots() {
        let snapshot = StateSnapshot {
            claimed: vec!["SYM-1".to_string()],
            running: vec![],
            retry_attempts: vec![],
        };

        let comparison = compare_snapshots(&snapshot, &snapshot);

        assert!(comparison.matches);
        assert!(comparison.differences.is_empty());
    }

    #[test]
    fn fixture_builder_supports_variable_substitution() {
        let builder = FixtureBuilder::new(".")
            .with_variable("ID", "SYM-123")
            .with_variable("STATE", "Running");

        // Test that variables are stored correctly
        assert_eq!(builder.variables.get("ID"), Some(&"SYM-123".to_string()));
        assert_eq!(builder.variables.get("STATE"), Some(&"Running".to_string()));
    }
}
