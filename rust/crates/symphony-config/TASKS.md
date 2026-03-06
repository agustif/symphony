# symphony-config Tasks

## Status Snapshot (2026-03-06)
- Completion: 98%
- Done: typed schema/defaults, state/path normalization, env indirection hardening, validation range checks with expanded tests, degraded startup parity when tracker auth is absent, structured Codex policy-field parity for object-form workflow data, and `server.port` front-matter plus CLI override support.
- In Progress: exhaustive precedence matrix generation across all overrideable fields.
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
- [x] Subtask C2.1.2: CLI override integration points.

### Task C2.2: Validation
- [x] Subtask C2.2.1: Required field validation by tracker kind.
- [x] Subtask C2.2.2: Enum/range validation for runtime fields.

## Epic C3: Tests
### Task C3.1: Parsing and fallback tests
- [x] Subtask C3.1.1: Invalid values fallback behavior.
- [x] Subtask C3.1.2: Validation error mapping behavior.

## Exit Criteria
- [x] Runtime can load and validate effective config deterministically.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 6.1 resolution precedence and env indirection | Implemented, including `server.port` front-matter parsing and CLI override support | Add generated exhaustive precedence matrix coverage for every overrideable field | `C2.1`, `C3.1` |
| Sec. 6.2 dynamic reload semantics | Partial via typed model | Verify runtime application semantics with config stamp/version transitions | `C2.1`, `C3.1` |
| Sec. 6.3 dispatch preflight validation | Mostly implemented | Add explicit preflight error taxonomy mapping for operator diagnostics | `C2.2` |
| Sec. 6.4 field cheat-sheet parity | Implemented in schema | Add generated spec-drift check between schema fields and SPEC examples | `C3.1` |
| Sec. 15.3 secret handling requirements | Partial | Add redaction guarantees for any config-derived diagnostics paths | `C2.2` |
<!-- SPEC_GAP_MAP_END -->
