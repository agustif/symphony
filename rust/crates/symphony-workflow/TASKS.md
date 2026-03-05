# symphony-workflow Tasks

## Status Snapshot (2026-03-05)
- Completion: 100%
- Done: file discovery, parser hardening, reload stamps, and retention semantics implemented with transition tests.
- In Progress: none.
- Remaining: production rollout gates.

## Scope
Own `WORKFLOW.md` discovery, parsing, reload metadata, and prompt extraction.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Config consumer: [../symphony-config/TASKS.md](../symphony-config/TASKS.md)

## Epic W1: File Discovery and Parsing
### Task W1.1: Workflow path contract
- [x] Subtask W1.1.1: Default and explicit path handling.
- [x] Subtask W1.1.2: Missing file error semantics.

### Task W1.2: Markdown/front-matter parsing
- [x] Subtask W1.2.1: YAML front matter decode and type checks.
- [x] Subtask W1.2.2: Prompt body extraction and fallback behavior.

## Epic W2: Reload and Stamping
### Task W2.1: Change detection
- [x] Subtask W2.1.1: Timestamp/content stamp calculation.
- [x] Subtask W2.1.2: Last-known-good retention behavior.

### Task W2.2: Error handling
- [x] Subtask W2.2.1: Parse/load errors surfaced with typed variants.
- [x] Subtask W2.2.2: Reload failure does not drop good state.

## Epic W3: Tests
### Task W3.1: Parser and reload tests
- [x] Subtask W3.1.1: Prompt-only and malformed front matter cases.
- [x] Subtask W3.1.2: Reload transition tests.

## Exit Criteria
- [x] Workflow loader supports runtime-safe reload semantics.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 5.1 file discovery and path resolution | Implemented | Keep cross-platform path edge-case regression checks in CI | `W1.1` |
| Sec. 5.2-5.3 markdown and front matter schema | Implemented | Add drift checks against future SPEC schema additions | `W1.2`, `W3.1` |
| Sec. 5.4 prompt template extraction | Implemented | Add additional template fixture coverage for multiline placeholder blocks | `W3.1` |
| Sec. 5.5 error surface | Implemented | Preserve typed error compatibility in integration tests | `W2.2`, `W3.1` |
<!-- SPEC_GAP_MAP_END -->
