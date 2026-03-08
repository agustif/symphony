# symphony-cli Tasks

## Status Snapshot (2026-03-08)
- Completion: 93%
- Done: baseline flag parsing, startup diagnostics, workflow-driven startup validation, optional HTTP wiring, runtime-driven HTTP snapshot serving, refresh unavailable mapping, degraded snapshot timeout/stale caching, signal-handling shutdown paths, retained-invalid reload diagnostics, restart-required reload retention, supervised background-task failure mapping, end-to-end host lifecycle coverage for both HTTP-enabled and HTTP-disabled startup paths, missing-workflow and bind-failure startup errors, host supervision shutdown test, and live terminal rendering sourced from the shared HTTP operator view are implemented.
- In Progress: last-mile host parity gaps around deeper production supervision semantics and startup-failure coverage, not terminal/web projection drift.
- Remaining: full CLI and host behavior parity with the spec and the Elixir reference, especially stronger watcher implementation parity and a broader startup-failure matrix.

## Scope
Own executable entrypoint, startup wiring, flags, and host lifecycle behavior.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime target: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic C1: CLI UX and Startup
### Task C1.1: Argument contract
- [x] Subtask C1.1.1: Positional workflow path and default behavior.
- [x] Subtask C1.1.2: Logs root and HTTP port flags.

### Task C1.2: Startup flow
- [x] Subtask C1.2.1: Validate workflow path and config before run.
- [x] Subtask C1.2.2: Structured startup diagnostics.
- [x] Subtask C1.2.3: Remove non-spec startup gates and keep startup contract workflow-driven.
- [x] Subtask C1.2.4: Surface retained-invalid reload diagnostics consistently.

## Epic C2: Runtime Wiring
### Task C2.1: Dependency wiring
- [x] Subtask C2.1.1: Construct tracker, protocol, workspace, and HTTP adapters.
- [x] Subtask C2.1.2: Monitor core task death and fail the host immediately when supervision is lost.
- [x] Subtask C2.1.3: Align refresh and reload wiring with runtime-side config validation.

### Task C2.2: Shutdown and signals
- [x] Subtask C2.2.1: Baseline graceful shutdown handling.
- [x] Subtask C2.2.2: Host exit and cleanup parity for background-task failure and restart scenarios.

## Epic C3: Tests
### Task C3.1: CLI behavior tests
- [x] Subtask C3.1.1: Parse and validation tests.
- [x] Subtask C3.1.2: Baseline startup failure mapping tests, including missing-workflow and bind-failure host exits.
- [x] Subtask C3.1.3: End-to-end host tests for refresh signaling, optional HTTP, and `--logs-root` file initialization after adding bind-failure and task-death coverage.

## Exit Criteria
- [ ] CLI provides production-grade startup and control behavior.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 6.1 startup config precedence | Strong reload/runtime parity for live-applicable vs restart-required config | Remaining gap is broader startup-failure and watcher validation, not restart-required exit semantics | `C1.2`, `C2.2` |
| Sec. 13.7 optional HTTP extension wiring | HTTP steady-state, degraded snapshots, refresh route, and host lifecycle tests are covered | Remaining gap is broader production traffic/latency soak coverage, not baseline wiring | `C2.1`, `C3.1` |
| Sec. 14.2 failure and recovery host behavior | Supervised background-task loss and restart cleanup are covered in unit and host tests | Remaining gap is richer recovery orchestration beyond fail-fast exit and cleanup | `C2.2` |
| Sec. 17.7 CLI and host lifecycle validation | Substantially covered with HTTP-enabled and HTTP-disabled end-to-end startup, reload, restart, retained-invalid reload, refresh, shutdown, bind-failure, and missing-workflow tests | Remaining gap is deeper watcher parity and a wider startup-failure/recovery matrix | `C2.2`, `C3.1` |
<!-- SPEC_GAP_MAP_END -->
