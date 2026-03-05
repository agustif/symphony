# Verus Proof Program

This directory hosts formal proof artifacts for runtime invariants and safety properties.

## Invariant classes

- Running implies claimed.
- Single running session entry per issue.
- Retry attempt monotonicity.
- No simultaneous running and retry state.
- Workspace path/key safety boundaries.

## Execution model

| Workflow | Trigger | Purpose | Entrypoint |
| --- | --- | --- | --- |
| `rust-ci` | `pull_request`, `push` to `main`, `workflow_dispatch` | Fast workspace quality gates. | `cargo fmt`, `cargo clippy`, `cargo test` |
| `rust-proofs` | `workflow_dispatch`, weekly `schedule` | Proof checks and long-suite orchestration. | `proofs/verus/scripts/run-proof-checks.sh`, `proofs/verus/scripts/run-long-suite.sh` |

## Script contract

- `scripts/run-proof-checks.sh` executes real Verus checks when `verus` is available.
- `scripts/run-proof-checks.sh` falls back to specification formatting validation when `verus` is absent.
- `scripts/run-long-suite.sh` remains placeholder-only until long suites are fully wired.

See `scripts/README.md` for detailed command behavior.

## Traceability map

| Proof module | Rust implementation anchor | SPEC anchor |
| --- | --- | --- |
| `specs/runtime_quick.rs` | `crates/symphony-domain/src/lib.rs` (`reduce`, `validate_invariants`) | State model and core invariant sections (`claimed`, `running`, `retry_attempts`) |
| `specs/runtime_full.rs` | `crates/symphony-domain/src/lib.rs` (`Claim`, `MarkRunning`, `QueueRetry`, `Release` chains) | Lifecycle transition semantics and retry handling |
| `specs/session_liveness.rs` | `crates/symphony-runtime/src/lib.rs` orchestration tick behavior | Dispatch/retry/release progress requirements |
| `specs/workspace_safety.rs` | `crates/symphony-workspace/src/lifecycle.rs` (`sanitize_workspace_key`, root containment checks) | Workspace safety and path containment requirements |

## Placement rationale

Verus proofs stay in `rust/proofs/verus/` by default.
This keeps production crates independent of proof-only dependencies and toolchains.
Traceability is preserved through the mapping table above and mirrored task links.

## Verus printable reference

- Local snapshot: `reference/verus-guide-print.md`
- Snapshot metadata and usage: `reference/README.md`
- Regeneration script: `scripts/sync-verus-guide-print.sh`

Regenerate from upstream clone:

```bash
rust/proofs/verus/scripts/sync-verus-guide-print.sh /tmp/verus-upstream
```

The proof runner and proof updates should follow options and semantics documented in that snapshot, especially CLI flags and proof attribute guidance.

## Next rollout gates

1. Replace remaining proof stubs with executable Verus model definitions.
2. Export machine-readable proof summaries as CI artifacts.
3. Replace placeholder long-suite script paths with concrete suite runners.
