# Conformance Suite Tasks

## Status Snapshot (2026-03-06)
- Completion: 60%
- Done: baseline workflow/config, core dispatch rules, tracker transport, protocol handshake/parsing matrix cases, baseline degraded HTTP snapshot cases, and state/issue payload completeness checks for the JSON observability surface including activity/timing summary fields are implemented and green.
- In Progress: required observability, host lifecycle, recovery, and operator-surface conformance coverage.
- Remaining: full required SPEC coverage for sections 17.2-17.7 and 18.1, plus recommended extension and real-integration profiles.

## Scope
Encode specification conformance behavior from SPEC sections 17 and 18, prioritizing required core coverage before optional extensions.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic C1: Config and Workflow Conformance
### Task C1.1: Workflow parsing cases
- [x] Subtask C1.1.1: Missing/invalid workflow file handling.
- [x] Subtask C1.1.2: Front matter type and prompt fallback behavior.

### Task C1.2: Config precedence and validation
- [x] Subtask C1.2.1: Env indirection and defaults.
- [x] Subtask C1.2.2: Invalid field rejection behavior.

## Epic C2: Runtime and Workspace Conformance
### Task C2.1: Workspace lifecycle and safety
- [x] Subtask C2.1.1: Containment, create, and reuse baseline cases.
- [ ] Subtask C2.1.2: Transient cleanup, `before_remove` rollback, and cleanup-failure semantics.

### Task C2.2: Dispatch, retry, and reconciliation semantics
- [x] Subtask C2.2.1: Candidate selection and blocker rules.
- [ ] Subtask C2.2.2: Session reuse across `max_turns`, failure/continuation backoff parity, and explicit child-stop behavior.

## Epic C3: Adapter and Protocol Conformance
### Task C3.1: Tracker contract
- [x] Subtask C3.1.1: Candidate/state fetch baseline contract.
- [ ] Subtask C3.1.2: Pagination edge, missing-node, and partial-data normalization cases.

### Task C3.2: App-server contract
- [x] Subtask C3.2.1: Handshake sequencing and stdout-only parsing.
- [ ] Subtask C3.2.2: Interrupted streams, rate-limit payload variants, session metadata propagation, and supported-tool hooks.

## Epic C4: Observability and Host Lifecycle Conformance
### Task C4.1: Snapshot and API contract
- [x] Subtask C4.1.1: `/api/v1/state` and `/api/v1/{issue}` payload completeness, timestamp, and error-schema parity.
- [ ] Subtask C4.1.2: Absolute token accounting, rate-limit propagation, and the remaining snapshot timeout/unavailable semantics beyond the new baseline degraded cases.

### Task C4.2: CLI and host lifecycle
- [ ] Subtask C4.2.1: Startup gate, bind-failure, refresh, and supervised-task death behavior.
- [ ] Subtask C4.2.2: Controlled shutdown and restart behavior with optional HTTP enabled.

## Epic C5: Operational Validation
### Task C5.1: Required operator behavior
- [ ] Subtask C5.1.1: Structured log and state-surface validation for required observability behavior.
- [ ] Subtask C5.1.2: Recovery validation for tracker failure, worker failure, and snapshot degradation paths.

### Task C5.2: Recommended extension and real-integration profiles
- [ ] Subtask C5.2.1: Optional HTTP and `linear_graphql` extension coverage.
- [ ] Subtask C5.2.2: Real integration profile with explicit skip behavior when external dependencies are unavailable.

## Exit Criteria
- [ ] Required SPEC sections 17.1-17.7 and 18.1 are implemented and passing.
- [ ] Optional extension coverage is separated from required conformance progress.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1 workflow and config parsing | Implemented for load/parse precedence and invalid-field rejection | Keep matrix current as contract evolves | `C1.1`, `C1.2` |
| Sec. 17.2 workspace manager and safety | Partial | Add transient cleanup, rollback, and cleanup-failure coverage | `C2.1` |
| Sec. 17.3 tracker client | Partial | Expand pagination, missing-node, and partial-data cases | `C3.1` |
| Sec. 17.4 orchestrator dispatch/retry/reconcile | Partial | Validate session reuse, restart semantics, child-stop behavior, and spec backoff parity | `C2.2`, `C5.1` |
| Sec. 17.5 app-server protocol client | Partial | Add rate-limit, session metadata, interrupted stream, and supported-tool compatibility coverage | `C3.2` |
| Sec. 17.6 observability | Partial | Extend beyond payload completeness into token/rate-limit semantics, full degraded snapshot behavior, and operator-visible validation | `C4.1`, `C5.1` |
| Sec. 17.7 CLI and host lifecycle | Not complete | Add startup, bind-failure, task-death, refresh, and shutdown coverage | `C4.2` |
| Sec. 18.1 required conformance | Not complete | Prove required operator-visible behavior and CI-ready harness coverage | `C5.1` |
| Sec. 18.2 recommended extensions | Partial | Keep optional HTTP and `linear_graphql` coverage separate from required progress | `C5.2` |
| Sec. 18.3 real integration profile | Not complete | Add live integration profile with explicit skip rules and evidence collection | `C5.2` |
<!-- SPEC_GAP_MAP_END -->
