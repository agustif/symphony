# symphony-workspace Tasks

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: containment and cwd safety invariants, baseline create/reuse/remove behavior, transient cleanup on reuse, hook timeout/truncation handling, typed hook failure taxonomy, long-path rejection, and permission-edge tests are implemented.
- In Progress: consumer-level cleanup/logging simplification outside this crate.
- Remaining: cross-crate conformance and operational edge-case validation (deferred to integration testing).

## Scope
Own workspace path safety, lifecycle hooks, and issue workspace cleanup.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic W1: Path Safety
### Task W1.1: Sanitization and containment
- [x] Subtask W1.1.1: Issue identifier sanitization for directory names.
- [x] Subtask W1.1.2: Root containment checks with symlink defense.

### Task W1.2: CWD contract
- [x] Subtask W1.2.1: Validate worker cwd is issue workspace, not root.
- [x] Subtask W1.2.2: Validate root itself cannot be treated as workspace.

## Epic W2: Lifecycle and Hooks
### Task W2.1: Workspace create, reuse, and remove
- [x] Subtask W2.1.1: Baseline deterministic create and reuse semantics.
- [x] Subtask W2.1.2: Transient cleanup on reuse and remove-by-issue rollback semantics.

### Task W2.2: Hook execution
- [x] Subtask W2.2.1: `after_create`, `before_run`, and `before_remove` execution.
- [x] Subtask W2.2.2: Treat `after_run` as best-effort and surface hook failures with richer taxonomy.
- [x] Subtask W2.2.3: Hook timeout and output truncation policies.

## Epic W3: Tests
### Task W3.1: Safety and lifecycle tests
- [x] Subtask W3.1.1: Containment and symlink escape tests.
- [x] Subtask W3.1.2: Long-path and permission-edge behavior tests remain after closing cleanup-failure and rollback coverage.

## Exit Criteria
- [x] Workspace manager enforces filesystem safety invariants and spec lifecycle semantics.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 9.1-9.2 workspace layout, create, and reuse | Complete | Long-path rejection and permission-edge tests implemented | `W3.1` |
| Sec. 9.4 workspace hooks | Complete | Consumer-side logging/fallback paths can now be simplified | `W2.2`, `W3.1` |
| Sec. 9.5 safety invariants and Sec. 15.2 filesystem safety | Complete | Long-path, permission, and stress regression coverage implemented | `W3.1` |
| Sec. 17.2 workspace validation matrix | Complete | Cross-crate conformance covered via integration tests | `W3.1` |
<!-- SPEC_GAP_MAP_END -->
