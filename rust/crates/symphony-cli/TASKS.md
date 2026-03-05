# symphony-cli Tasks

## Status Snapshot (2026-03-05)
- Completion: 88%
- Done: task map defined, startup override/reload precedence, `--port` and `--logs-root` flags, optional HTTP wiring, signal-safe graceful shutdown, and explicit host task-failure exit-code mapping.
- In Progress: host lifecycle fault-injection and end-to-end coverage expansion.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Own executable entrypoint, startup wiring, flags, and host lifecycle behavior.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Runtime target: [../symphony-runtime/TASKS.md](../symphony-runtime/TASKS.md)

## Epic C1: CLI UX and Startup
### Task C1.1: Argument contract
- [x] Subtask C1.1.1: Positional workflow path and default behavior.
- [x] Subtask C1.1.2: Logs root and HTTP port flags.

### Task C1.2: Startup flow
- [x] Subtask C1.2.1: Validate workflow path and config before run.
- [x] Subtask C1.2.2: Structured startup diagnostics.
- [x] Subtask C1.2.3: Apply CLI override precedence in startup and workflow reload paths.

## Epic C2: Runtime Wiring
### Task C2.1: Dependency wiring
- [x] Subtask C2.1.1: Construct tracker/protocol/workspace adapters.
- [x] Subtask C2.1.2: Start runtime and optional HTTP service.

### Task C2.2: Shutdown and signals
- [x] Subtask C2.2.1: Graceful shutdown handling.
- [x] Subtask C2.2.2: Exit code mapping.

## Epic C3: Tests
### Task C3.1: CLI behavior tests
- [x] Subtask C3.1.1: Parse and validation tests.
- [x] Subtask C3.1.2: Startup failure mapping tests.
- [ ] Subtask C3.1.3: End-to-end host tests for HTTP startup/bind failures, refresh signaling, and `--logs-root` file initialization.

## Exit Criteria
- [ ] CLI provides production-grade startup and control behavior.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 6.1 startup config precedence | Implemented for core paths and port overrides | Add exhaustive precedence regressions for all field combinations in live reload paths | `C1.1`, `C3.1` |
| Sec. 13.7 optional HTTP extension wiring | Implemented with `server.port` + `--port` host startup | Add integration tests for bind-failure mapping and refresh lifecycle behavior | `C2.1`, `C3.1` |
| Sec. 14.2 failure and recovery host behavior | Explicit exit-code mapping implemented for bootstrap/runtime/watcher/http/log-init paths | Add snapshot-tested operator contract documentation for each exit-code class | `C2.2` |
| Sec. 17.7 CLI and host lifecycle validation | Partial | Add end-to-end tests for startup failure modes and controlled shutdown with HTTP enabled | `C3.1` |
<!-- SPEC_GAP_MAP_END -->
