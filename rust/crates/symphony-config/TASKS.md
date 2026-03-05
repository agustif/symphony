# symphony-config Tasks

## Status Snapshot (2026-03-05)
- Completion: 70%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own typed runtime configuration, defaults, env resolution, and validation.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Workflow source: [../symphony-workflow/TASKS.md](../symphony-workflow/TASKS.md)
- Runtime consumer: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic C1: Typed Configuration Model
### Task C1.1: Config schema types
- [ ] Subtask C1.1.1: Tracker/polling/workspace/hooks/agent/codex structures.
- [ ] Subtask C1.1.2: Optional fields with explicit defaults.

### Task C1.2: Normalization
- [ ] Subtask C1.2.1: State-name normalization and limits normalization.
- [ ] Subtask C1.2.2: Path normalization semantics.

## Epic C2: Resolution and Validation
### Task C2.1: Resolution
- [ ] Subtask C2.1.1: `$VAR` env indirection for secrets/paths.
- [ ] Subtask C2.1.2: CLI override integration points.

### Task C2.2: Validation
- [ ] Subtask C2.2.1: Required field validation by tracker kind.
- [ ] Subtask C2.2.2: Enum/range validation for runtime fields.

## Epic C3: Tests
### Task C3.1: Parsing and fallback tests
- [ ] Subtask C3.1.1: Invalid values fallback behavior.
- [ ] Subtask C3.1.2: Validation error mapping behavior.

## Exit Criteria
- [ ] Runtime can load and validate effective config deterministically.
