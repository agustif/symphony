# Verus Proof Tasks

## Status Snapshot (2026-03-07)
- Completion: 97%
- Done: task map defined, proof runner wired, printable guide snapshot vendor flow added, stale-snapshot CI guard wired, reducer/workspace proof modules implemented, slot-accounting and rejection-case preservation lemmas landed, reducer command-taxonomy lemmas landed, a dedicated `agent_update_safety.rs` module now proves missing-running rejection, topology preservation, monotonic token totals, and session-rebase semantics, fair-step liveness assumptions were added for dispatch/retry/release, workspace separator/fixed-point safety lemmas landed, and both quick and full profiles passed on the current proof sources.
- In Progress: deeper scheduler fairness modeling, remaining workspace policy-composition edge cases, and turning local proof evidence into a harder release gate.
- Remaining: stronger multi-step progress proofs and production rollout gates.

## Scope
Implement Verus proofs for runtime-critical state invariants.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Epic V1: Reducer Safety Proofs
### Task V1.1: Claim/running relation proof
- [x] Subtask V1.1.1: Prove running implies claimed for all transitions.
- [x] Subtask V1.1.2: Prove release removes running and claim entries.

### Task V1.2: Uniqueness and monotonicity proofs
- [x] Subtask V1.2.1: Prove single running entry per issue.
- [x] Subtask V1.2.2: Prove retry attempt monotonicity under requeue.

## Epic V2: Retry and Dispatch Proofs
### Task V2.1: Dispatch soundness
- [x] Subtask V2.1.1: Prove dispatch preconditions prevent duplicate run creation.
- [x] Subtask V2.1.2: Prove slot accounting never goes negative.
- [x] Subtask V2.1.3: Prove reducer rejection cases are state-preserving no-ops.

### Task V2.2: Reconciliation correctness
- [x] Subtask V2.2.1: Prove terminal/non-active transitions eventually release claim.
- [x] Subtask V2.2.2: Prove continuation retry semantics preserve invariants.

## Epic V3: Tooling and CI
### Task V3.1: Proof harness integration
- [x] Subtask V3.1.1: Add proof runner scripts.
- [x] Subtask V3.1.2: Integrate proof checks into CI.

### Task V3.2: Verus reference operations
- [x] Subtask V3.2.1: Vendor printable Verus guide snapshot under `reference/`.
- [x] Subtask V3.2.2: Add reproducible snapshot sync script.
- [x] Subtask V3.2.3: Add CI guard for stale snapshot metadata.

## Exit Criteria
- [x] All V1 and V2 proofs pass in CI.

## Proof Traceability Matrix

| Module | Reducer/runtime anchor | Executable regression coverage |
| --- | --- | --- |
| `runtime_quick.rs` | `symphony-domain::validate_invariants` | Domain invariant/unit tests, property-based event streams, `symphony-testkit::validate_trace` |
| `runtime_full.rs` | `symphony-domain::reduce` lifecycle transitions | Domain lifecycle tests, conformance lifecycle cases, retry integration suites |
| `agent_update_safety.rs` | `crates/symphony-domain/src/agent_update.rs` | Domain `UpdateAgent` tests, token monotonicity properties, observability/orchestrator conformance cases |
| `session_liveness.rs` | `symphony-runtime` dispatch/retry/release loops | Runtime conformance, retry exhaustion integration tests, interleavings suites |
| `workspace_safety.rs` | `symphony-workspace` path validation | Workspace conformance and workspace lifecycle tests |

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 7 state machine invariants | `runtime_quick.rs`, `runtime_full.rs`, and `agent_update_safety.rs` are verified, including slot-accounting bounds, rejection-case state preservation, command-taxonomy lemmas for valid lifecycle chains, and `UpdateAgent` accounting safety | Extend command/result correspondence across longer multi-event traces | `V2.1` |
| Sec. 8 retry/reconciliation progress obligations | `session_liveness.rs` verified with explicit fair-step dispatch/retry/release assumptions | Extend from one-step fairness assumptions to stronger eventual-processing and starvation-freedom models | `V2.2` |
| Sec. 9.5 workspace safety invariants | `workspace_safety.rs` verified, including separator removal, backslash removal, placeholder sanitization, and sanitize fixed-point obligations for allowed keys | Add stronger policy-composition obligations for dot-segment exclusion after sanitization and wider path-canonicalization modeling | `V2.2` |
| Sec. 17/18 verification gate requirements | Proof scripts and stale-reference guard wired in CI; quick and full profiles both reran locally on the current tree, including the dedicated agent-update proof phase | Enforce branch protection and expand required suite matrix | `V3.1`, `V3.2` |
<!-- SPEC_GAP_MAP_END -->
