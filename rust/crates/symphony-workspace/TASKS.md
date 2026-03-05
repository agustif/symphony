# symphony-workspace Tasks

## Status Snapshot (2026-03-05)
- Completion: 100%
- Done: containment/cwd safety invariants, deterministic lifecycle semantics, and lifecycle hook contract stubs with timeout/truncation tests.
- In Progress: none.
- Remaining: production rollout gates.

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
### Task W2.1: Workspace create/reuse/remove
- [x] Subtask W2.1.1: Deterministic create/reuse semantics.
- [x] Subtask W2.1.2: Remove by issue identifier semantics.

### Task W2.2: Hook execution
- [x] Subtask W2.2.1: after_create/before_run/after_run/before_remove execution.
- [x] Subtask W2.2.2: Hook timeout and output truncation policies.

## Epic W3: Tests
### Task W3.1: Safety and lifecycle tests
- [x] Subtask W3.1.1: Containment and symlink escape tests.
- [x] Subtask W3.1.2: Hook failure/timeout behavior tests.

## Exit Criteria
- [x] Workspace manager enforces filesystem safety invariants.
