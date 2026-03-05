# Test Program Master Tasks

## Status Snapshot (2026-03-05)
- Completion: 45%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Deliver complete validation coverage from unit tests to long-running stress profiles.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Child Task Maps
- Conformance: [conformance/TASKS.md](conformance/TASKS.md)
- Interleavings: [interleavings/TASKS.md](interleavings/TASKS.md)
- Soak: [soak/TASKS.md](soak/TASKS.md)

## Epic T1: Testing Infrastructure
### Task T1.1: Shared harnesses
- [ ] Subtask T1.1.1: Deterministic clock harness.
- [ ] Subtask T1.1.2: Fake tracker/app-server/workspace harnesses.
- [ ] Subtask T1.1.3: Structured fixture loading and snapshot helpers.

### Task T1.2: Quality gates
- [ ] Subtask T1.2.1: Define pass/fail thresholds per suite.
- [ ] Subtask T1.2.2: Wire thresholds into CI.

## Epic T2: Coverage governance
### Task T2.1: Coverage audit
- [ ] Subtask T2.1.1: Map each crate API to test ownership.
- [ ] Subtask T2.1.2: Track uncovered branches and deadlines.

## Exit Criteria
- [ ] All child suites green with required thresholds.
