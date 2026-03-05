# Conformance Suite Tasks

## Status Snapshot (2026-03-05)
- Completion: 100%
- Done: workflow/config, orchestrator, tracker, and app-server conformance matrix cases are implemented and green in suite-gate CI commands, including explicit CLI `--port` precedence, protocol policy-outcome coverage (input-required, timeout, response-timeout, non-hard-fail unsupported tool-call marker events), startup tool-advertisement payload coverage, and nested ID/usage/tool-call extractor compatibility checks.
- In Progress: regression maintenance only.
- Remaining: expand matrix only when SPEC adds new obligations.

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
- [x] Subtask C3.1.1: Candidate/state fetch contract.
- [x] Subtask C3.1.2: Error mapping contract.

### Task C3.2: App-server contract
- [x] Subtask C3.2.1: Handshake and message ordering.
- [x] Subtask C3.2.2: Stdout-only protocol parsing.

## Exit Criteria
- [x] Conformance matrix fully implemented and passing.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 17.1 workflow and config parsing | Implemented for load/parse precedence and invalid-field rejection | Track SPEC deltas and add cases only when contract changes | `C1.1`, `C1.2` |
| Sec. 17.2 workspace manager and safety | Reconciliation semantics covered | Track SPEC deltas and add cases only when contract changes | `C2.2` |
| Sec. 17.3 tracker client | Candidate/state fetch and transport-error mapping covered | Track SPEC deltas and add cases only when contract changes | `C3.1` |
| Sec. 17.4 orchestrator dispatch/retry/reconcile | Candidate blocker, ordering, backoff, and reconciliation matrix covered | Track SPEC deltas and add cases only when contract changes | `C2.1`, `C2.2` |
| Sec. 17.5 app-server protocol client | Startup ordering and stdout parsing covered | Track SPEC deltas and add cases only when contract changes | `C3.2` |
<!-- SPEC_GAP_MAP_END -->
