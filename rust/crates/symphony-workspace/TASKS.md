# symphony-workspace Tasks

## Status Snapshot (2026-03-05)
- Completion: 65%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own workspace path safety, lifecycle hooks, and issue workspace cleanup.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic W1: Path Safety
### Task W1.1: Sanitization and containment
- [ ] Subtask W1.1.1: Issue identifier sanitization for directory names.
- [ ] Subtask W1.1.2: Root containment checks with symlink defense.

### Task W1.2: CWD contract
- [ ] Subtask W1.2.1: Validate worker cwd is issue workspace, not root.
- [ ] Subtask W1.2.2: Validate root itself cannot be treated as workspace.

## Epic W2: Lifecycle and Hooks
### Task W2.1: Workspace create/reuse/remove
- [ ] Subtask W2.1.1: Deterministic create/reuse semantics.
- [ ] Subtask W2.1.2: Remove by issue identifier semantics.

### Task W2.2: Hook execution
- [ ] Subtask W2.2.1: after_create/before_run/after_run/before_remove execution.
- [ ] Subtask W2.2.2: Hook timeout and output truncation policies.

## Epic W3: Tests
### Task W3.1: Safety and lifecycle tests
- [ ] Subtask W3.1.1: Containment and symlink escape tests.
- [ ] Subtask W3.1.2: Hook failure/timeout behavior tests.

## Exit Criteria
- [ ] Workspace manager enforces filesystem safety invariants.
