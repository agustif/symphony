# Rust Port Status

Last reviewed: 2026-03-07
Owner: Rust implementation maintainers
Update cadence: update after every meaningful runtime, observability, CLI, validation, or proof milestone.

## Executive Summary

- The Rust port has a healthy local build and a materially stronger validation story than it had at the start of this milestone.
- Fresh evidence on March 7, 2026 shows `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `./proofs/verus/scripts/run-proof-checks.sh --profile full` all passing on the current tree.
- The domain and testkit layers now share a first-class invariant catalog plus an executable reducer-contract trace harness. That closes a real traceability gap between reducer code, generated traces, and Verus proofs.
- The implementation is close to production readiness, but it is not yet ready to replace the Elixir runtime as the primary orchestrator. The remaining work is in rollout policy, host cutover criteria, and ongoing proof-to-release enforcement.

## Verified Local Evidence

| Command | Result |
| --- | --- |
| `cargo fmt --all` | Passed |
| `cargo test -p symphony-domain` | Passed with `27` reducer/unit/property/doctest checks |
| `cargo test -p symphony-testkit` | Passed with green conformance, integration, interleaving, soak, reducer-contract, and doctest targets |
| `cargo test --workspace` | Passed with all crate, conformance, integration, interleaving, soak, and doctest targets green |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `./proofs/verus/scripts/run-proof-checks.sh --profile full` | Passed with all phases green (`26`, `41`, `8`, `36`, and `12` verified obligations) |

Validation evidence is recorded separately in [../TESTS_AND_PROOFS_SUMMARY.md](../TESTS_AND_PROOFS_SUMMARY.md).

## Current Completion Map

| Area | Status | Evidence |
| --- | --- | --- |
| Workflow loading | Complete | [../crates/symphony-workflow/TASKS.md](../crates/symphony-workflow/TASKS.md) |
| Typed config | Near complete | [../crates/symphony-config/TASKS.md](../crates/symphony-config/TASKS.md) |
| Agent protocol | Near complete | [../crates/symphony-agent-protocol/TASKS.md](../crates/symphony-agent-protocol/TASKS.md) |
| Linear tracker adapter | Mostly complete | [../crates/symphony-tracker-linear/TASKS.md](../crates/symphony-tracker-linear/TASKS.md) |
| Workspace lifecycle | Mostly complete | [../crates/symphony-workspace/TASKS.md](../crates/symphony-workspace/TASKS.md) |
| Runtime core | Mostly complete | [../crates/symphony-runtime/TASKS.md](../crates/symphony-runtime/TASKS.md) |
| CLI and host lifecycle | Mostly complete | [../crates/symphony-cli/TASKS.md](../crates/symphony-cli/TASKS.md) |
| HTTP observability surface | Largely complete | [../crates/symphony-http/TASKS.md](../crates/symphony-http/TASKS.md) |
| Observability model | Mostly complete | [../crates/symphony-observability/TASKS.md](../crates/symphony-observability/TASKS.md) |
| Test program depth | Strong and getting tighter | [../tests/TASKS.md](../tests/TASKS.md) |
| Proof program | Strong but not release-gated enough | [../proofs/verus/TASKS.md](../proofs/verus/TASKS.md) |

## What Is Solid

- `WORKFLOW.md` loading, reload retention semantics, and typed config parsing are in good shape.
- The reducer/domain layer now publishes a stable invariant catalog with explicit proof and SPEC metadata.
- `symphony-testkit::validate_trace` now validates initial-state correctness, command/state coherence, and event-by-event reducer contracts instead of only checking post-step invariants.
- The new `reducer_contract_suite` exercises the public domain API, seeded invalid-state fixtures, deterministic event streams, and interleaved lifecycles through the shared trace harness.
- The Verus program now covers reducer invariants, agent-update accounting, session liveness, and workspace safety in the full proof profile, and that profile reran green on the current tree.

## Remaining Blockers

1. Release-gate policy
   - Proof evidence is strong locally, but branch-protection policy and explicit emergency bypass rules are still open in [../proofs/TASKS.md](../proofs/TASKS.md) and [../proofs/verus/TASKS.md](../proofs/verus/TASKS.md).
2. Cutover criteria
   - The repo still needs explicit alpha/beta/GA definitions and production-like rollout criteria in [../TASKS.md](../TASKS.md).
3. Host-operational confidence
   - The current slice did not rerun a live Rust-host smoke against the real workflow, so cutover should still require recurring host-level validation in addition to reducer/proof evidence.

## Validation Depth Observed

- `cargo test --workspace -- --list` reports `734` executable tests on the current tree.
- The shared `symphony-testkit` suite currently includes `137` conformance tests, `29` integration tests, `18` interleaving tests, `15` soak tests, and a new `5`-test reducer-contract suite.
- The reducer crate now includes public rustdoc examples for `reduce` and `validate_invariants`, and the trace harness has its own executable documentation example.
- The full Verus profile passed on the current tree rather than relying on retained evidence from an earlier slice.

## Recommended Finish Order

1. Enforce proof/test subsets as explicit release gates in CI and branch protection.
2. Keep the invariant catalog, reducer-contract suite, and Verus traceability tables in sync whenever reducer behavior changes.
3. Re-run real host smoke and cutover rehearsal flows on a recurring cadence, not just crate-level and proof-level validation.
4. Close the remaining rollout docs in [../TASKS.md](../TASKS.md) and the runbook/cutover material in the docs program.
