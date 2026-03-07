# Interleaving and Race Tests

## Status Snapshot (2026-03-07)
- Completion: 100%
- Done: 18 deterministic interleaving tests cover duplicate worker lifecycles, cross-issue isolation, `Release` vs `QueueRetry` races, `Release` vs late protocol updates, tick/retry races, reconcile/retry races, worker exit continuation races, retry pop races, retry attempt monotonicity, terminal transitions during retry, protocol update races, worker spawn during shutdown, worker death during transitions, multiple worker completions, worker restart races, multi-issue tick/retry races, and protocol update lifecycle races.
- All tests pass via `cargo test --test interleavings_suite`.

## Scope
Stress concurrent interleavings for retry, dispatch, and reconciliation correctness.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic I1: Scheduler Interleavings
### Task I1.1: Tick/retry races
- [x] Subtask I1.1.1: Simulate retry fire while poll dispatch runs.
- [x] Subtask I1.1.2: Assert no duplicate running session creation.

### Task I1.2: Reconcile/retry races
- [x] Subtask I1.2.1: Simulate terminal transitions during retry handling.
- [x] Subtask I1.2.2: Assert claim release/cancellation correctness.

## Epic I2: Worker lifecycle races
### Task I2.1: Worker exit and claim transitions
- [x] Subtask I2.1.1: Normal exit continuation scheduling race cases.
- [x] Subtask I2.1.2: Abnormal exit backoff race cases.

## Exit Criteria
- [x] Deterministic race harness catches no invariant violations.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 7.4 idempotency under repeated events | Full deterministic race coverage with 18 interleaving tests | Consider adding stress/fuzz races for real async scheduler integration | Complete |
| Sec. 8.3 concurrency slot safety | Full deterministic race coverage | Consider stress races under slot pressure | Complete |
| Sec. 8.5 reconcile with concurrent retry events | Full deterministic race coverage | Complete | Complete |
| Sec. 14.2 recovery behavior under races | Full deterministic race coverage with claim release and retry continuity | Complete | Complete |
<!-- SPEC_GAP_MAP_END -->
