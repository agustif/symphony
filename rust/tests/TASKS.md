# Test Program Master Tasks

## Status Snapshot (2026-03-05)
- Completion: 55%
- Done: task map defined, suite thresholds documented, and suite-specific CI gates added.
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
- [x] Subtask T1.2.1: Define pass/fail thresholds per suite.
- [x] Subtask T1.2.2: Wire thresholds into CI.

## Epic T2: Coverage governance
### Task T2.1: Coverage audit
- [ ] Subtask T2.1.1: Map each crate API to test ownership.
- [ ] Subtask T2.1.2: Track uncovered branches and deadlines.

## Exit Criteria
- [ ] All child suites green with required thresholds.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1-17.8 validation matrix | Partial suite coverage | Complete matrix ownership map and threshold enforcement | `T2.1`, `T1.2` |
| Sec. 18.1 required conformance gates | Suite-specific CI jobs added for conformance/interleavings/soak and proofs | Promote these checks to protected-branch required checks | `T1.2` |
| Sec. 18.3 operational validation | Partial soak and failure tests | Add production-like long-run profiles and incident assertions | `T1.1` |
<!-- SPEC_GAP_MAP_END -->
