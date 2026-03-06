# Rust Docs Index

Owner: Rust implementation maintainers
Update cadence: update after every meaningful runtime, observability, host lifecycle, validation, or proof milestone.

## What Lives Where

| Document | Purpose | Update When |
| --- | --- | --- |
| [architecture/overview.md](architecture/overview.md) | Source-of-truth architecture narrative for crate layout, data flow, and ownership boundaries. | Architecture or crate-boundary changes. |
| [adr/0001-reducer-first-runtime.md](adr/0001-reducer-first-runtime.md) through [adr/0005-observability-contract.md](adr/0005-observability-contract.md) | Architectural decisions, rationale, rejected alternatives, and enforcement expectations. | A decision changes or a new decision is formalized. |
| [port-status.md](port-status.md) | Current implementation assessment, cutover blockers, and code-level spec mismatches. | Any material status change or fresh assessment sweep. |
| [../TESTS_AND_PROOFS_SUMMARY.md](../TESTS_AND_PROOFS_SUMMARY.md) | Last verified local validation commands and results. | After running fmt, clippy, tests, or proofs for status reporting. |
| [../tests/README.md](../tests/README.md) | Test-suite layout and validation intent. | Test program structure changes. |
| [../proofs/verus/README.md](../proofs/verus/README.md) | Proof layout, local proof workflow, and toolchain notes. | Proof workflow or proof layout changes. |

## Topic Boundaries

- Put implementation shape, crate interactions, and invariants in `architecture/`.
- Put decision rationale, policy, and alternatives in `adr/`.
- Put current completion assessments, blockers, and code-review observations in `port-status.md`.
- Put concrete command results and last verification evidence in `../TESTS_AND_PROOFS_SUMMARY.md`.

## Maintenance Rules

- Update `port-status.md` when closing or opening a major runtime, observability, CLI, validation, or proof gap.
- Update `../TESTS_AND_PROOFS_SUMMARY.md` only from commands actually run.
- Keep status observations out of ADRs unless the status implies a durable architectural decision.
