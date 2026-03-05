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
- [ ] Subtask R1.1.1: Tick scheduling and immediate refresh triggers.
- [ ] Subtask R1.1.2: Runtime config reload integration.

### Task R1.2: Dispatch selection
- [ ] Subtask R1.2.1: Candidate filtering with active/terminal rules.
- [ ] Subtask R1.2.2: Global/per-state slot enforcement.

## Epic R2: Retry and Reconciliation
### Task R2.1: Retry queue engine
- [x] Subtask R2.1.1: Continuation and failure backoff scheduling.
- [ ] Subtask R2.1.2: Retry cancellation and replacement rules.

### Task R2.2: Active-run reconciliation
- [ ] Subtask R2.2.1: Stall detection and forced restart.
- [x] Subtask R2.2.2: Tracker refresh reconciliation actions.

## Epic R3: Worker Lifecycle
### Task R3.1: Worker spawn/monitor
- [ ] Subtask R3.1.1: Spawn with workspace and prompt context.
- [x] Subtask R3.1.2: Exit reason mapping to retries.

### Task R3.2: Event integration
- [ ] Subtask R3.2.1: Integrate protocol updates into runtime state.
- [ ] Subtask R3.2.2: Surface snapshot updates for observers.

## Epic R4: Tests
### Task R4.1: Deterministic unit/integration tests
- [x] Subtask R4.1.1: Poll/dispatch/retry tests.
- [ ] Subtask R4.1.2: Reconciliation and stall tests.

## Exit Criteria
- [ ] Runtime behavior matches required orchestration semantics.
