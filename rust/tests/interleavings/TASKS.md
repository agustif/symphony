# Interleaving and Race Tests

## Status Snapshot (2026-03-05)
- Completion: 45%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Stress concurrent interleavings for retry, dispatch, and reconciliation correctness.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic I1: Scheduler Interleavings
### Task I1.1: Tick/retry races
- [ ] Subtask I1.1.1: Simulate retry fire while poll dispatch runs.
- [ ] Subtask I1.1.2: Assert no duplicate running session creation.

### Task I1.2: Reconcile/retry races
- [ ] Subtask I1.2.1: Simulate terminal transitions during retry handling.
- [ ] Subtask I1.2.2: Assert claim release/cancellation correctness.

## Epic I2: Worker lifecycle races
### Task I2.1: Worker exit and claim transitions
- [ ] Subtask I2.1.1: Normal exit continuation scheduling race cases.
- [ ] Subtask I2.1.2: Abnormal exit backoff race cases.

## Exit Criteria
- [ ] Deterministic race harness catches no invariant violations.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 7.4 idempotency under repeated events | Partial | Add duplicate/reordered event schedules for reducer/runtime integration | `I1.1` |
| Sec. 8.3 concurrency slot safety | Partial | Add stress races proving no duplicate dispatch under slot pressure | `I1.1`, `I1.2` |
| Sec. 8.5 reconcile with concurrent retry events | Partial | Add terminal/non-active transition races during retry pop/requeue | `I1.2` |
| Sec. 14.2 recovery behavior under races | Partial | Add deterministic assertions for claim release and retry continuity | `I2.1` |
<!-- SPEC_GAP_MAP_END -->
