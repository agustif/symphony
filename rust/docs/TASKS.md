# Documentation Program

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: docs index, current port-status assessment, refreshed architecture overview, crate dependency map, operational runbook, cutover checklist, protocol examples, config examples, and the missing secret-handling and cutover-policy ADRs are merged.
- In Progress: final drift cleanup across docs.
- Remaining: ongoing synchronization as parity work closes.

## Scope
Deliver complete, non-duplicative documentation for architecture, decisions, and operations.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Child Task Maps
- Architecture docs: [architecture/TASKS.md](architecture/TASKS.md)
- ADRs: [adr/TASKS.md](adr/TASKS.md)
- Current status assessment: [port-status.md](port-status.md)
- Validation evidence: [../TESTS_AND_PROOFS_SUMMARY.md](../TESTS_AND_PROOFS_SUMMARY.md)

## Epic D1: Information Architecture
### Task D1.1: Document map and ownership
- [x] Subtask D1.1.1: Establish doc index with ownership and update cadence.
- [x] Subtask D1.1.2: Define which topics live in architecture docs vs ADRs.
- [ ] Subtask D1.1.3: Enforce no duplicate decision text across files.

### Task D1.2: Reference quality bar
- [x] Subtask D1.2.1: Add explicit invariants section references to domain/runtime docs.
- [x] Subtask D1.2.2: Add protocol examples aligned with app-server contract.
- [x] Subtask D1.2.3: Add configuration field examples aligned with parser behavior.

## Epic D2: Operational Docs
### Task D2.1: Runbook
- [x] Subtask D2.1.1: Startup, health checks, and incident triage steps.
- [x] Subtask D2.1.2: Retry saturation and stalled-session response procedures.
- [x] Subtask D2.1.3: Safe shutdown and recovery procedures.

### Task D2.2: Cutover docs
- [x] Subtask D2.2.1: Shadow-mode checklist.
- [x] Subtask D2.2.2: Primary switch checklist.
- [x] Subtask D2.2.3: Rollback checklist.

## Exit Criteria
- [ ] All child task maps are complete.
- [ ] Docs reviewed against implemented APIs.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 3 system overview and Sec. 4 model docs | Partial | Complete source-of-truth architecture map with ownership and update cadence | `D1.1` |
| Sec. 5-6 workflow/config contract docs | Complete | Config examples synchronized with parser behavior | `D1.2` |
| Sec. 7-10 runtime and protocol docs | Complete | Protocol examples aligned with app-server contract | `D1.2`, `D2.1` |
| Sec. 13-15 observability and operations docs | Mostly implemented | Keep the runbook synchronized as runtime and operator surfaces evolve | `D2.1` |
| Sec. 18 rollout checklist | Implemented | Keep cutover and rollback criteria synchronized with CI and live validation evidence | `D2.2` |
<!-- SPEC_GAP_MAP_END -->
