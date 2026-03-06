# symphony-domain Tasks

## Status Snapshot (2026-03-05)
- Completion: 92%
- Done: task map defined and merged with serialization-safe transition rejection payloads, structured invariant violation diagnostics, and property-based transition-sequence tests.
- In Progress: edge-case conformance and proof depth.
- Remaining: full SPEC parity and production rollout gates.

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

### Task D3.2: Test depth
- [x] Subtask D3.2.1: Unit tests for every transition.
- [x] Subtask D3.2.2: Property tests for transition sequences.

## Exit Criteria
- [ ] Reducer and invariants are deterministic and fully tested.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 4.1 entities and Sec. 4.2 normalization | Implemented for core runtime entities with serialization-safe rejection/violation payloads | Expand diagnostics schema only if SPEC introduces additional required fields | `D1.2` |
| Sec. 7 issue orchestration state machine | Core transitions implemented with structured rejection and invariant violation diagnostics | Expand transition detail coverage for additional edge branches only when SPEC changes | `D2.2`, `D3.1` |
| Sec. 7.4 idempotency and recovery rules | Determinism validated | Expand sequence/property tests to multi-event interleavings | `D3.2` |
| Sec. 16.4 and Sec. 16.6 reference reducer behavior | Implemented in reducer | Close residual mismatch checks between reducer events and algorithm pseudocode | `D2.1`, `D2.2` |
<!-- SPEC_GAP_MAP_END -->
