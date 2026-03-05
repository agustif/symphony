# symphony-runtime Tasks

## Status Snapshot (2026-03-05)
- Completion: 92%
- Done: task map defined, startup terminal workspace cleanup, active-run tracker-state reconciliation with terminal cleanup hooks, candidate required-field filtering plus `Todo` blocker gating, worker protocol-stream policy handling for approval/input-required and timeout/cancelled with normalized startup/runtime error categories, live app-server handshake integration using shared startup payload builders, buffered handshake stream preservation into runtime monitoring, unsupported dynamic tool-call fallback responses (`success=false,error=unsupported_tool_call`) without permanent-failure escalation, and concrete `linear_graphql` tool execution with input validation and structured success/failure outputs.
- In Progress: restart semantics hardening and broader live-integration coverage.
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
- [x] Subtask R1.2.3: `Todo` blocker gating and required-field eligibility checks.

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
- [x] Subtask R3.2.3: Treat approval/input-required protocol signals as permanent worker failures.
- [x] Subtask R3.2.4: Align protocol timeout/error taxonomy with explicit runtime policy outcomes.
- [x] Subtask R3.2.5: Return unsupported dynamic tool-call responses while keeping the session alive.
- [x] Subtask R3.2.6: Implement `linear_graphql` tool-call execution path using tracker-config endpoint/auth.

## Epic R4: Tests
### Task R4.1: Deterministic unit/integration tests
- [x] Subtask R4.1.1: Poll/dispatch/retry tests.
- [x] Subtask R4.1.2: Reconciliation and stall tests.
- [x] Subtask R4.1.3: Add integration tests for approval/input-required hard-fail and timeout-path behavior.

## Exit Criteria
- [ ] Runtime behavior matches required orchestration semantics.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 8.1-8.5 poll, dispatch, retry, reconcile loops | Core scheduler implemented with blocker-aware candidate gating | Add exhaustive parity for slot starvation, retry replacement, and reconcile error branches | `R1.2`, `R2.1`, `R2.2` |
| Sec. 8.6 startup terminal workspace cleanup | Implemented | Add stronger ordering/fault-drill coverage for cleanup under hook failures and tracker partial data | `R2.2`, `R4.1` |
| Sec. 10 app-server integration and Sec. 12 prompt assembly | Worker stream monitor consumes typed protocol policy outcomes for approval/input-required and timeout paths, preserves buffered post-handshake stream bytes, returns unsupported dynamic tool-call responses, and executes `linear_graphql` with validated input and structured outputs | Expand live-integration coverage (mocked GraphQL success/error branches and startup/version variant compatibility) | `R3.2`, `R4.1` |
| Sec. 14 recovery behavior and restart semantics | Partial | Complete restart policy docs and resilient behavior for protocol/session crash recovery branches | `R2.2`, `R4.1` |
| Sec. 16.2-16.6 reference algorithms | Mostly implemented | Add algorithm-to-code traceability assertions in tests | `R4.1` |
<!-- SPEC_GAP_MAP_END -->
