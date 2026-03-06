# symphony-tracker-linear Tasks

## Status Snapshot (2026-03-06)
- Completion: 93%
- Done: task map defined, project-scoped candidate/state queries landed, page-size parity is fixed at `50`, normalized labels/blockers/priority/timestamps now match the current spec, bounded timeout/retry transport policy with transient status/timeout coverage is implemented, and live Linear auth-header compatibility now matches Elixir by sending the raw API token while tolerating accidental `Bearer ` prefixes in configured secrets.
- In Progress: pagination edge/missing-node coverage and the remaining live API compatibility drills.
- Remaining: full SPEC parity and production rollout gates.

## Scope
Implement Linear GraphQL adapter against `symphony-tracker` contract.

## Parent Link
- Parent: [../../TASKS.md](../../TASKS.md)
- Contract: [../symphony-tracker/TASKS.md](../symphony-tracker/TASKS.md)

## Epic L1: GraphQL Client and Transport
### Task L1.1: Request pipeline
- [x] Subtask L1.1.1: Endpoint/auth setup.
- [x] Subtask L1.1.2: Request timeout/retry policy (non-orchestrator retries only).

### Task L1.2: Response handling
- [x] Subtask L1.2.1: HTTP status validation.
- [x] Subtask L1.2.2: GraphQL error extraction.

## Epic L2: Query Implementations
### Task L2.1: Candidate query
- [x] Subtask L2.1.1: Project + active state filter.
- [x] Subtask L2.1.2: Pagination handling.

### Task L2.2: Refresh and terminal queries
- [x] Subtask L2.2.1: Issue IDs refresh query.
- [x] Subtask L2.2.2: Terminal states query semantics via state-filtered project query.

## Epic L3: Normalization and tests
### Task L3.1: Payload normalization
- [x] Subtask L3.1.1: Normalize blockers, labels, priority, timestamps, branch metadata, and URLs.
- [x] Subtask L3.1.2: Verify strict required field handling.

### Task L3.2: Adapter tests
- [x] Subtask L3.2.1: Mock server query/response tests.
- [x] Subtask L3.2.2: Error-path coverage tests.

## Exit Criteria
- [ ] Linear adapter fully satisfies tracker trait behavior.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 11.2 Linear query semantics | Implemented with bounded transport timeout/backoff behavior and live-compatible auth header semantics | Add the remaining live API compatibility drills beyond header/auth behavior | `L3.2` |
| Sec. 11.3 normalization rules | Implemented for current normalized contract | Keep coverage current as the tracker contract evolves | `L3.1`, `L3.2` |
| Sec. 11.4 error handling contract | Implemented for status/graphql/payload/timeout boundaries | Keep transport retry boundaries covered as the adapter evolves | `L1.1`, `L3.2` |
| Sec. 17.3 tracker validation matrix | Partial | Expand adapter contract tests to include pagination edge, missing-node, and partial-data cases | `L3.2` |
<!-- SPEC_GAP_MAP_END -->
