# symphony-http Tasks

## Status Snapshot (2026-03-08)
- Completion: 92%
- Done: route wiring, state-driven snapshot sourcing, Elixir-shaped state and issue-detail payloads, refresh status/error semantics, timeout/unavailable stale-snapshot handling, degraded dashboard rendering, safe HTML escaping, runtime activity/throughput summary fields, additive health/task-map/operator summary fields, deterministic redaction-safe message shaping, shared operator projection fields consumed by both HTTP and terminal surfaces, endpoint integration/conformance tests including offline/stale operator cases, and workspace conformance tests (15 tests) are implemented.
- In Progress: preserving Elixir parity as any new operator-visible fields arrive from upstream runtime or protocol work, without letting terminal and HTTP projections drift.
- Remaining: remove the last surface-level Elixir mismatches instead of carrying compatibility-only branches.

## Scope
Own HTTP observability/control endpoints and serialization contracts.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Snapshot model: [../symphony-observability/TASKS.md](../symphony-observability/TASKS.md)

## Epic H1: Endpoint Surface
### Task H1.1: Human dashboard endpoint
- [x] Subtask H1.1.1: `/` endpoint rendering integration.
- [x] Subtask H1.1.2: Degraded, unavailable, and stale-snapshot rendering.

### Task H1.2: JSON API endpoints
- [x] Subtask H1.2.1: Baseline route wiring for `/api/v1/state`, `/api/v1/{issue_identifier}`, and `/api/v1/refresh`.
- [x] Subtask H1.2.2: Full payload parity for `/api/v1/state` and `/api/v1/{issue_identifier}`.
- [x] Subtask H1.2.3: Status-code and refresh behavior parity for `/api/v1/refresh`.

## Epic H2: Snapshot Sourcing and Contracts
### Task H2.1: State-driven snapshot contract
- [x] Subtask H2.1.1: Serve API payloads from runtime snapshot only, without live tracker dependency.
- [x] Subtask H2.1.2: Propagate snapshot timeout and unavailable states without collapsing the HTTP surface.

### Task H2.2: Serialization contract
- [x] Subtask H2.2.1: Align timestamps, error schema, and envelope fields with the spec contract.
- [x] Subtask H2.2.2: Align running, retrying, issue-detail, token, and rate-limit payload fields with the observability schema.

## Epic H3: Tests
### Task H3.1: Endpoint tests
- [x] Subtask H3.1.1: Baseline end-to-end endpoint behavior tests.
- [x] Subtask H3.1.2: Contract-completeness tests for payload fields, degraded states, and route/payload parity.

## Exit Criteria
- [ ] HTTP contract is state-driven, spec-accurate, and covered by integration tests.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 13.7.2 JSON REST API | Implemented for steady-state, stale, timeout, and unavailable cases | Close the remaining Elixir-vs-Rust payload edge cases instead of preserving extra route or payload branches | `H2.2`, `H3.1` |
| Sec. 13.7.1 human dashboard | Mostly implemented with live, stale, and offline operator detail | Preserve degraded-state accuracy and extend panels only if the observability model gains new fields | `H1.1`, `H3.1` |
| Sec. 13.3 runtime snapshot surface contract | Mostly implemented | Preserve compatibility as activity/timing detail continues to grow | `H2.2`, `H3.1` |
| Sec. 14.2 failure behavior | Mostly implemented for snapshot degradation | Keep dashboard and API behavior stable as recovery semantics deepen | `H1.1`, `H3.1` |
| Sec. 17.6 observability endpoint validation | Mostly implemented | Keep extending conformance coverage as new operator-visible fields are added | `H3.1` |
<!-- SPEC_GAP_MAP_END -->
