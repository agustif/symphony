# Soak Test Tasks

## Status Snapshot (2026-03-05)
- Completion: 45%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Validate long-duration behavior under sustained load and failure churn.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic S1: Duration Profiles
### Task S1.1: Medium soak profile
- [ ] Subtask S1.1.1: 1-hour mixed workload profile.
- [ ] Subtask S1.1.2: Resource usage and retry queue bounds assertions.

### Task S1.2: Extended soak profile
- [ ] Subtask S1.2.1: 8-hour durability profile.
- [ ] Subtask S1.2.2: Stalled worker and recovery assertions.

## Epic S2: Failure-injection profiles
### Task S2.1: Tracker instability profile
- [ ] Subtask S2.1.1: Intermittent tracker failures with recovery assertions.

### Task S2.2: Protocol instability profile
- [ ] Subtask S2.2.1: App-server malformed output and timeout assertions.

## Exit Criteria
- [ ] Soak profiles pass without memory/resource regressions.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 8.1 poll loop durability | Partial | Add sustained mixed-workload soak with bounded queue growth assertions | `S1.1` |
| Sec. 8.4 retry backoff durability | Partial | Add long-run retry churn profile with cap and fairness validation | `S1.2` |
| Sec. 14 failure/recovery strategy under load | Partial | Add tracker/protocol instability injections with recovery time objectives | `S2.1`, `S2.2` |
| Sec. 18.3 operational validation | Partial | Define production soak acceptance thresholds and artifact requirements | `S1.2` |
<!-- SPEC_GAP_MAP_END -->
