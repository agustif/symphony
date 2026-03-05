# symphony-agent-protocol Tasks

## Status Snapshot (2026-03-05)
- Completion: 65%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own coding-agent app-server protocol transport, framing, and event translation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic P1: Protocol Framing and IO
### Task P1.1: Startup handshake support
- [ ] Subtask P1.1.1: initialize/session/new/turn/start message sequence support.
- [ ] Subtask P1.1.2: Optional tool advertisement support.

### Task P1.2: Stream parsing contract
- [ ] Subtask P1.2.1: Parse JSON protocol lines from stdout only.
- [ ] Subtask P1.2.2: Route stderr to diagnostics without protocol parsing.

## Epic P2: Event Translation
### Task P2.1: Runtime event mapping
- [ ] Subtask P2.1.1: Map protocol updates to typed events.
- [ ] Subtask P2.1.2: Map approval and input-required paths to policy outcomes.

### Task P2.2: Dynamic tool support
- [ ] Subtask P2.2.1: Unsupported tool call handling.
- [ ] Subtask P2.2.2: Hook points for optional supported tools.

## Epic P3: Tests
### Task P3.1: Protocol robustness
- [ ] Subtask P3.1.1: Partial-line and malformed JSON tests.
- [ ] Subtask P3.1.2: Approval/input-required timeout handling tests.

## Exit Criteria
- [ ] Protocol crate is robust against malformed or mixed stream inputs.
