# symphony-testkit Tasks

## Status Snapshot (2026-03-07)
- Completion: 93%
- Done: task map defined, deterministic clocks and timer queues landed, reusable tracker/protocol/workspace fakes exist, snapshot/trace helpers are shared across suites, and the trace harness now validates initial-state correctness plus per-event reducer contracts.
- In Progress: workflow/config-specific fixture breadth and remaining hardening for deeper conformance.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own reusable fixtures, fakes, and deterministic harness utilities across test suites.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Test program: [../../tests/TASKS.md](../../tests/TASKS.md)

## Epic K1: Fixture Factories
### Task K1.1: Domain fixture builders
- [x] Subtask K1.1.1: Issue/workflow/config fixture builders.
- [x] Subtask K1.1.2: Runtime state fixture builders.

### Task K1.2: Adapter fakes
- [x] Subtask K1.2.1: Fake tracker behaviors.
- [x] Subtask K1.2.2: Fake app-server stream generator.
- [x] Subtask K1.2.3: Fake workspace/hook runner.

## Epic K2: Deterministic Utilities
### Task K2.1: Time and scheduler controls
- [x] Subtask K2.1.1: Deterministic clock utility.
- [x] Subtask K2.1.2: Deterministic timer queue utility.

### Task K2.2: Assertions and snapshots
- [x] Subtask K2.2.1: Structured assertion helpers.
- [x] Subtask K2.2.2: Snapshot normalization helpers.
- [x] Subtask K2.2.3: Expose reducer-contract trace validation and a dedicated reducer contract integration suite.

## Exit Criteria
- [x] Cross-suite tests share deterministic utilities from this crate.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17 test matrix shared harness needs | Partial utilities implemented | Provide complete deterministic fixtures for config/workflow/runtime/protocol scenarios | `K1.1`, `K1.2` |
| Sec. 17.4 orchestrator conformance replay | Shared trace validation and interleaving utilities are in place | Keep expanding scenario breadth while preserving reducer-contract assertions | `K2.1`, `K2.2` |
| Sec. 17.5 protocol client robustness tests | Partial | Add stream fuzzing helpers and malformed event generators | `K1.2`, `K2.2` |
| Sec. 18.1 conformance gates | Reducer-contract suite and one-call harness entrypoints now exist | Wire remaining release-gate policy and suite subsets in CI | `K2.2` |
<!-- SPEC_GAP_MAP_END -->
