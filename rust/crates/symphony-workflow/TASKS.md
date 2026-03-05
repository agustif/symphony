# symphony-workflow Tasks

## Status Snapshot (2026-03-05)
- Completion: 70%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own `WORKFLOW.md` discovery, parsing, reload metadata, and prompt extraction.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Config consumer: [../symphony-config/TASKS.md](../symphony-config/TASKS.md)

## Epic W1: File Discovery and Parsing
### Task W1.1: Workflow path contract
- [ ] Subtask W1.1.1: Default and explicit path handling.
- [ ] Subtask W1.1.2: Missing file error semantics.

### Task W1.2: Markdown/front-matter parsing
- [ ] Subtask W1.2.1: YAML front matter decode and type checks.
- [ ] Subtask W1.2.2: Prompt body extraction and fallback behavior.

## Epic W2: Reload and Stamping
### Task W2.1: Change detection
- [ ] Subtask W2.1.1: Timestamp/content stamp calculation.
- [ ] Subtask W2.1.2: Last-known-good retention behavior.

### Task W2.2: Error handling
- [ ] Subtask W2.2.1: Parse/load errors surfaced with typed variants.
- [ ] Subtask W2.2.2: Reload failure does not drop good state.

## Epic W3: Tests
### Task W3.1: Parser and reload tests
- [ ] Subtask W3.1.1: Prompt-only and malformed front matter cases.
- [ ] Subtask W3.1.2: Reload transition tests.

## Exit Criteria
- [ ] Workflow loader supports runtime-safe reload semantics.
