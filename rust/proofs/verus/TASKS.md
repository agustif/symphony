# Verus Proof Tasks

## Status Snapshot (2026-03-05)
- Completion: 10%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Implement Verus proofs for runtime-critical state invariants.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic V1: Reducer Safety Proofs
### Task V1.1: Claim/running relation proof
- [ ] Subtask V1.1.1: Prove running implies claimed for all transitions.
- [ ] Subtask V1.1.2: Prove release removes running and claim entries.

### Task V1.2: Uniqueness and monotonicity proofs
- [ ] Subtask V1.2.1: Prove single running entry per issue.
- [ ] Subtask V1.2.2: Prove retry attempt monotonicity under requeue.

## Epic V2: Retry and Dispatch Proofs
### Task V2.1: Dispatch soundness
- [ ] Subtask V2.1.1: Prove dispatch preconditions prevent duplicate run creation.
- [ ] Subtask V2.1.2: Prove slot accounting never goes negative.

### Task V2.2: Reconciliation correctness
- [ ] Subtask V2.2.1: Prove terminal/non-active transitions eventually release claim.
- [ ] Subtask V2.2.2: Prove continuation retry semantics preserve invariants.

## Epic V3: Tooling and CI
### Task V3.1: Proof harness integration
- [ ] Subtask V3.1.1: Add proof runner scripts.
- [ ] Subtask V3.1.2: Integrate proof checks into CI.

## Exit Criteria
- [ ] All V1 and V2 proofs pass in CI.
