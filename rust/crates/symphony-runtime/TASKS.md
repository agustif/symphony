# symphony-runtime Tasks

## Status Snapshot (2026-03-05)
- Completion: 55%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

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

### Task R1.2: Dispatch selection
- [x] Subtask R1.2.1: Candidate filtering with active/terminal rules.
- [x] Subtask R1.2.2: Global/per-state slot enforcement.

## Epic R2: Retry and Reconciliation
### Task R2.1: Retry queue engine
- [x] Subtask R2.1.1: Continuation and failure backoff scheduling.
- [x] Subtask R2.1.2: Retry cancellation and replacement rules.

### Task R2.2: Active-run reconciliation
- [x] Subtask R2.2.1: Stall detection and forced restart.
- [x] Subtask R2.2.2: Tracker refresh reconciliation actions.

## Epic R3: Worker Lifecycle
### Task R3.1: Worker spawn/monitor
- [x] Subtask R3.1.1: Spawn with workspace and prompt context.
- [x] Subtask R3.1.2: Exit reason mapping to retries.

### Task R3.2: Event integration
- [x] Subtask R3.2.1: Integrate protocol updates into runtime state.
- [x] Subtask R3.2.2: Surface snapshot updates for observers.

## Epic R4: Tests
### Task R4.1: Deterministic unit/integration tests
- [x] Subtask R4.1.1: Poll/dispatch/retry tests.
- [x] Subtask R4.1.2: Reconciliation and stall tests.

## Exit Criteria
- [ ] Runtime behavior matches required orchestration semantics.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 8.1-8.5 poll, dispatch, retry, reconcile loops | Core scheduler implemented | Add exhaustive parity for slot starvation, retry replacement, and reconcile error branches | `R1.2`, `R2.1`, `R2.2` |
| Sec. 8.6 startup terminal workspace cleanup | Partial | Implement startup sweep and ordering guarantees for terminal cleanup | `R2.2` |
| Sec. 10 app-server integration and Sec. 12 prompt assembly | Partially integrated | Complete approval/input-required loop and full prompt context semantics | `R3.1`, `R3.2` |
| Sec. 14 recovery behavior and restart semantics | Partial | Add restart recovery policy and persistent partial-state recovery hooks | `R2.2`, `R4.1` |
| Sec. 16.2-16.6 reference algorithms | Mostly implemented | Add algorithm-to-code traceability assertions in tests | `R4.1` |
<!-- SPEC_GAP_MAP_END -->
