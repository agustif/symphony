# Formal Verification Program

## Status Snapshot (2026-03-08)
- Completion: 96%
- Done: task map defined, proof runner wired, printable guide snapshot vendor flow added, stale-snapshot CI guard wired, reducer/workspace proof modules implemented, slot-accounting and rejection-case preservation lemmas landed, reducer command-taxonomy lemmas landed, dedicated `UpdateAgent` accounting proofs landed, fair-step liveness assumptions were added for dispatch/retry/release, workspace separator/fixed-point safety lemmas landed, quick/full profiles passed on the current tree, the emergency bypass policy is documented, and the proof-touch guard script is checked in.
- In Progress: stronger fairness models and proof-to-spec traceability depth.
- Remaining: full SPEC parity for policy-composition edge cases and stronger production-rollout evidence.

## Scope
Define, implement, and enforce formal proofs for orchestrator invariants.

## Parent Link
- Parent: [../TASKS.md](../TASKS.md)

## Child Task Map
- Verus-specific tasks: [verus/TASKS.md](verus/TASKS.md)

## Epic P1: Proof Planning and Traceability
### Task P1.1: Invariant-to-proof mapping
- [x] Subtask P1.1.1: Map each domain invariant to one proof artifact.
- [x] Subtask P1.1.2: Map each proof artifact to reducer functions.
- [x] Subtask P1.1.3: Map each proof artifact to runtime assertions/tests.

### Task P1.2: CI policy
- [x] Subtask P1.2.1: Define required proof jobs for merge.
- [x] Subtask P1.2.2: Define acceptable bypass policy for emergencies.

## Epic P2: Proof Operations
### Task P2.1: Regression maintenance
- [x] Subtask P2.1.1: Track proof breakages by commit.
- [x] Subtask P2.1.2: Require proof updates alongside invariant changes.

## Exit Criteria
- [x] Proof program is wired into CI and release gates.

## Proof-to-Test Traceability

| Proof artifact | Executable anchor | Required runtime/test evidence |
| --- | --- | --- |
| `proofs/verus/specs/runtime_quick.rs` | `symphony-domain::validate_invariants` and `symphony-domain::invariant_catalog` | `symphony-domain` invariant/unit tests, property-based event-stream tests, `symphony-testkit::validate_trace`, reducer-contract suite, interleavings/soak trace validation |
| `proofs/verus/specs/runtime_full.rs` | `symphony-domain::reduce` lifecycle commands | Domain lifecycle tests, conformance lifecycle cases, retry integration tests, trace command/state coherence checks, reducer-contract suite |
| `proofs/verus/specs/agent_update_safety.rs` | `crates/symphony-domain/src/agent_update.rs` | Domain `UpdateAgent` tests, property-based token monotonicity checks, observability/orchestrator conformance cases |
| `proofs/verus/specs/session_liveness.rs` | `symphony-runtime` dispatch/retry/release flow | Runtime conformance cases, retry exhaustion integration tests, interleavings around retry/release/dispatch races |
| `proofs/verus/specs/workspace_safety.rs` | `symphony-workspace` path validation | Workspace conformance cases plus workspace lifecycle tests |

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Task |
| --- | --- | --- | --- |
| Sec. 7.4 idempotency/recovery invariants | Reducer invariants, command-taxonomy transitions, and `UpdateAgent` accounting safety are formalized in Verus and mapped to executable tests | Keep proof-to-runtime-test traceability current as reducer behavior evolves | `P1.1`, `P2.1` |
| Sec. 8.3-8.4 concurrency and retry guarantees | One-step dispatch/retry/release fairness assumptions and slot-accounting proofs are formalized | Extend to stronger starvation-freedom and eventual-processing guarantees across repeated scheduler steps | `P1.1`, `P2.1` |
| Sec. 9.5 and Sec. 15.2 safety invariants | Workspace model proof now covers separator removal, placeholder sanitization, and sanitize fixed points for allowed keys | Add canonicalization and race-sensitive proof obligations beyond current string-level model | `P2.1` |
| Sec. 17 and Sec. 18 formal validation gate | Quick/full proof profiles plus real interleavings/soak long-suite runners were rerun on the current tree | Make proof summaries machine-readable and keep proof-touch enforcement active in release policy | `P1.2`, `P2.1` |
<!-- SPEC_GAP_MAP_END -->
