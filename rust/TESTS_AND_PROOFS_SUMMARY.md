# Rust Tests and Proofs Summary

Last verified: 2026-03-08
Scope: local workspace validation in `/Users/af/symphony/rust`

## Commands Run

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --all` | Passed | Formatting is clean on the current tree. |
| `cargo test -p symphony-domain` | Passed | Reducer crate passed `27` unit/property/doctest checks, including the public invariant catalog and new rustdoc examples. |
| `cargo test -p symphony-testkit` | Passed | Shared harness crate passed unit, conformance, integration, interleaving, soak, reducer-contract, and doctest targets. |
| `cargo test --workspace` | Passed | Full workspace is green, including all crate/unit/integration/conformance/interleaving/soak/doctest targets. |
| `cargo test --workspace -- --list` | Passed | Current workspace enumerates `736` executable tests. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | Workspace linting is warning-free on the current tree. |
| `./proofs/verus/scripts/run-proof-checks.sh --profile full` | Passed | Full Verus profile passed all five phases on the current tree. |

## Test Coverage Summary

| Suite | Count | Status |
| --- | --- | --- |
| Conformance | 137 | All passing |
| Integration | 29 | All passing |
| Interleavings | 18 | All passing |
| Soak | 15 | All passing |
| Reducer contract | 5 | All passing |
| **Total workspace** | **734** | All passing |

## Verification Summary

| Component | Status |
| --- | --- |
| Public invariant catalog | Executable and documented |
| Reducer transition semantics | Unit and property tested |
| Trace command/state contracts | Executable and integration tested |
| Agent-update accounting safety | Proven in Verus and covered by reducer tests |
| Session liveness | Proven in Verus |
| Workspace safety | Proven in Verus |
| Full proof profile | Passed (`26 + 41 + 8 + 36 + 12` verified obligations) |

## Important Caveats

- The remaining gap is no longer raw test count. The current work is about release-gate policy, operational cutover criteria, and keeping proof-to-runtime traceability current.
- The new `reducer_contract_suite` hardens the seam between `symphony-domain`, `symphony-testkit`, and the Verus model, but it does not replace production-like host smoke runs.
- Coverage reports remain support metrics. The correctness bar is reducer determinism, explicit invariant enforcement, trace-contract validation, and proof alignment.

## Primary Follow-Up Docs

- Current implementation assessment: [docs/port-status.md](docs/port-status.md)
- Master plan and gap map: [TASKS.md](TASKS.md)
- Test program status: [tests/TASKS.md](tests/TASKS.md)
- Proof program status: [proofs/TASKS.md](proofs/TASKS.md)
