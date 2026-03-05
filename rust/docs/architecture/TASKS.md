# Architecture Documentation Tasks

## Status Snapshot (2026-03-05)
- Completion: 70%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Document runtime structure, data flow, invariants, and module boundaries.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic A1: System Topology
### Task A1.1: Runtime component map
- [ ] Subtask A1.1.1: Diagram reducer, runtime loop, adapters, and surfaces.
- [ ] Subtask A1.1.2: Show data ownership and mutability boundaries.
- [ ] Subtask A1.1.3: Map crate dependencies and layering constraints.

### Task A1.2: Data-flow narrative
- [ ] Subtask A1.2.1: Poll-to-dispatch path.
- [ ] Subtask A1.2.2: Worker completion and retry path.
- [ ] Subtask A1.2.3: Reconciliation and cleanup path.

## Epic A2: Correctness and Safety Model
### Task A2.1: Invariant definitions
- [ ] Subtask A2.1.1: Claimed/running/retry relationships.
- [ ] Subtask A2.1.2: Slot and per-state concurrency rules.
- [ ] Subtask A2.1.3: Workspace containment and hook safety.

### Task A2.2: Failure model
- [ ] Subtask A2.2.1: Tracker failures.
- [ ] Subtask A2.2.2: Agent protocol failures.
- [ ] Subtask A2.2.3: Runtime crash/restart behavior.

## Exit Criteria
- [ ] Architecture docs cover all crates once, without duplication.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 3 system topology and dependencies | Partial | Complete layered crate dependency and data-ownership diagrams | `A1.1` |
| Sec. 7-8 orchestration flow | Partial | Provide end-to-end state transition narratives for dispatch/retry/reconcile | `A1.2` |
| Sec. 9 and Sec. 15 safety model | Partial | Expand documented invariants for filesystem and hook boundaries | `A2.1` |
| Sec. 14 failure model | Partial | Document failure classes to recovery actions and operator controls | `A2.2` |
<!-- SPEC_GAP_MAP_END -->
