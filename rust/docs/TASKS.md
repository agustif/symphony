# Documentation Program

## Status Snapshot (2026-03-05)
- Completion: 65%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Deliver complete, non-duplicative documentation for architecture, decisions, and operations.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Child Task Maps
- Architecture docs: [architecture/TASKS.md](architecture/TASKS.md)
- ADRs: [adr/TASKS.md](adr/TASKS.md)

## Epic D1: Information Architecture
### Task D1.1: Document map and ownership
- [ ] Subtask D1.1.1: Establish doc index with ownership and update cadence.
- [ ] Subtask D1.1.2: Define which topics live in architecture docs vs ADRs.
- [ ] Subtask D1.1.3: Enforce no duplicate decision text across files.

### Task D1.2: Reference quality bar
- [ ] Subtask D1.2.1: Add explicit invariants section references to domain/runtime docs.
- [ ] Subtask D1.2.2: Add protocol examples aligned with app-server contract.
- [ ] Subtask D1.2.3: Add configuration field examples aligned with parser behavior.

## Epic D2: Operational Docs
### Task D2.1: Runbook
- [ ] Subtask D2.1.1: Startup, health checks, and incident triage steps.
- [ ] Subtask D2.1.2: Retry saturation and stalled-session response procedures.
- [ ] Subtask D2.1.3: Safe shutdown and recovery procedures.

### Task D2.2: Cutover docs
- [ ] Subtask D2.2.1: Shadow-mode checklist.
- [ ] Subtask D2.2.2: Primary switch checklist.
- [ ] Subtask D2.2.3: Rollback checklist.

## Exit Criteria
- [ ] All child task maps are complete.
- [ ] Docs reviewed against implemented APIs.
