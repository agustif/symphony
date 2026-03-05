# symphony-http Tasks

## Status Snapshot (2026-03-05)
- Completion: 60%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own HTTP observability/control endpoints and serialization contracts.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Snapshot model: [../symphony-observability/TASKS.md](../symphony-observability/TASKS.md)

## Epic H1: Endpoint Surface
### Task H1.1: Human dashboard endpoint
- [ ] Subtask H1.1.1: `/` endpoint rendering integration.
- [ ] Subtask H1.1.2: Offline/error state rendering.

### Task H1.2: JSON API endpoints
- [x] Subtask H1.2.1: `/api/v1/state`.
- [x] Subtask H1.2.2: `/api/v1/{issue_identifier}`.
- [x] Subtask H1.2.3: `/api/v1/refresh`.

## Epic H2: API Contracts and Error Handling
### Task H2.1: Error schema
- [ ] Subtask H2.1.1: Method/parse/timeout/unavailable errors.
- [x] Subtask H2.1.2: 404 unknown issue behavior.

### Task H2.2: Escaping and safety
- [ ] Subtask H2.2.1: HTML escaping for dashboard.
- [x] Subtask H2.2.2: JSON payload stability tests.

## Epic H3: Tests
### Task H3.1: Endpoint tests
- [x] Subtask H3.1.1: End-to-end endpoint behavior tests.
- [x] Subtask H3.1.2: Error-path and payload-shape tests.

## Exit Criteria
- [x] HTTP contract is stable and covered by integration tests.
