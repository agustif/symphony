# symphony-runtime Tasks

## Status Snapshot (2026-03-07)
- Completion: 92%
- Done: baseline poll/dispatch loop, blocker-aware candidate gating, startup terminal cleanup, dispatch-time candidate revalidation before worker launch, protocol-driven state updates, observer-facing state snapshots, retry due-time metadata, best-effort `after_run`, unsupported dynamic tool-call fallback, concrete `linear_graphql` execution, app-server multi-turn session reuse, real-activity-driven stall detection, explicit child-stop coverage for retry restart, non-active release, terminal reconciliation, and shutdown paths, and host supervision shutdown test are implemented.
- In Progress: retained-invalid reload parity at the runtime boundary and residual edge-branch recovery behavior.
- Remaining: full SPEC-accurate orchestration and recovery behavior at the last host/runtime integration seams.

## Scope
Own async orchestration loop, scheduling, dispatch, reconciliation, and retry execution.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Upstream reducer: [../symphony-domain/TASKS.md](../symphony-domain/TASKS.md)
- Upstream config/workflow: [../symphony-config/TASKS.md](../symphony-config/TASKS.md), [../symphony-workflow/TASKS.md](../symphony-workflow/TASKS.md)
- Adapter contracts: [../symphony-tracker/TASKS.md](../symphony-tracker/TASKS.md), [../symphony-workspace/TASKS.md](../symphony-workspace/TASKS.md), [../symphony-agent-protocol/TASKS.md](../symphony-agent-protocol/TASKS.md)

## Epic R1: Scheduler Core
### Task R1.1: Poll loop lifecycle
- [x] Subtask R1.1.1: Tick scheduling and immediate refresh triggers.
- [x] Subtask R1.1.2: Runtime config reload integration.
- [x] Subtask R1.1.3: Dispatch-time config revalidation and retained-invalid reload parity.

### Task R1.2: Dispatch selection
- [x] Subtask R1.2.1: Candidate filtering with active and terminal rules.
- [x] Subtask R1.2.2: Global and per-state slot enforcement.
- [x] Subtask R1.2.3: `Todo` blocker gating and required-field eligibility checks.

## Epic R2: Retry and Reconciliation
### Task R2.1: Retry queue engine
- [x] Subtask R2.1.1: Continuation and failure backoff parity.
- [x] Subtask R2.1.2: Retry cancellation, replacement, and observer-facing due-time semantics.

### Task R2.2: Active-run reconciliation
- [x] Subtask R2.2.1: Stall detection and forced restart driven by real protocol activity timestamps.
- [x] Subtask R2.2.2: Tracker refresh reconciliation with explicit child-stop and cleanup behavior.

## Epic R3: Worker Lifecycle
### Task R3.1: Worker spawn and session lifecycle
- [x] Subtask R3.1.1: Spawn with workspace and prompt context.
- [x] Subtask R3.1.2: Reuse one app-server session across `max_turns`.
- [x] Subtask R3.1.3: Stop subprocesses and sessions explicitly on terminal stop, non-active stop, restart, and host shutdown.

### Task R3.2: Event integration
- [x] Subtask R3.2.1: Integrate live protocol updates into runtime state and activity tracking.
- [x] Subtask R3.2.2: Surface spec-accurate snapshot updates for observers.
- [x] Subtask R3.2.3: Treat approval and input-required protocol signals as terminal policy outcomes.
- [x] Subtask R3.2.4: Align protocol timeout and error taxonomy with explicit runtime policy outcomes.
- [x] Subtask R3.2.5: Return unsupported dynamic tool-call responses while keeping the session alive.
- [x] Subtask R3.2.6: Implement `linear_graphql` tool-call execution with validated input and structured outputs.

### Task R3.3: Hook and exit semantics
- [x] Subtask R3.3.1: Treat `after_run` hook failures as best-effort diagnostics, not worker failures.
- [x] Subtask R3.3.2: Preserve worker/session metadata needed by observability and recovery paths.

## Epic R4: Tests
### Task R4.1: Deterministic unit and integration tests
- [x] Subtask R4.1.1: Baseline poll, dispatch, and retry tests.
- [x] Subtask R4.1.2: Baseline reconciliation and stall tests.
- [x] Subtask R4.1.3: Add session-reuse, child-stop, retry-metadata, and protocol-update integration coverage.
- [x] Subtask R4.1.4: Add algorithm-to-code traceability assertions for the reference algorithms.

## Exit Criteria
- [ ] Runtime behavior matches required orchestration semantics.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 8.1-8.5 poll, dispatch, retry, and reconcile loops | Largely implemented | Close the remaining retained-invalid reload seam and keep edge-branch retry behavior aligned as host/runtime integration settles | `R1.1`, `R2.1`, `R4.1` |
| Sec. 8.6 startup terminal workspace cleanup | Implemented for baseline cleanup paths | Add parity for partial-data cleanup and cleanup-failure ordering | `R2.2`, `R4.1` |
| Sec. 10 app-server integration and Sec. 12 prompt assembly | Mostly implemented | Keep version compatibility coverage expanding while preserving the new explicit stop semantics | `R2.2`, `R3.1`, `R3.2`, `R4.1` |
| Sec. 14 recovery behavior and restart semantics | Mostly implemented | Preserve cleanup ordering across the remaining host/runtime restart edges | `R2.2`, `R3.1`, `R4.1` |
| Sec. 16.2-16.6 reference algorithms | Mostly implemented | Add any remaining edge-branch traceability assertions as new runtime branches land | `R4.1` |
<!-- SPEC_GAP_MAP_END -->
