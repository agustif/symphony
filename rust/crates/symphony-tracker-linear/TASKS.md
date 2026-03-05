# symphony-tracker-linear Tasks

## Status Snapshot (2026-03-05)
- Completion: 65%
- Done: task map defined and initial implementation batch merged.
- In Progress: hardening, edge-case conformance, and proof depth.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Implement Linear GraphQL adapter against `symphony-tracker` contract.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Contract: [../symphony-tracker/TASKS.md](../symphony-tracker/TASKS.md)

## Epic L1: GraphQL Client and Transport
### Task L1.1: Request pipeline
- [x] Subtask L1.1.1: Endpoint/auth setup.
- [ ] Subtask L1.1.2: Request timeout/retry policy (non-orchestrator retries only).

### Task L1.2: Response handling
- [x] Subtask L1.2.1: HTTP status validation.
- [x] Subtask L1.2.2: GraphQL error extraction.

## Epic L2: Query Implementations
### Task L2.1: Candidate query
- [ ] Subtask L2.1.1: Project + active state filter.
- [x] Subtask L2.1.2: Pagination handling.

### Task L2.2: Refresh and terminal queries
- [x] Subtask L2.2.1: Issue IDs refresh query.
- [ ] Subtask L2.2.2: Terminal states query.

## Epic L3: Normalization and tests
### Task L3.1: Payload normalization
- [ ] Subtask L3.1.1: Normalize blockers, labels, assignee, priority, timestamps.
- [x] Subtask L3.1.2: Verify strict required field handling.

### Task L3.2: Adapter tests
- [x] Subtask L3.2.1: Mock server query/response tests.
- [x] Subtask L3.2.2: Error-path coverage tests.

## Exit Criteria
- [ ] Linear adapter fully satisfies tracker trait behavior.
