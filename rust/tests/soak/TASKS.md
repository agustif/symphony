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
