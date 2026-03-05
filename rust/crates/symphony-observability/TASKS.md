# symphony-observability Tasks

## Status Snapshot (2026-03-05)
- Completion: 60%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own runtime snapshot model, metrics aggregation, and display-ready view shaping.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime producer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)
- HTTP consumer: [../symphony-http/TASKS.md](../symphony-http/TASKS.md)

## Epic O1: Snapshot Model
### Task O1.1: Snapshot schema
- [ ] Subtask O1.1.1: Running/retry/totals/rate-limit structures.
- [ ] Subtask O1.1.2: Per-issue status view structures.

### Task O1.2: Sanitization
- [ ] Subtask O1.2.1: Strip control bytes/ansi from event text.
- [ ] Subtask O1.2.2: Secret-safe redaction helpers.

## Epic O2: Aggregation
### Task O2.1: Token and throughput accounting
- [ ] Subtask O2.1.1: Cumulative token counters.
- [ ] Subtask O2.1.2: Rolling throughput buckets.

### Task O2.2: Queue and timing views
- [ ] Subtask O2.2.1: Retry due-time and delay views.
- [ ] Subtask O2.2.2: Poll countdown/checking views.

## Epic O3: Tests
### Task O3.1: View-model correctness tests
- [ ] Subtask O3.1.1: Aggregation math tests.
- [ ] Subtask O3.1.2: Sanitization/redaction tests.

## Exit Criteria
- [ ] Snapshot model supports both JSON API and dashboard rendering.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 13.3 runtime snapshot interface | Partial | Finalize running/retrying/totals/rate-limit schemas and required fields | `O1.1` |
| Sec. 13.4 human-readable status surface | Partial helpers only | Add display-ready shaping for countdowns, queue state, and health markers | `O2.2` |
| Sec. 13.5 session metrics and token accounting | Partial | Implement full cumulative and per-issue accounting including throughput windows | `O2.1` |
| Sec. 13.6 humanized event summaries | Not complete | Add sanitization and summary utilities with deterministic formatting | `O1.2`, `O3.1` |
<!-- SPEC_GAP_MAP_END -->
