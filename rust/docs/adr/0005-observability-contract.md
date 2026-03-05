# ADR-0005: Observability Contract

## Status
Accepted

## Date
2026-03-05

## Context
Operators need stable runtime introspection without coupling diagnostics to internal implementation details.
Observability output must be safe during partial failures.

## Decision
Adopt a versioned observability contract centered on runtime snapshots and lifecycle events.

1. `RuntimeSnapshot` is the canonical read model for operator-facing state.
2. Snapshot fields are additive-only within a schema version.
3. Each tick emits counters for running, retrying, and adapter errors.
4. Invariant violations emit high-severity events with issue identifiers.
5. Observability pipeline failures never mutate orchestrator state.
6. Metric cardinality is bounded by stable labels only.

## Consequences
- Dashboards and alerts can evolve safely against versioned schemas.
- Incident triage uses consistent event semantics across adapters.
- Runtime remains available even when telemetry backends fail.
- Teams must maintain schema compatibility guarantees.

## Rejected Alternatives
- Ad hoc metrics emitted directly from each crate.
  This causes schema drift and inconsistent alert behavior.
- Blocking orchestrator progress on telemetry delivery.
  This turns monitoring outages into control-plane outages.
