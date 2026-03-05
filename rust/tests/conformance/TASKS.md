# Conformance Suite Tasks

## Status Snapshot (2026-03-05)
- Completion: 40%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Encode specification conformance behavior from SPEC sections 17 and 18.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic C1: Config and Workflow Conformance
### Task C1.1: Workflow parsing cases
- [ ] Subtask C1.1.1: Missing/invalid workflow file handling.
- [ ] Subtask C1.1.2: Front matter type and prompt fallback behavior.

### Task C1.2: Config precedence and validation
- [ ] Subtask C1.2.1: Env indirection and defaults.
- [ ] Subtask C1.2.2: Invalid field rejection behavior.

## Epic C2: Orchestrator Conformance
### Task C2.1: Dispatch/retry semantics
- [ ] Subtask C2.1.1: Candidate selection and blocker rules.
- [ ] Subtask C2.1.2: Backoff and continuation retry behavior.

### Task C2.2: Reconciliation semantics
- [ ] Subtask C2.2.1: Terminal state stop and cleanup.
- [ ] Subtask C2.2.2: Non-active state stop without cleanup.

## Epic C3: Adapter/Protocol Conformance
### Task C3.1: Tracker contract
- [ ] Subtask C3.1.1: Candidate/state fetch contract.
- [ ] Subtask C3.1.2: Error mapping contract.

### Task C3.2: App-server contract
- [ ] Subtask C3.2.1: Handshake and message ordering.
- [ ] Subtask C3.2.2: Stdout-only protocol parsing.

## Exit Criteria
- [ ] Conformance matrix fully implemented and passing.
