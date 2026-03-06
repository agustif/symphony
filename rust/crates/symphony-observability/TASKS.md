# symphony-observability Tasks

## Status Snapshot (2026-03-06)
- Completion: 86%
- Done: snapshot struct shells, runtime-fed session metadata, absolute token accounting, retry views, rate-limit payload propagation, runtime-seconds totals, poll/last-activity timing views, rolling throughput windows, HTTP-facing view types, and degraded snapshot envelope types are implemented.
- In Progress: secret-safe summary shaping and richer operator/dashboard metrics.
- Remaining: finish redaction-safe summary formatting and any residual operator-surface validation gaps.

## Scope
Own runtime snapshot model, metrics aggregation, and display-ready view shaping.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime producer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)
- HTTP consumer: [../symphony-http/TASKS.md](../symphony-http/TASKS.md)

## Epic O1: Snapshot Model
### Task O1.1: Snapshot schema
- [x] Subtask O1.1.1: Baseline running, retrying, totals, and rate-limit struct shells.
- [x] Subtask O1.1.2: Spec-accurate per-issue status, running, and retry payload fields.
- [x] Subtask O1.1.3: Snapshot error and degraded-state view types.

### Task O1.2: Sanitization
- [x] Subtask O1.2.1: Strip control bytes and ANSI sequences from event text.
- [ ] Subtask O1.2.2: Secret-safe redaction helpers and deterministic summary formatting.

## Epic O2: Aggregation Semantics
### Task O2.1: Token and rate-limit accounting
- [x] Subtask O2.1.1: Track absolute thread totals from protocol-defined cumulative payloads only.
- [x] Subtask O2.1.2: Track latest rate-limit payload and live runtime-seconds totals.
- [x] Subtask O2.1.3: Add per-issue token totals and rolling throughput windows.

### Task O2.2: Queue and timing views
- [x] Subtask O2.2.1: Retry due-time, delay, attempt, and last-error views.
- [x] Subtask O2.2.2: Poll countdown, poll-in-progress, and last-activity views.

## Epic O3: Runtime Integration
### Task O3.1: Protocol-driven state views
- [x] Subtask O3.1.1: Feed session IDs, turn counts, event summaries, and timestamps from live protocol updates.
- [x] Subtask O3.1.2: Distinguish running and retrying issue views without placeholder blocks.

## Epic O4: Tests
### Task O4.1: View-model correctness tests
- [x] Subtask O4.1.1: Aggregation math tests for totals, rate limits, and throughput windows.
- [x] Subtask O4.1.2: Sanitization regression tests.
- [x] Subtask O4.1.3: Protocol-payload compatibility tests for cumulative token and rate-limit events.

## Exit Criteria
- [ ] Snapshot model supports both JSON API and dashboard rendering with spec-accurate data.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 13.3 runtime snapshot interface | Mostly implemented | Add any still-missing issue-detail fields needed by operator surfaces and validate degraded envelopes end to end | `O1.1`, `O3.1`, `O4.1` |
| Sec. 13.4 human-readable status surface | Mostly implemented | Finish richer degraded markers and any remaining operator polish beyond the current countdown/activity views | `O2.2`, `O3.1` |
| Sec. 13.5 session metrics and token accounting | Mostly implemented | Keep throughput math validated as the session metric surface grows | `O2.1`, `O4.1` |
| Sec. 13.6 humanized event summaries | Partial | Add deterministic summary formatting and redaction rules | `O1.2`, `O4.1` |
| Sec. 17.6 observability | Mostly implemented | Finish the remaining redaction and end-to-end operator validation paths | `O1.2`, `O3.1`, `O4.1` |
<!-- SPEC_GAP_MAP_END -->
