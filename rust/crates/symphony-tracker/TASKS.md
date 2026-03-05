# symphony-tracker Tasks

## Status Snapshot (2026-03-05)
- Completion: 70%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own tracker abstraction contracts and normalized issue model.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Linear implementation: [../symphony-tracker-linear/TASKS.md](../symphony-tracker-linear/TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic T1: Contract Design
### Task T1.1: Core trait surface
- [ ] Subtask T1.1.1: Candidate fetch contract.
- [ ] Subtask T1.1.2: Fetch-by-IDs state refresh contract.
- [ ] Subtask T1.1.3: Fetch-by-states contract.

### Task T1.2: Domain normalization
- [ ] Subtask T1.2.1: Canonical issue DTO and state normalization.
- [ ] Subtask T1.2.2: Blocker and assignee semantics representation.

## Epic T2: Error Model
### Task T2.1: Typed errors
- [ ] Subtask T2.1.1: Transport/status/graphql/validation error variants.
- [ ] Subtask T2.1.2: Runtime-friendly display/context payloads.

## Epic T3: Tests
### Task T3.1: Trait contract tests
- [ ] Subtask T3.1.1: Fake adapter compliance tests.
- [ ] Subtask T3.1.2: Error mapping tests.

## Exit Criteria
- [ ] Adapter contract is stable and integration-ready.
