# Rust Release Gates

Owner: Rust implementation maintainers
Last updated: 2026-03-07

This document defines the Rust rollout milestones and the suite subsets that
gate each milestone.

## Milestones

| Milestone | Goal | Required validation |
| --- | --- | --- |
| Alpha | Safe for regular developer iteration and internal dogfooding | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p symphony-domain`, `cargo test -p symphony-testkit --test reducer_contract_suite`, `cargo test -p symphony-testkit --test conformance_suite`, `./proofs/verus/scripts/run-proof-checks.sh --profile quick` |
| Beta | Safe for shadow mode and candidate production rehearsals | All Alpha gates, `cargo test --workspace`, `cargo test -p symphony-testkit --test interleavings_suite`, `cargo test -p symphony-testkit --test soak_suite`, current [runbook.md](runbook.md), current [cutover-checklist.md](cutover-checklist.md) |
| GA | Safe to become the primary orchestrator | All Beta gates, `./proofs/verus/scripts/run-proof-checks.sh --profile full`, no open release-blocking parity issues, a fresh live Rust-host smoke run against the target workflow, and a completed shadow-to-primary cutover review |

## CI Mapping

These milestones map directly to CI jobs and checked-in scripts.

| Validation | Source |
| --- | --- |
| Format and lint gates | `.github/workflows/rust-ci.yml` |
| Conformance, reducer-contract, interleavings, and soak suite gates | `.github/workflows/rust-ci.yml` |
| Quick and full Verus proof profiles | `.github/workflows/rust-proofs.yml` plus `proofs/verus/scripts/run-proof-checks.sh` |
| Proof-touch policy for invariant changes | `.github/workflows/rust-proofs.yml` plus `proofs/verus/scripts/require-proof-updates.sh` |

## Emergency Bypass Policy

Emergency bypass is allowed only when waiting for the full Rust gate set would
materially worsen an active production incident or a high-severity security
remediation.

Every bypass must include all of the following:

1. A linked incident or security issue explaining why the normal gates are too slow.
2. A named approver who is accountable for the bypass.
3. A written rollback plan that can be executed with the current [cutover-checklist.md](cutover-checklist.md).
4. A time-boxed follow-up window of no more than 24 hours to rerun the skipped gates and publish the results.
5. A post-incident note in [../TESTS_AND_PROOFS_SUMMARY.md](../TESTS_AND_PROOFS_SUMMARY.md) or the corresponding issue recording which gates were skipped.

Emergency bypass does not waive proof-touch requirements for follow-up work.
If a reducer invariant or trace contract changed, the follow-up change must
either update the Verus artifacts or explicitly update the proof task maps to
explain why the current proofs still cover the change.

## Maintenance Rules

- Update this file whenever a required suite name, proof profile, or cutover bar changes.
- Keep this file aligned with [../TASKS.md](../TASKS.md), [../tests/TASKS.md](../tests/TASKS.md), and [../proofs/TASKS.md](../proofs/TASKS.md).
- Do not mark a milestone complete in task maps unless the corresponding command set is executable from the current tree.
