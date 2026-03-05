# symphony-config Tasks

## Status Snapshot (2026-03-05)
- Completion: 95%
- Done: typed schema/defaults, state/path normalization, env indirection hardening, and validation range checks with expanded tests.
- In Progress: CLI override integration points.
- Remaining: final SPEC parity and rollout gates.

## Scope
Own typed runtime configuration, defaults, env resolution, and validation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Workflow source: [../symphony-workflow/TASKS.md](../symphony-workflow/TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic C1: Typed Configuration Model
### Task C1.1: Config schema types
- [x] Subtask C1.1.1: Tracker/polling/workspace/hooks/agent/codex structures.
- [x] Subtask C1.1.2: Optional fields with explicit defaults.

### Task C1.2: Normalization
- [x] Subtask C1.2.1: State-name normalization and limits normalization.
- [x] Subtask C1.2.2: Path normalization semantics.

## Epic C2: Resolution and Validation
### Task C2.1: Resolution
- [x] Subtask C2.1.1: `$VAR` env indirection for secrets/paths.
- [ ] Subtask C2.1.2: CLI override integration points.

### Task C2.2: Validation
- [x] Subtask C2.2.1: Required field validation by tracker kind.
- [x] Subtask C2.2.2: Enum/range validation for runtime fields.

## Epic C3: Tests
### Task C3.1: Parsing and fallback tests
- [x] Subtask C3.1.1: Invalid values fallback behavior.
- [x] Subtask C3.1.2: Validation error mapping behavior.

## Exit Criteria
- [x] Runtime can load and validate effective config deterministically.
