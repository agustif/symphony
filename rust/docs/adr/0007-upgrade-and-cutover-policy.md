# ADR-0007: Upgrade and Cutover Policy

## Status
Accepted

## Date
2026-03-06

## Context

The Rust implementation is progressing toward parity with the Elixir reference,
but rollout readiness depends on more than green unit tests. We need an
explicit policy for shadowing, primary switch criteria, and rollback so status
docs and operational decisions use the same bar.

## Decision

Adopt the following rollout policy:

1. Rust does not become the primary orchestrator based on crate-level test
   health alone.
2. Promotion requires:
   - green workspace validation
   - green required conformance coverage
   - production-like live smoke evidence
   - a current runbook and rollback checklist
   - compatibility review for workflow/config contract and operator-visible
     schema additions
3. Cutover proceeds in phases:
   - shadow mode
   - operator review
   - controlled primary switch
   - post-cutover observation window
4. Any operator-visible mismatch in required behavior is a cutover blocker
   until it is either fixed or explicitly downgraded from the required set.
5. Rollback must be rehearsable using the documented checklist without
   requiring undocumented tribal knowledge.

Runtime and operator-surface compatibility policy:

- additive snapshot and API fields are allowed within the same rollout line
- breaking workflow, protocol, or operator-visible contract changes require an
  explicit status/doc update before promotion
- host-owned config changes that require restart must be documented as such in
  rollout notes and runbooks

## Consequences

- Completion percentages alone cannot justify production cutover.
- Task maps, conformance coverage, and operational docs become direct inputs to
  rollout decisions.
- The team can separate “code compiles and passes tests” from “system is ready
  to replace Elixir.”

## Rejected Alternatives

- Treat a green `cargo test --workspace` result as sufficient for promotion.
  This ignores live integration and operator-surface risk.
- Handle cutover informally in issue comments.
  This produces inconsistent and non-repeatable rollout decisions.
