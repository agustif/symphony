# symphony-cli Tasks

## Status Snapshot (2026-03-05)
- Completion: 55%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own executable entrypoint, startup wiring, flags, and host lifecycle behavior.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime target: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic C1: CLI UX and Startup
### Task C1.1: Argument contract
- [ ] Subtask C1.1.1: Positional workflow path and default behavior.
- [ ] Subtask C1.1.2: Logs root and HTTP port flags.

### Task C1.2: Startup flow
- [x] Subtask C1.2.1: Validate workflow path and config before run.
- [x] Subtask C1.2.2: Structured startup diagnostics.

## Epic C2: Runtime Wiring
### Task C2.1: Dependency wiring
- [ ] Subtask C2.1.1: Construct tracker/protocol/workspace adapters.
- [ ] Subtask C2.1.2: Start runtime and optional HTTP service.

### Task C2.2: Shutdown and signals
- [ ] Subtask C2.2.1: Graceful shutdown handling.
- [ ] Subtask C2.2.2: Exit code mapping.

## Epic C3: Tests
### Task C3.1: CLI behavior tests
- [x] Subtask C3.1.1: Parse and validation tests.
- [x] Subtask C3.1.2: Startup failure mapping tests.

## Exit Criteria
- [ ] CLI provides production-grade startup and control behavior.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 6.1 startup config precedence | Implemented for core paths | Complete all documented CLI overrides including logs root and HTTP options | `C1.1` |
| Sec. 13.7 optional HTTP extension wiring | Partial | Add explicit CLI switches and startup wiring for HTTP server lifecycle | `C2.1` |
| Sec. 14.2 failure and recovery host behavior | Partial | Implement signal-safe graceful shutdown and exit code contract | `C2.2` |
| Sec. 17.7 CLI and host lifecycle validation | Partial | Add integration tests for startup failure modes and controlled shutdown | `C3.1` |
<!-- SPEC_GAP_MAP_END -->
