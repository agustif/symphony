# ADR-0004: Retry Semantics

## Status
Accepted

## Date
2026-03-05

## Context
The orchestrator must recover from transient failures without violating ownership and execution invariants.
Retry behavior must be deterministic and observable.

## Decision
Retries are modeled as explicit state, not implicit loops.

1. Retry attempt count is tracked per issue in domain state.
2. Retry eligibility is decided by error classification:
   - transient: retryable
   - permanent: terminal
3. Delay uses exponential backoff with jitter:
   `delay = min(base * 2^attempt, max_delay) + jitter`
4. Maximum attempts are bounded by policy.
5. Retrying an issue never bypasses ownership invariants.
6. Releasing an issue clears retry metadata.

## Consequences
- Retry state is inspectable and testable.
- Backoff reduces adapter pressure during outages.
- Terminal failures stop wasting capacity.
- Policy tuning is required per integration class.

## Rejected Alternatives
- Unbounded immediate retries.
  This can amplify outages and starve healthy work.
- Retry logic implemented only inside adapters.
  This hides control-plane state and weakens observability.
