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

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 13.7.2 JSON REST API | `/state`, `/issue`, `/refresh` implemented | Complete standardized error schema and retryable/unavailable error payloads | `H2.1` |
| Sec. 13.7.1 human dashboard | Not complete | Implement root dashboard rendering with safe escaping and degraded states | `H1.1`, `H2.2` |
| Sec. 13.3 snapshot surface contract | Partial passthrough | Ensure output fields match observability schema exactly | `H2.2`, `H3.1` |
| Sec. 17.6 observability endpoint validation | Partial | Add conformance tests for endpoint payload completeness and backward compatibility | `H3.1` |
<!-- SPEC_GAP_MAP_END -->
