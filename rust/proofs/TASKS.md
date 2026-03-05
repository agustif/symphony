# Formal Verification Program

## Status Snapshot (2026-03-05)
- Completion: 25%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Define, implement, and enforce formal proofs for orchestrator invariants.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Child Task Map
- Verus-specific tasks: [verus/TASKS.md](verus/TASKS.md)

## Epic P1: Proof Planning and Traceability
### Task P1.1: Invariant-to-proof mapping
- [ ] Subtask P1.1.1: Map each domain invariant to one proof artifact.
- [ ] Subtask P1.1.2: Map each proof artifact to reducer functions.
- [ ] Subtask P1.1.3: Map each proof artifact to runtime assertions/tests.

### Task P1.2: CI policy
- [ ] Subtask P1.2.1: Define required proof jobs for merge.
- [ ] Subtask P1.2.2: Define acceptable bypass policy for emergencies.

## Epic P2: Proof Operations
### Task P2.1: Regression maintenance
- [ ] Subtask P2.1.1: Track proof breakages by commit.
- [ ] Subtask P2.1.2: Require proof updates alongside invariant changes.

## Exit Criteria
- [ ] Proof program is wired into CI and release gates.
