# Conformance Suite Tasks

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: workflow/config precedence and invalid-field cases, core dispatch rules, tracker transport plus empty/blank/missing-id normalization, protocol handshake/parsing compatibility including startup aliases and fragmented streams, degraded HTTP snapshot cases, observability payload completeness including rate-limit/activity/timestamp fields, HTTP-enabled host lifecycle coverage for startup/bind-failure/refresh/reload/shutdown, and recovery conformance for startup terminal cleanup, terminal cleanup fallback, tracker-refresh failure, worker failure, and snapshot degradation are implemented and green.
- 132 conformance tests passing covering all required SPEC sections 17 and 18.
- Remaining: optional extension profiles and explicit live integration evidence.

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
- [x] Subtask C2.1.2: Transient cleanup, `before_remove` rollback, and cleanup-failure semantics.

### Task C2.2: Dispatch, retry, and reconciliation semantics
- [x] Subtask C2.2.1: Candidate selection and blocker rules.
- [x] Subtask C2.2.2: Session reuse across `max_turns`, failure/continuation backoff parity, and explicit child-stop behavior.

## Epic C3: Adapter and Protocol Conformance
### Task C3.1: Tracker contract
- [x] Subtask C3.1.1: Candidate/state fetch baseline contract.
- [x] Subtask C3.1.2: Missing-node and partial-data normalization cases.
- [x] Subtask C3.1.3: Pagination edge cases.

### Task C3.2: App-server contract
- [x] Subtask C3.2.1: Handshake sequencing and stdout-only parsing.
- [x] Subtask C3.2.2: Interrupted streams, rate-limit payload variants, session metadata propagation, supported-tool hooks, startup timeout/compatibility branches, and high-volume parsing stress.

## Epic C4: Observability and Host Lifecycle Conformance
### Task C4.1: Snapshot and API contract
- [x] Subtask C4.1.1: `/api/v1/state` and `/api/v1/{issue}` payload completeness, timestamp, and error-schema parity.
- [x] Subtask C4.1.2: Absolute token accounting, rate-limit propagation, and the remaining snapshot timeout/unavailable semantics beyond the new baseline degraded cases.

### Task C4.2: CLI and host lifecycle
- [x] Subtask C4.2.1: Startup gate, bind-failure, refresh, and supervised-task death behavior.
- [x] Subtask C4.2.2: Controlled shutdown and restart behavior with optional HTTP enabled.

## Epic C5: Operational Validation
### Task C5.1: Required operator behavior
- [x] Subtask C5.1.1: Structured log and state-surface validation for required observability behavior.
- [x] Subtask C5.1.2: Recovery validation for tracker failure, worker failure, and snapshot degradation paths.

### Task C5.2: Recommended extension and real-integration profiles
- [x] Subtask C5.2.1: Optional HTTP and `linear_graphql` extension coverage.
- [x] Subtask C5.2.2: Real integration profile with explicit skip behavior when external dependencies are unavailable.

## Exit Criteria
- [x] Required SPEC sections 17.1-17.7 and 18.1 are implemented and passing.
- [x] Optional extension coverage is separated from required conformance progress.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1 workflow and config parsing | Complete | Keep matrix current as contract evolves | `C1.1`, `C1.2` |
| Sec. 17.2 workspace manager and safety | Complete with cleanup/fallback semantics | Extend with operational edge cases as discovered | `C2.1` |
| Sec. 17.3 tracker client | Complete with pagination edges | Extend with live tracker interop | `C3.1` |
| Sec. 17.4 orchestrator dispatch/retry/reconcile | Complete with session reuse, restart, child-stop | Extend with spec backoff parity refinements | `C2.2`, `C5.1` |
| Sec. 17.5 app-server protocol client | Complete with parser, startup, rate-limit, stream-fragment | Extend with live app-server interop | `C3.2` |
| Sec. 17.6 observability | Complete with payload, rate-limit, timing, degraded snapshot | Keep extending operator-visible validation | `C4.1`, `C5.1` |
| Sec. 17.7 CLI and host lifecycle | Complete with startup, bind-failure, refresh, task-death, shutdown | Add deeper watcher/relaunch parity | `C4.2` |
| Sec. 18.1 required conformance | Complete | Maintain CI-ready harness coverage | `C5.1` |
| Sec. 18.2 recommended extensions | Complete | Keep optional coverage separate | `C5.2` |
| Sec. 18.3 real integration profile | Complete with explicit-skip profile | Add real success-path evidence collection | `C5.2` |
<!-- SPEC_GAP_MAP_END -->
