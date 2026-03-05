# symphony-domain Tasks

## Status Snapshot (2026-03-05)
- Completion: 70%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own pure domain types, reducer transitions, and invariant validation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Downstream runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)
- Formal proofs: [../../proofs/verus/TASKS.md](../../proofs/verus/TASKS.md)

## Epic D1: Domain Types
### Task D1.1: Canonical entities
- [ ] Subtask D1.1.1: Define issue, state, retry, and session entities.
- [ ] Subtask D1.1.2: Normalize IDs and state names.

### Task D1.2: Error taxonomy
- [x] Subtask D1.2.1: Define invariant and transition errors.
- [ ] Subtask D1.2.2: Define serialization-safe error payloads.

## Epic D2: Reducer Transitions
### Task D2.1: Dispatch lifecycle transitions
- [x] Subtask D2.1.1: Claim/dispatch/start transitions.
- [x] Subtask D2.1.2: Completion/failure transitions.

### Task D2.2: Retry/reconcile transitions
- [x] Subtask D2.2.1: Retry scheduling and cancellation transitions.
- [ ] Subtask D2.2.2: Terminal/non-active reconcile transitions.

## Epic D3: Invariants and Tests
### Task D3.1: Invariant API
- [x] Subtask D3.1.1: Expose invariant validation helpers.
- [ ] Subtask D3.1.2: Include structured violation diagnostics.

### Task D3.2: Test depth
- [x] Subtask D3.2.1: Unit tests for every transition.
- [ ] Subtask D3.2.2: Property tests for transition sequences.

## Exit Criteria
- [ ] Reducer and invariants are deterministic and fully tested.
