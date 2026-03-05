# Verus Proof Tasks

## Status Snapshot (2026-03-05)
- Completion: 68%
- Done: task map defined, proof runner wired, printable guide snapshot vendor flow added, stale-snapshot CI guard wired, and concrete reducer/workspace proof modules implemented.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Implement Verus proofs for runtime-critical state invariants.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic V1: Reducer Safety Proofs
### Task V1.1: Claim/running relation proof
- [x] Subtask V1.1.1: Prove running implies claimed for all transitions.
- [x] Subtask V1.1.2: Prove release removes running and claim entries.

### Task V1.2: Uniqueness and monotonicity proofs
- [x] Subtask V1.2.1: Prove single running entry per issue.
- [x] Subtask V1.2.2: Prove retry attempt monotonicity under requeue.

## Epic V2: Retry and Dispatch Proofs
### Task V2.1: Dispatch soundness
- [x] Subtask V2.1.1: Prove dispatch preconditions prevent duplicate run creation.
- [ ] Subtask V2.1.2: Prove slot accounting never goes negative.

### Task V2.2: Reconciliation correctness
- [x] Subtask V2.2.1: Prove terminal/non-active transitions eventually release claim.
- [x] Subtask V2.2.2: Prove continuation retry semantics preserve invariants.

## Epic V3: Tooling and CI
### Task V3.1: Proof harness integration
- [x] Subtask V3.1.1: Add proof runner scripts.
- [x] Subtask V3.1.2: Integrate proof checks into CI.

### Task V3.2: Verus reference operations
- [x] Subtask V3.2.1: Vendor printable Verus guide snapshot under `reference/`.
- [x] Subtask V3.2.2: Add reproducible snapshot sync script.
- [x] Subtask V3.2.3: Add CI guard for stale snapshot metadata.

## Exit Criteria
- [ ] All V1 and V2 proofs pass in CI.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 7 state machine invariants | `runtime_quick.rs` and `runtime_full.rs` verified | Add stronger lemmas for slot bounds and full transition rejection taxonomy | `V2.1` |
| Sec. 8 retry/reconciliation progress obligations | `session_liveness.rs` verified | Extend to full scheduler fairness and eventual processing assumptions | `V2.2` |
| Sec. 9.5 workspace safety invariants | `workspace_safety.rs` verified | Add proof obligations for path normalization edge and policy composition | `V2.2` |
| Sec. 17/18 verification gate requirements | Proof scripts and stale-reference guard wired in CI | Enforce branch protection and expand required suite matrix | `V3.1`, `V3.2` |
<!-- SPEC_GAP_MAP_END -->
