# ADR-0003: Adapter Boundary Contract

## Status
Accepted

## Date
2026-03-05

## Context
Adapters interface with trackers, workspaces, and agent runtimes.
Unbounded adapter behavior can violate orchestrator safety properties.

## Decision
Define adapter boundaries as strict contracts:

1. Adapters return typed `Result<T, E>` values only.
2. Adapters convert protocol payloads into domain-safe types at boundaries.
3. Adapters never mutate orchestrator state directly.
4. All domain mutations occur through reducer events.
5. Outbound commands must be idempotent by issue identity and attempt identity.
6. Adapter errors are recoverable runtime signals, not panics.

## Consequences
- Runtime correctness remains independent from integration complexity.
- Contract tests can validate adapters without reducer internals.
- Integrations can be swapped with bounded blast radius.
- More upfront schema and error taxonomy work is required.

## Rejected Alternatives
- Let each adapter implement custom orchestration rules.
  This fragments correctness logic and breaks consistency.
- Permit shared mutable state between adapters and runtime core.
  This increases race and reentrancy risk.
