# Verus Proof Plan

This directory tracks formal proof execution for Rust runtime invariants and CI integration points.

## Invariants

- No running issue without claim.
- Single-running-session invariant per issue.
- Retry attempt monotonicity.
- Release correctness across terminal and non-active transitions.

## CI execution model

| Workflow | Trigger | Purpose | Entrypoint |
| --- | --- | --- | --- |
| `rust-ci` | `pull_request`, `push` to `main`, `workflow_dispatch` | Fast correctness checks for workspace quality gates. | `cargo fmt`, `cargo clippy`, `cargo test` |
| `rust-proofs` | `workflow_dispatch`, weekly `schedule` | Skeleton pipeline for Verus proofs and long suites. | `proofs/verus/scripts/run-proof-checks.sh`, `proofs/verus/scripts/run-long-suite.sh` |

## Script contract

The `scripts/` directory exposes executable placeholders that are safe to run in CI today.

- Placeholders validate input shape.
- Placeholders print planned production commands.
- Placeholders fail only on invalid arguments.

See `scripts/README.md` for rollout steps and command details.

## Near-term rollout

1. Add initial Verus specs for session and issue-state invariants.
2. Wire `run-proof-checks.sh` to real `verus` invocations.
3. Add machine-readable proof summaries for CI artifact upload.
4. Expand long suites from placeholder mode to deterministic test entrypoints.
