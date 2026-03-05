# symphony-agent-protocol Tasks

## Status Snapshot (2026-03-05)
- Completion: 75%
- Done: startup/turn sequencing validator and typed protocol method categorization are implemented with coverage for malformed envelopes.
- In Progress: optional tool advertisement support and approval/input-required policy outcome mapping.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own coding-agent app-server protocol transport, framing, and event translation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic P1: Protocol Framing and IO
### Task P1.1: Startup handshake support
- [x] Subtask P1.1.1: initialize/session/new/turn/start message sequence support.
- [ ] Subtask P1.1.2: Optional tool advertisement support.

### Task P1.2: Stream parsing contract
- [x] Subtask P1.2.1: Parse JSON protocol lines from stdout only.
- [x] Subtask P1.2.2: Route stderr to diagnostics without protocol parsing.

## Epic P2: Event Translation
### Task P2.1: Runtime event mapping
- [x] Subtask P2.1.1: Map protocol updates to typed events.
- [ ] Subtask P2.1.2: Map approval and input-required paths to policy outcomes.

### Task P2.2: Dynamic tool support
- [ ] Subtask P2.2.1: Unsupported tool call handling.
- [ ] Subtask P2.2.2: Hook points for optional supported tools.

## Epic P3: Tests
### Task P3.1: Protocol robustness
- [x] Subtask P3.1.1: Partial-line and malformed JSON tests.
- [ ] Subtask P3.1.2: Approval/input-required timeout handling tests.

## Exit Criteria
- [x] Protocol crate is robust against malformed or mixed stream inputs.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 10.2 session startup handshake | Sequence validator enforces initialize -> initialized -> thread/start or session/new -> turn/start with continuation turn ordering | Add optional tool advertisement wiring in startup payload helpers | `P1.1` |
| Sec. 10.3 stdout streaming contract | Implemented and tested | Add high-volume stream backpressure and reassembly stress tests | `P1.2`, `P3.1` |
| Sec. 10.5 approval and user-input policy mapping | Typed method categories now classify approval and input-required methods | Map approvals/input-required to explicit runtime policy outcomes | `P2.1` |
| Sec. 10.6 timeout and error mapping | Partial | Add typed timeout/retryable/non-retryable mapping and tests | `P2.1`, `P3.1` |
| Sec. 17.5 app-server client validation | Improved malformed envelope and sequence coverage | Build expanded matrix for interrupted streams and timeout-driven approval/input paths | `P3.1` |
<!-- SPEC_GAP_MAP_END -->
