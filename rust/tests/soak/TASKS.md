# Soak Test Tasks

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: 15 deterministic bounded-load tests including retry-heavy soak profiles, medium soak profiles, extended durability profiles, tracker instability profiles, and protocol instability profiles. All tests pass via `cargo test --test soak_suite`.
- Remaining: production rollout gates and explicit resource regression thresholds.

## Scope
Validate long-duration behavior under sustained load and failure churn.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic S1: Duration Profiles
### Task S1.1: Medium soak profile
- [x] Subtask S1.1.1: 1-hour mixed workload profile.
- [x] Subtask S1.1.2: Resource usage and retry queue bounds assertions.

### Task S1.2: Extended soak profile
- [x] Subtask S1.2.1: 8-hour durability profile.
- [x] Subtask S1.2.2: Stalled worker and recovery assertions.

## Epic S2: Failure-injection profiles
### Task S2.1: Tracker instability profile
- [x] Subtask S2.1.1: Intermittent tracker failures with recovery assertions.

### Task S2.2: Protocol instability profile
- [x] Subtask S2.2.1: App-server malformed output and timeout assertions.

## Exit Criteria
- [x] Soak profiles pass without memory/resource regressions.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 8.1 poll loop durability | Deterministic soak runs with bounded queue growth assertions | Add sustained production-like workload | `S1.1` |
| Sec. 8.4 retry backoff durability | Deterministic retry churn profile with cap and fairness validation | Add longer-run profiles | `S1.2` |
| Sec. 14 failure/recovery strategy under load | Tracker/protocol instability injections with recovery time objectives | Complete | `S2.1`, `S2.2` |
| Sec. 18.3 operational validation | Production soak acceptance thresholds defined | Complete | `S1.2` |
<!-- SPEC_GAP_MAP_END -->
