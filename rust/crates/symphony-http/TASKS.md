# symphony-http Tasks

## Status Snapshot (2026-03-06)
- Completion: 90%
- Done: route wiring, state-driven snapshot sourcing, Elixir-shaped state and issue-detail payloads, refresh status/error semantics, timeout/unavailable stale-snapshot handling, degraded dashboard rendering, safe HTML escaping, runtime activity/throughput summary fields, and endpoint integration/conformance tests including state/issue contract completeness cases are implemented.
- In Progress: richer dashboard depth outside the current summary cards and any compatibility edge cases outside the current API contract.
- Remaining: complete the remaining dashboard/operator surface polish and any required compatibility shims.

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
- [x] Subtask H3.1.2: Contract-completeness tests for payload fields, degraded states, and backward compatibility.

## Exit Criteria
- [ ] HTTP contract is state-driven, spec-accurate, and covered by integration tests.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 13.7.2 JSON REST API | Implemented for steady-state, stale, timeout, and unavailable cases | Keep backward-compatible additive fields stable as the operator surface grows | `H2.2`, `H3.1` |
| Sec. 13.7.1 human dashboard | Implemented for baseline live, stale, and offline rendering | Add richer state visibility and operator detail beyond the current summary cards | `H1.1`, `H3.1` |
| Sec. 13.3 runtime snapshot surface contract | Mostly implemented | Preserve compatibility as activity/timing detail continues to grow | `H2.2`, `H3.1` |
| Sec. 14.2 failure behavior | Mostly implemented for snapshot degradation | Keep dashboard and API behavior stable as recovery semantics deepen | `H1.1`, `H3.1` |
| Sec. 17.6 observability endpoint validation | Mostly implemented | Keep extending conformance coverage as new operator-visible fields are added | `H3.1` |
<!-- SPEC_GAP_MAP_END -->
