# ADR Backlog

## Status Snapshot (2026-03-05)
- Completion: 80%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Capture and maintain architecture decisions with rationale and consequences.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic R1: Core Design Decisions
### Task R1.1: Reducer-first control plane
- [ ] Subtask R1.1.1: Record accepted rationale and rejected alternatives.
- [ ] Subtask R1.1.2: Define enforcement expectations in code review.

### Task R1.2: Async runtime model
- [ ] Subtask R1.2.1: Record Tokio runtime assumptions.
- [ ] Subtask R1.2.2: Record timer and task scheduling strategy.

### Task R1.3: Adapter boundary policy
- [ ] Subtask R1.3.1: Define tracker/protocol/workspace trait boundaries.
- [ ] Subtask R1.3.2: Define serialization and error-boundary policy.

## Epic R2: Security and Operations Decisions
### Task R2.1: Secret handling policy
- [ ] Subtask R2.1.1: Env indirection and logging redaction policy.
- [ ] Subtask R2.1.2: Allowed diagnostics and forbidden outputs.

### Task R2.2: Deployment and upgrade policy
- [ ] Subtask R2.2.1: Runtime versioning and compatibility policy.
- [ ] Subtask R2.2.2: Upgrade sequencing policy.

## Exit Criteria
- [ ] Every major architectural choice is represented as an ADR.
