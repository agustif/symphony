# Test Program Master Tasks

## Status Snapshot (2026-03-06)
- Completion: 60%
- Done: shared deterministic harnesses landed in `symphony-testkit`, baseline conformance cases landed, suite thresholds are documented, suite-specific CI jobs are added, and the current tree has fresh local workspace evidence from `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and a live Linear-backed Rust app smoke run.
- In Progress: required conformance expansion, protected release gates, and formalizing production-like validation depth into repeatable profiles.
- Remaining: full required SPEC coverage, deeper interleaving/soak depth, and a checked-in real integration profile with explicit skip/evidence rules.

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
- [x] Subtask T1.1.1: Deterministic clock harness.
- [x] Subtask T1.1.2: Fake tracker, app-server, and workspace harnesses.
- [x] Subtask T1.1.3: Structured fixture loading and snapshot helpers.

### Task T1.2: Quality gates
- [x] Subtask T1.2.1: Define pass/fail thresholds per suite.
- [x] Subtask T1.2.2: Wire thresholds into CI.
- [ ] Subtask T1.2.3: Promote required suites to protected-branch release gates.

## Epic T2: Coverage Governance
### Task T2.1: Coverage audit
- [ ] Subtask T2.1.1: Map each crate API to test ownership.
- [ ] Subtask T2.1.2: Track uncovered branches and deadlines.

### Task T2.2: Required conformance completeness
- [ ] Subtask T2.2.1: Finish required SPEC sections 17.6, 17.7, and 18.1 coverage.
- [ ] Subtask T2.2.2: Separate optional extension coverage from required conformance progress.

## Exit Criteria
- [ ] All child suites green with required thresholds.
- [ ] Required SPEC conformance coverage is complete and traceable.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1-17.8 validation matrix | Partial suite coverage | Complete required coverage ownership and close remaining observability and host lifecycle gaps | `T2.1`, `T2.2` |
| Sec. 18.1 required conformance gates | CI jobs exist | Promote required checks to release gates and back them with shared deterministic harnesses | `T1.1`, `T1.2`, `T2.2` |
| Sec. 18.3 operational validation | Partial soak and failure tests plus manual live-run evidence | Add checked-in production-like long-run profiles, explicit live integration harnesses, and incident assertions | `T1.1` |
<!-- SPEC_GAP_MAP_END -->
