# Rust Test Program

This directory defines the cross-crate validation suites wired into CI.

## Suite Thresholds

| Suite | Command | Threshold |
| --- | --- | --- |
| Conformance | `cargo test --package symphony-testkit --test conformance_suite --all-features --locked` | 100% pass, 0 ignored failures |
| Interleavings | `cargo test --package symphony-testkit --test interleavings_suite --all-features --locked` | 100% pass, deterministic outputs |
| Soak (CI-bounded) | `cargo test --package symphony-testkit --test soak_suite --all-features --locked` | 100% pass, bounded state assertions |

## CI Enforcement

- Workflow: `.github/workflows/rust-ci.yml`
- Jobs: `suite-conformance`, `suite-interleavings`, `suite-soak`

