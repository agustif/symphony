# Architecture Documentation Tasks

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: architecture overview now documents current crate layering, runtime data flow, state ownership, invariants, failure boundaries, and a topology diagram, and `architecture/dependencies.md` records the current workspace dependency graph and layering rules.
- In Progress: keeping the dependency map and ownership boundaries synchronized with code changes.
- Remaining: final drift cleanup as implementation details continue to close.

## Scope
Document runtime structure, data flow, invariants, and module boundaries.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic A1: System Topology
### Task A1.1: Runtime component map
- [x] Subtask A1.1.1: Diagram reducer, runtime loop, adapters, and surfaces.
- [x] Subtask A1.1.2: Show data ownership and mutability boundaries.
- [x] Subtask A1.1.3: Map crate dependencies and layering constraints.

### Task A1.2: Data-flow narrative
- [x] Subtask A1.2.1: Poll-to-dispatch path.
- [x] Subtask A1.2.2: Worker completion and retry path.
- [x] Subtask A1.2.3: Reconciliation and cleanup path.

## Epic A2: Correctness and Safety Model
### Task A2.1: Invariant definitions
- [x] Subtask A2.1.1: Claimed/running/retry relationships.
- [x] Subtask A2.1.2: Slot and per-state concurrency rules.
- [x] Subtask A2.1.3: Workspace containment and hook safety.

### Task A2.2: Failure model
- [x] Subtask A2.2.1: Tracker failures.
- [x] Subtask A2.2.2: Agent protocol failures.
- [x] Subtask A2.2.3: Runtime crash/restart behavior.

## Exit Criteria
- [x] Architecture docs cover all crates once, without duplication.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 3 system topology and dependencies | Implemented | Keep the crate dependency map and ownership boundaries synchronized with code | `A1.1` |
| Sec. 7-8 orchestration flow | Implemented | Keep dispatch/retry/reconcile narratives current as lifecycle semantics change | `A1.2` |
| Sec. 9 and Sec. 15 safety model | Implemented | Keep filesystem and hook-safety notes aligned with workspace/runtime behavior | `A2.1` |
| Sec. 14 failure model | Implemented | Keep failure-class and recovery documentation aligned with runtime/host behavior | `A2.2` |
<!-- SPEC_GAP_MAP_END -->
