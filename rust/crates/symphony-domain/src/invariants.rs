//! Executable invariant catalog for the pure reducer state.
//!
//! The catalog is intentionally explicit so production code, tests, and Verus
//! proofs can all point at the same named invariants instead of duplicating
//! string literals in separate layers.

use crate::{InvariantError, OrchestratorState};

pub(crate) type InvariantCheck = fn(&OrchestratorState) -> Result<(), InvariantError>;

const INVARIANT_CHECKS: [InvariantCheck; 4] = [
    validate_running_requires_claim,
    validate_retry_requires_claim,
    validate_running_and_retry_are_disjoint,
    validate_retry_attempts_are_positive,
];

/// Human-readable metadata for one executable reducer invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvariantDescriptor {
    /// Stable machine-facing code used in diagnostics and operator payloads.
    pub code: &'static str,
    /// Short title used in architecture, proof, and test traceability docs.
    pub summary: &'static str,
    /// Longer description of the state property enforced by the reducer.
    pub description: &'static str,
    /// SPEC sections that currently rely on this invariant.
    pub spec_sections: &'static [&'static str],
    /// Verus proof modules that currently model or prove this invariant.
    pub proof_modules: &'static [&'static str],
}

const INVARIANT_CATALOG: [InvariantDescriptor; 4] = [
    InvariantDescriptor {
        code: "running_without_claim",
        summary: "Running implies claimed",
        description: "Every issue in the running map must also remain present in the claimed set.",
        spec_sections: &["Sec. 7", "Sec. 8", "Sec. 16"],
        proof_modules: &["proofs/verus/specs/runtime_quick.rs"],
    },
    InvariantDescriptor {
        code: "retry_without_claim",
        summary: "Retry implies claimed",
        description: "Every issue queued for retry must remain present in the claimed set until release.",
        spec_sections: &["Sec. 7", "Sec. 8", "Sec. 16"],
        proof_modules: &["proofs/verus/specs/runtime_quick.rs"],
    },
    InvariantDescriptor {
        code: "running_and_retrying",
        summary: "Running and retry are disjoint",
        description: "An issue cannot be simultaneously marked running and queued for retry.",
        spec_sections: &["Sec. 7", "Sec. 8"],
        proof_modules: &[
            "proofs/verus/specs/runtime_quick.rs",
            "proofs/verus/specs/session_liveness.rs",
        ],
    },
    InvariantDescriptor {
        code: "retry_attempt_must_be_positive",
        summary: "Retry attempts stay positive",
        description: "Stored retry attempts must always be strictly positive in stable reducer state.",
        spec_sections: &["Sec. 8", "Sec. 16"],
        proof_modules: &["proofs/verus/specs/runtime_quick.rs"],
    },
];

/// Returns the ordered invariant catalog used by runtime assertions and docs.
#[must_use]
pub fn invariant_catalog() -> &'static [InvariantDescriptor] {
    &INVARIANT_CATALOG
}

/// Looks up a catalog entry by diagnostic code.
#[must_use]
pub fn invariant_descriptor(code: &str) -> Option<&'static InvariantDescriptor> {
    invariant_catalog()
        .iter()
        .find(|descriptor| descriptor.code == code)
}

pub(crate) fn validate_all(state: &OrchestratorState) -> Result<(), InvariantError> {
    for check in INVARIANT_CHECKS {
        check(state)?;
    }
    Ok(())
}

fn validate_running_requires_claim(state: &OrchestratorState) -> Result<(), InvariantError> {
    if state
        .running
        .keys()
        .any(|issue_id| !state.claimed.contains(issue_id))
    {
        return Err(InvariantError::RunningWithoutClaim);
    }

    Ok(())
}

fn validate_retry_requires_claim(state: &OrchestratorState) -> Result<(), InvariantError> {
    if state
        .retry_attempts
        .keys()
        .any(|issue_id| !state.claimed.contains(issue_id))
    {
        return Err(InvariantError::RetryWithoutClaim);
    }

    Ok(())
}

fn validate_running_and_retry_are_disjoint(
    state: &OrchestratorState,
) -> Result<(), InvariantError> {
    if state
        .running
        .keys()
        .any(|issue_id| state.retry_attempts.contains_key(issue_id))
    {
        return Err(InvariantError::RunningAndRetrying);
    }

    Ok(())
}

fn validate_retry_attempts_are_positive(state: &OrchestratorState) -> Result<(), InvariantError> {
    if state
        .retry_attempts
        .values()
        .any(|retry_entry| retry_entry.attempt == 0)
    {
        return Err(InvariantError::RetryAttemptMustBePositive);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{invariant_catalog, invariant_descriptor};
    use std::collections::HashSet;

    #[test]
    fn invariant_catalog_codes_are_unique() {
        let mut codes = HashSet::new();
        for descriptor in invariant_catalog() {
            assert!(codes.insert(descriptor.code), "duplicate invariant code");
        }
    }

    #[test]
    fn invariant_catalog_entries_have_traceability_metadata() {
        for descriptor in invariant_catalog() {
            assert!(!descriptor.summary.is_empty());
            assert!(!descriptor.description.is_empty());
            assert!(!descriptor.spec_sections.is_empty());
            assert!(!descriptor.proof_modules.is_empty());
            assert_eq!(invariant_descriptor(descriptor.code), Some(descriptor));
        }
    }
}
