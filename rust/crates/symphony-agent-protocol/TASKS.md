# symphony-agent-protocol Tasks

## Status Snapshot (2026-03-07)
- Completion: 99%
- Done: startup/turn sequencing validator and typed protocol method categorization are implemented with coverage for malformed envelopes, including approval-required and `requestUserInput` alias normalization, typed policy-outcome mapping for approval/input-required/timeout/cancelled paths with normalized error categories (`codex_not_found`, `invalid_workspace_cwd`, `response_timeout`, `response_error`, `port_exit`), non-fatal handling for `unsupported_tool_call` marker events, startup payload builders with optional tool advertisement support, response-envelope decoding for methodless `result`/`error` payloads, and payload extractors for nested `thread_id`/`turn_id`, usage-shape variants, and tool-call id/name fields.
- In Progress: stress-path streaming robustness and rate-limit payload compatibility depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own coding-agent app-server protocol transport, framing, and event translation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic P1: Protocol Framing and IO
### Task P1.1: Startup handshake support
- [x] Subtask P1.1.1: initialize/session/new/turn/start message sequence support.
- [x] Subtask P1.1.2: Optional tool advertisement support.

### Task P1.2: Stream parsing contract
- [x] Subtask P1.2.1: Parse JSON protocol lines from stdout only.
- [x] Subtask P1.2.2: Route stderr to diagnostics without protocol parsing.
- [x] Subtask P1.2.3: Accept JSON-RPC response envelopes that carry `result` or `error` without a `method` field.

## Epic P2: Event Translation
### Task P2.1: Runtime event mapping
- [x] Subtask P2.1.1: Map protocol updates to typed events.
- [x] Subtask P2.1.2: Map approval and input-required paths to policy outcomes.
- [x] Subtask P2.1.3: Normalize approval-required method aliases into canonical categories.

### Task P2.2: Dynamic tool support
- [x] Subtask P2.2.1: Unsupported tool call handling.
- [ ] Subtask P2.2.2: Hook points for optional supported tools.

## Epic P3: Tests
### Task P3.1: Protocol robustness
- [x] Subtask P3.1.1: Partial-line and malformed JSON tests.
- [x] Subtask P3.1.2: Approval/input-required timeout handling tests.

## Exit Criteria
- [x] Protocol crate is robust against malformed or mixed stream inputs.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 10.2 session startup handshake | Sequence validator plus startup payload builders support initialize/initialized/thread-start/turn-start and optional tool advertisement, runtime launch path consumes these payload builders, and methodless response envelopes carrying startup IDs now decode correctly | Expand compatibility checks across app-server version-specific startup field variants | `P1.1`, `P1.2`, `P3.1` |
| Sec. 10.3 stdout streaming contract | Implemented and tested | Add high-volume stream backpressure and reassembly stress tests | `P1.2`, `P3.1` |
| Sec. 10.5 approval and user-input policy mapping | Typed method categories and policy outcome mapping cover approval/input-required, including alias variants; unsupported tool-call markers are non-fatal so sessions can continue | Add non-default policy strategy hooks (operator-surfacing and auto-resolve variants) plus concrete optional tool handlers | `P2.1`, `P2.2` |
| Sec. 10.6 timeout and error mapping | Typed timeout/error mapping implemented for turn-failed/cancelled plus normalized error categories (`codex_not_found`, `invalid_workspace_cwd`, `response_timeout`, `response_error`, `port_exit`) | Expand coverage for payload-shape compatibility variants and startup handshake timeout branches | `P2.1`, `P3.1` |
| Sec. 17.5 app-server client validation | Improved malformed envelope and sequence coverage with startup payload/tool advertisement, nested ID/usage extractors, and runtime handshake integration | Build expanded matrix for interrupted streams and rate-limit payload variants | `P3.1` |
<!-- SPEC_GAP_MAP_END -->
