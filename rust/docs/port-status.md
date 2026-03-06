# Rust Port Status

Last reviewed: 2026-03-06
Owner: Rust implementation maintainers
Update cadence: update after every meaningful runtime, observability, CLI, validation, or proof milestone.

## Executive Summary

- The Rust port is beyond prototype quality and has a healthy local build.
- The repo-wide master plan still reads as mid-program at `64%` completion in [../TASKS.md](../TASKS.md).
- Core crates are strong, but the implementation is not yet ready to replace the Elixir runtime as the primary orchestrator.

## Verified Local Evidence

The following commands were run locally during this assessment:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | Passed |
| `cargo test -p symphony-http -p symphony-runtime -p symphony-cli` | Passed |
| `./proofs/verus/scripts/run-proof-checks.sh` | Passed quick profile with `15 verified, 0 errors` |

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
| CLI and host lifecycle | Mid-to-late | [../crates/symphony-cli/TASKS.md](../crates/symphony-cli/TASKS.md) |
| HTTP observability surface | Largely complete | [../crates/symphony-http/TASKS.md](../crates/symphony-http/TASKS.md) |
| Observability model | Mostly complete | [../crates/symphony-observability/TASKS.md](../crates/symphony-observability/TASKS.md) |
| Test program depth | Incomplete | [../tests/TASKS.md](../tests/TASKS.md) and [../tests/conformance/TASKS.md](../tests/conformance/TASKS.md) |
| Proof program | Strong but not closed | [../proofs/verus/TASKS.md](../proofs/verus/TASKS.md) |

## What Is Solid

- `WORKFLOW.md` loading, reload retention semantics, and typed config parsing are in good shape.
- The reducer/domain layer has deterministic transition coverage and property tests.
- The Linear adapter and protocol adapter both have meaningful local coverage and pass the current workspace gates.
- Runtime snapshots now carry session IDs, turn counts, cumulative tokens, runtime seconds, retry due-times, rate-limit payloads, poll timing, last-runtime-activity, and rolling throughput views from live state.
- The HTTP surface now serves runtime-derived state and matches Elixir for steady-state route shapes, nested issue-detail payloads, refresh error semantics, degraded state envelopes, and operator-visible activity/timing summary fields.
- The workspace builds cleanly and the current local validation loop is healthy.

## Cutover Blockers

The remaining blockers are about spec parity and operational behavior, not basic scaffolding:

1. Runtime lifecycle parity
   - Session reuse across `max_turns`, explicit child-stop behavior, and real-activity-driven reconciliation remain open in [../crates/symphony-runtime/TASKS.md](../crates/symphony-runtime/TASKS.md).
2. Observability correctness
   - Snapshot envelopes, activity timing, and throughput views are now in place, but secret-safe summary shaping and the last operator-facing validation paths remain open in [../crates/symphony-observability/TASKS.md](../crates/symphony-observability/TASKS.md).
3. Dashboard and operator depth
   - The JSON API is close and the dashboard now exposes activity/throughput summary cards, but it still lacks richer operator detail compared with Elixir in [../crates/symphony-http/TASKS.md](../crates/symphony-http/TASKS.md).
4. CLI and host supervision
   - Task-death supervision is substantially better, but startup/reload parity and controlled restart semantics for host-owned config remain open in [../crates/symphony-cli/TASKS.md](../crates/symphony-cli/TASKS.md).
5. Validation depth
   - Required conformance for Sections 17.6, 17.7, and 18.1 is still open in [../tests/conformance/TASKS.md](../tests/conformance/TASKS.md), and interleavings/soak remain intentionally shallow in [../tests/interleavings/TASKS.md](../tests/interleavings/TASKS.md) and [../tests/soak/TASKS.md](../tests/soak/TASKS.md).

## Code-Level Spec Mismatches Observed

### Dashboard parity is still shallow

- Elixir keeps the dashboard alive through degraded states and provides a richer operator surface.
- Rust now preserves degraded states and stale cached snapshots and shows poll/activity/throughput summary cards, but it still renders a compact HTML summary with limited operator detail in [../crates/symphony-http/src/state_handlers.rs](../crates/symphony-http/src/state_handlers.rs#L78).

### Runtime-side `max_turns` and explicit child-stop behavior are still not closed

- The runtime task map still tracks session reuse across `max_turns` and explicit subprocess stop on restart and shutdown as open in [../crates/symphony-runtime/TASKS.md](../crates/symphony-runtime/TASKS.md#L38).
- During this review, the observer-facing metadata path was verified, but not the full multi-turn session lifecycle.

### Message summaries are sanitized, but not yet Elixir-humanized

- Rust now trims and truncates Codex messages for the HTTP surface in [../crates/symphony-http/src/payloads.rs](../crates/symphony-http/src/payloads.rs#L529).
- Elixir still has a richer event-aware humanizer for session starts, approvals, tool calls, and reasoning deltas in [../../elixir/lib/symphony_elixir/status_dashboard.ex](../../elixir/lib/symphony_elixir/status_dashboard.ex#L1068).

### Host supervision and reload parity remain open

- The CLI now serves runtime-fed state, refresh control, and immediate background-task exit detection, but it still lacks host-owned reload/restart parity and retained-invalid reload behavior in [../crates/symphony-cli/TASKS.md](../crates/symphony-cli/TASKS.md).
- Those gaps still block a production cutover even though the HTTP path itself is cleaner.

## Validation Depth Observed

- `cargo test --workspace` is green locally, including conformance, interleavings, and soak suites.
- The interleaving suite currently contains two deterministic tests in [../tests/interleavings/mod.rs](../tests/interleavings/mod.rs).
- The soak suite currently contains two deterministic tests in [../tests/soak/mod.rs](../tests/soak/mod.rs).
- Those suites are useful smoke checks, but they do not yet satisfy their own task-map depth targets.

## Task-Map Drift Corrected in This Review

- The domain crate already had property tests in [../crates/symphony-domain/src/property_tests.rs](../crates/symphony-domain/src/property_tests.rs); its task map has been updated accordingly.
- The testkit crate already had deterministic clocks, timer queues, fakes, trace helpers, and snapshot utilities in [../crates/symphony-testkit/src/lib.rs](../crates/symphony-testkit/src/lib.rs); its task map and the parent test-program task map have been updated to reflect that implemented base layer.
- This update also closed stale notes about live-tracker HTTP coupling, placeholder observability fields, strict prompt rendering, `after_run` failure handling, and missing degraded HTTP envelopes.

## Recommended Finish Order

1. Close runtime lifecycle parity in `symphony-runtime`, especially explicit child-stop behavior and real-activity-driven reconciliation.
2. Finish CLI reload/restart parity for host-owned config and retain-invalid reload handling.
3. Deepen the dashboard beyond the current compact summary cards and align message humanization where it matters operationally.
4. Finish the remaining observability redaction and operator-surface validation paths.
5. Expand conformance coverage for Sections 17.6, 17.7, and 18.1, then deepen interleavings and soak beyond smoke level.
