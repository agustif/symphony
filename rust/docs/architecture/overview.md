# Symphony Rust Architecture Overview

This document defines the runtime contract for the Rust orchestrator.
It is intentionally explicit about flow, invariants, and failure handling.

## Scope
The runtime uses a reducer-first control plane with adapter-isolated IO.
Core crates and responsibilities are:

1. `symphony-domain`: deterministic state machine and invariants.
2. `symphony-runtime`: orchestration loop and adapter coordination.
3. `symphony-tracker*`, `symphony-workspace`, `symphony-agent-protocol`: integration boundaries.
4. `symphony-observability`, `symphony-http`, `symphony-cli`: read models and operator surfaces.

## Runtime Data-Flow
Each tick follows one ordered pipeline.
No step may bypass the reducer.

1. `Fetch`: runtime asks adapters for external facts.
   Current example: `TrackerClient::fetch_candidates()`.
2. `Normalize`: adapter payloads become domain events.
   Current event set: `Claim`, `MarkRunning`, `Release`.
3. `Reduce`: `reduce(state, event)` computes next state plus commands.
4. `Validate`: `validate_invariants(next_state)` must pass before side effects.
5. `Execute`: runtime executes emitted commands through adapters.
   Current command set is scaffolded (`Command::None`).
6. `Publish`: runtime emits `RuntimeSnapshot` for API and operators.

### Tick Ordering Rules
1. A tick has a single linear event order.
2. Reducer execution is pure and deterministic.
3. Invariant validation gates command execution.
4. Snapshot publication never mutates domain state.

## State Model
`OrchestratorState` currently tracks three maps/sets:

- `claimed`: issues owned by this orchestrator instance.
- `running`: issues currently executing.
- `retry_attempts`: retry metadata keyed by issue.

Current runtime snapshot fields are:

- `running`: count of active work.
- `retrying`: count of active retries.

## Invariants
The runtime treats these as non-negotiable correctness properties.

| ID | Invariant | Enforcement |
| --- | --- | --- |
| INV-001 | `running ⊆ claimed` | `validate_invariants` in `symphony-domain` |
| INV-002 | `Release(issue)` clears `running`, `claimed`, and `retry_attempts` for that issue | Reducer transition semantics |
| INV-003 | Reducer output is deterministic for equal `(state, event)` input | Pure function contract |
| INV-004 | Adapter errors cannot directly mutate domain state | Runtime boundary contract |

## Failure Model
Failures are classified by boundary and impact.

| Class | Detection | Control-Plane Effect | Recovery Strategy |
| --- | --- | --- | --- |
| Adapter fetch failure | `Result::Err` from adapter calls | No reducer transition for failed input | Log, increment error metric, retry next tick |
| Invariant violation | `validate_invariants` returns `Err` | Tick is aborted before command execution | Fail closed, emit critical signal, require operator investigation |
| Command execution failure | Adapter returns error while applying command | Domain state remains canonical; side effect may be missing | Retry by policy, idempotency key required at adapter boundary |
| Observability sink failure | Snapshot/export fails | No domain mutation | Drop export, keep runtime alive, emit telemetry error |

## Current Baseline vs Target Contract
Current implementation is a skeleton proving boundary placement.
Not all target behavior is implemented yet.

- `Runtime::snapshot` currently derives `running` from tracker candidate count.
- `retrying` is currently reported as `0`.
- Command execution is scaffolded via `Command::None`.

The ADR set in `docs/adr/` defines the target behavior to implement next.
