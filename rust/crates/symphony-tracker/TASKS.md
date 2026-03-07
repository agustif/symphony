# symphony-tracker Tasks

## Status Snapshot (2026-03-07)
- Completion: 100%
- Done: task map defined, canonical issue DTO/state normalization landed, blocker-aware normalized issue fields are implemented end-to-end, routing-only fields are documented in lib.rs module docs, and error mapping tests cover all TrackerError variants.
- Adapter contract is stable and integration-ready.

## Scope
Own tracker abstraction contracts and normalized issue model.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Linear implementation: [../symphony-tracker-linear/TASKS.md](../symphony-tracker-linear/TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic T1: Contract Design
### Task T1.1: Core trait surface
- [x] Subtask T1.1.1: Candidate fetch contract.
- [x] Subtask T1.1.2: Fetch-by-IDs state refresh contract.
- [x] Subtask T1.1.3: Fetch-by-states contract.

### Task T1.2: Domain normalization
- [x] Subtask T1.2.1: Canonical issue DTO and state normalization.
- [x] Subtask T1.2.2: Blocker semantics representation.
- [x] Subtask T1.2.3: Explicitly document any routing-only fields that stay outside the core tracker contract.

## Epic T2: Error Model
### Task T2.1: Typed errors
- [x] Subtask T2.1.1: Transport/status/graphql/validation error variants.
- [x] Subtask T2.1.2: Runtime-friendly display/context payloads.

## Epic T3: Tests
### Task T3.1: Trait contract tests
- [x] Subtask T3.1.1: Fake adapter compliance tests.
- [x] Subtask T3.1.2: Error mapping tests.

## Exit Criteria
- [x] Adapter contract is stable and integration-ready.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 11.1 required tracker operations | Complete | Add strict guarantees around operation ordering and missing issue semantics | Complete |
| Sec. 11.3 normalized issue model | Complete with routing-only field documentation | Complete | `T1.2` |
| Sec. 11.4 typed error model | Complete with error mapping tests | Complete | `T3.1` |
| Sec. 11.5 tracker write boundary | Complete (documented as read-only contract) | Complete | `T2.1` |
<!-- SPEC_GAP_MAP_END -->
