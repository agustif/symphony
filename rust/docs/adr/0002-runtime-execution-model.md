# ADR-0002: Runtime Execution Model

## Status
Accepted

## Date
2026-03-05

## Context
The orchestrator must remain deterministic while integrating with async, failure-prone systems.
We need a runtime model that is auditable and testable.

## Decision
Adopt a tick-based execution model with strict stage ordering:

1. Fetch external facts from adapters.
2. Normalize facts into domain events.
3. Apply reducer transitions.
4. Validate invariants.
5. Execute side effects from reducer-emitted commands.
6. Publish observability snapshots.

Reducer transitions are the only legal way to mutate orchestration state.
Invariant checks are a hard gate before side effects.

## Consequences
- Enables deterministic replay for reducer logic.
- Keeps IO failures out of the domain mutation path.
- Makes safety checks explicit and centrally enforced.
- Adds discipline cost for adapter and scheduler implementations.

## Rejected Alternatives
- Direct mutable runtime state across async tasks.
  This weakens determinism and complicates verification.
- Adapter-driven state mutation.
  This couples correctness to network behavior.
