# symphony-domain Tasks

## Status Snapshot (2026-03-07)
- Completion: 96%
- Done: task map defined and merged with serialization-safe transition rejection payloads, structured invariant violation diagnostics, a public modular invariant catalog with proof/spec metadata, rustdoc-backed reducer examples, extracted agent-update accounting helpers, and property-based reducer/event-stream tests.
- In Progress: proof-to-runtime traceability and remaining multi-crate rollout gates.
- Remaining: final proof/doc parity and production rollout gates.

## Scope
Own pure domain types, reducer transitions, and invariant validation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Downstream runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)
- Formal proofs: [../../proofs/verus/TASKS.md](../../proofs/verus/TASKS.md)

## Epic D1: Domain Types
### Task D1.1: Canonical entities
- [x] Subtask D1.1.1: Define issue, state, retry, and session entities.
- [x] Subtask D1.1.2: Normalize IDs and state names.

### Task D1.2: Error taxonomy
- [x] Subtask D1.2.1: Define invariant and transition errors.
- [x] Subtask D1.2.2: Define serialization-safe error payloads.

## Epic D2: Reducer Transitions
### Task D2.1: Dispatch lifecycle transitions
- [x] Subtask D2.1.1: Claim/dispatch/start transitions.
- [x] Subtask D2.1.2: Completion/failure transitions.

### Task D2.2: Retry/reconcile transitions
- [x] Subtask D2.2.1: Retry scheduling and cancellation transitions.
- [x] Subtask D2.2.2: Terminal/non-active reconcile transitions.

## Epic D3: Invariants and Tests
### Task D3.1: Invariant API
- [x] Subtask D3.1.1: Expose invariant validation helpers.
- [x] Subtask D3.1.2: Include structured violation diagnostics.
- [x] Subtask D3.1.3: Keep invariant checks modular and reusable for reducer/runtime assertions.
- [x] Subtask D3.1.4: Publish invariant catalog metadata that links executable checks to SPEC and proof artifacts.

### Task D3.2: Test depth
- [x] Subtask D3.2.1: Unit tests for every transition.
- [x] Subtask D3.2.2: Property tests for transition sequences.
- [x] Subtask D3.2.3: Cover `UpdateAgent` usage/session/turn semantics with deterministic unit tests.

## Exit Criteria
- [x] Reducer and invariants are deterministic and fully tested at the crate boundary.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 4.1 entities and Sec. 4.2 normalization | Implemented for core runtime entities with serialization-safe rejection/violation payloads | Expand diagnostics schema only if SPEC introduces additional required fields | `D1.2` |
| Sec. 7 issue orchestration state machine | Core transitions implemented with structured rejection and invariant violation diagnostics, including `UpdateAgent` accounting semantics | Keep proof and runtime traceability current as the reducer evolves | `D2.2`, `D3.1` |
| Sec. 7.4 idempotency and recovery rules | Determinism validated with property-based event streams and explicit rejection-state preservation checks | Expand only if new reducer events are introduced | `D3.2` |
| Sec. 16.4 and Sec. 16.6 reference reducer behavior | Implemented in reducer, including absolute token delta accounting and session-reset baselines | Keep algorithm comments and proof abstractions aligned with reducer helpers | `D2.1`, `D2.2` |
<!-- SPEC_GAP_MAP_END -->
