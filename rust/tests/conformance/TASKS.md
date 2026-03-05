# Conformance Suite Tasks

## Status Snapshot (2026-03-05)
- Completion: 75%
- Done: workflow/config edge cases, dispatch/reconcile semantics, and protocol handshake parsing conformance cases are implemented.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Encode specification conformance behavior from SPEC sections 17 and 18.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic C1: Config and Workflow Conformance
### Task C1.1: Workflow parsing cases
- [x] Subtask C1.1.1: Missing/invalid workflow file handling.
- [x] Subtask C1.1.2: Front matter type and prompt fallback behavior.

### Task C1.2: Config precedence and validation
- [x] Subtask C1.2.1: Env indirection and defaults.
- [x] Subtask C1.2.2: Invalid field rejection behavior.

## Epic C2: Orchestrator Conformance
### Task C2.1: Dispatch/retry semantics
- [x] Subtask C2.1.1: Candidate selection and blocker rules.
- [x] Subtask C2.1.2: Backoff and continuation retry behavior.

### Task C2.2: Reconciliation semantics
- [x] Subtask C2.2.1: Terminal state stop and cleanup.
- [x] Subtask C2.2.2: Non-active state stop without cleanup.

## Epic C3: Adapter/Protocol Conformance
### Task C3.1: Tracker contract
- [ ] Subtask C3.1.1: Candidate/state fetch contract.
- [ ] Subtask C3.1.2: Error mapping contract.

### Task C3.2: App-server contract
- [x] Subtask C3.2.1: Handshake and message ordering.
- [x] Subtask C3.2.2: Stdout-only protocol parsing.

## Exit Criteria
- [ ] Conformance matrix fully implemented and passing.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1 workflow and config parsing | Implemented for parse/load precedence and invalid-field rejection | Add additional malformed YAML and codex policy matrix cases | `C1.1`, `C1.2` |
| Sec. 17.2 workspace manager and safety | Reconciliation semantics covered | Add root-escape, hook timeout, and workspace cleanup conformance cases | `C2.2` |
| Sec. 17.3 tracker client | Partial | Add contract cases for missing issue/state and normalized payload fields | `C3.1` |
| Sec. 17.4 orchestrator dispatch/retry/reconcile | Dispatch/retry/reconcile core matrix covered | Add workspace-hook interaction and stall-path conformance coverage | `C2.1`, `C2.2` |
| Sec. 17.5 app-server protocol client | Startup ordering and stdout parsing covered | Add approval path and timeout mapping cases | `C3.2` |
<!-- SPEC_GAP_MAP_END -->
