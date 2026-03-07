# Test Program Master Tasks

## Status Snapshot (2026-03-07)
- Completion: 93%
- Done: shared deterministic harnesses landed in `symphony-testkit`, conformance/interleaving/soak suites are wired into CI, CI now publishes Rust coverage summaries and LCOV artifacts, the conformance matrix now covers workflow/config, tracker normalization, protocol compatibility, runtime retry/reconcile paths, degraded HTTP, and HTTP-enabled host lifecycle, the current tree has fresh local evidence from `cargo test --workspace` plus `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace -- --list` currently reports `572` Rust test cases.
- Interleavings suite complete: 18 tests covering scheduler interleavings, worker lifecycle races, and concurrency safety.
- Soak suite complete: 15 tests covering long-running stress profiles.
- Conformance suite: 132 tests covering SPEC sections 17 and 18 requirements.
- Release gates: `suite-gates` CI job runs conformance, interleavings, and soak suites on PRs and pushes to main.
- Remaining: explicit uncovered-branch tracking with remediation deadlines.

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
- [x] Subtask T1.2.3: Publish workspace coverage summaries and LCOV artifacts in CI.
- [x] Subtask T1.2.4: Promote required suites to protected-branch release gates.

## Epic T2: Coverage Governance
### Task T2.1: Coverage audit
- [x] Subtask T2.1.1: Map each crate API to test ownership.
- [ ] Subtask T2.1.2: Track uncovered branches and deadlines.

### Task T2.2: Required conformance completeness
- [x] Subtask T2.2.1: Finish required SPEC sections 17.6, 17.7, and 18.1 coverage.
- [x] Subtask T2.2.2: Separate optional extension coverage from required conformance progress.

## Exit Criteria
- [x] All child suites green with required thresholds.
- [x] Required SPEC conformance coverage is complete and traceable.
- [x] Rust validation surface matches or exceeds the current Elixir suite depth (`450+` tests) while remaining proof-backed and SPEC-traceable.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1-17.8 validation matrix | Complete suite coverage with 132 conformance, 18 interleaving, 15 soak tests | Add explicit live integration harnesses for production-like evidence | `T1.1`, `T2.2` |
| Sec. 18.1 required conformance gates | CI jobs plus coverage artifacts exist, all required sections covered, suite-gates job enforces conformance/interleaving/soak on PRs and main | Maintain as SPEC evolves | Complete |
| Sec. 18.3 operational validation | Full interleaving and soak coverage plus proof evidence | Add checked-in production-like long-run profiles and incident assertions | `T1.1` |
<!-- SPEC_GAP_MAP_END -->
