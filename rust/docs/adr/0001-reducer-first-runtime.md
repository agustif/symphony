# ADR-0001: Reducer-First Runtime

## Status
Accepted

## Context
The orchestration spec depends on strict invariants around claimed/running/retry state.

## Decision
Implement a pure reducer core and keep async/IO outside reducer transitions.

## Consequences
- Easier property testing and formal verification.
- Clearer boundary between correctness logic and integration code.
