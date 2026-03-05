# symphony-testkit Tasks

## Status Snapshot (2026-03-05)
- Completion: 65%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own reusable fixtures, fakes, and deterministic harness utilities across test suites.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Test program: [../../tests/TASKS.md](../../tests/TASKS.md)

## Epic K1: Fixture Factories
### Task K1.1: Domain fixture builders
- [ ] Subtask K1.1.1: Issue/workflow/config fixture builders.
- [ ] Subtask K1.1.2: Runtime state fixture builders.

### Task K1.2: Adapter fakes
- [ ] Subtask K1.2.1: Fake tracker behaviors.
- [ ] Subtask K1.2.2: Fake app-server stream generator.
- [ ] Subtask K1.2.3: Fake workspace/hook runner.

## Epic K2: Deterministic Utilities
### Task K2.1: Time and scheduler controls
- [ ] Subtask K2.1.1: Deterministic clock utility.
- [ ] Subtask K2.1.2: Deterministic timer queue utility.

### Task K2.2: Assertions and snapshots
- [ ] Subtask K2.2.1: Structured assertion helpers.
- [ ] Subtask K2.2.2: Snapshot normalization helpers.

## Exit Criteria
- [ ] Cross-suite tests share deterministic utilities from this crate.
