# ADR Backlog

## Status Snapshot (2026-03-06)
- Completion: 95%
- Done: reducer-first, runtime execution, adapter boundary, retry semantics, observability contract, secret-handling/redaction, and upgrade/cutover policy ADRs are now written and accepted.
- In Progress: keeping ADR enforcement expectations aligned with code-review and rollout practice.
- Remaining: final drift cleanup when architecture policy changes again.

## Scope
Capture and maintain architecture decisions with rationale and consequences.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic R1: Core Design Decisions
### Task R1.1: Reducer-first control plane
- [x] Subtask R1.1.1: Record accepted rationale and rejected alternatives.
- [ ] Subtask R1.1.2: Define enforcement expectations in code review.

### Task R1.2: Async runtime model
- [x] Subtask R1.2.1: Record Tokio runtime assumptions.
- [x] Subtask R1.2.2: Record timer and task scheduling strategy.

### Task R1.3: Adapter boundary policy
- [x] Subtask R1.3.1: Define tracker/protocol/workspace trait boundaries.
- [x] Subtask R1.3.2: Define serialization and error-boundary policy.

## Epic R2: Security and Operations Decisions
### Task R2.1: Secret handling policy
- [x] Subtask R2.1.1: Env indirection and logging redaction policy.
- [x] Subtask R2.1.2: Allowed diagnostics and forbidden outputs.

### Task R2.2: Deployment and upgrade policy
- [x] Subtask R2.2.1: Runtime versioning and compatibility policy.
- [x] Subtask R2.2.2: Upgrade sequencing policy.

## Exit Criteria
- [ ] Every major architectural choice is represented as an ADR.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 3.2 abstraction levels and boundaries | Implemented | Keep reducer-first and adapter-boundary ADRs aligned with code-review practice | `R1.1`, `R1.3` |
| Sec. 8 scheduler model and Sec. 16 algorithms | Implemented | Keep runtime scheduling and timer policy current as lifecycle semantics evolve | `R1.2` |
| Sec. 14 recovery strategy | Implemented | Keep restart, degradation, and cutover policy ADRs aligned with rollout practice | `R2.2` |
| Sec. 15 security and safety constraints | Implemented | Keep secret-handling and redaction policy aligned with observability/runtime behavior | `R2.1` |
<!-- SPEC_GAP_MAP_END -->
