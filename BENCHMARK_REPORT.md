# Symphony Benchmark Report

Measured run date: 2026-03-08

This file tracks the latest local benchmark run committed in this repository.

## Scope

- Host: Apple Silicon MacBook Pro, macOS 25.4.0, arm64
- Rust toolchain: `cargo 1.95.0-nightly`
- Elixir toolchain: `mise`-managed Elixir/Erlang
- Shared tracker fixture: local fake Linear GraphQL server with 200 synthetic issues
- Shared workflow: [`benchmarks/workflows/common-workflow.md`](benchmarks/workflows/common-workflow.md)
- Startup benchmark: `hyperfine`
- HTTP benchmark: `k6` fixed arrival-rate scenarios against `/api/v1/state`
- Rust microbench: Criterion under [`benches/`](benches/)
- Workload scope: control-plane and observability path only, not a full fake Codex agent session harness

Raw artifacts for this run were written to `/tmp/symphony-benchmark-results-20260308-061606`.

## Headline Results

| Scenario | Offered load | Rust | Elixir | Notes |
| --- | --- | --- | --- | --- |
| Startup | 5 runs, 1 warmup | `118.9 ms ± 4.0 ms` | `816.1 ms ± 16.5 ms` | Rust was `6.86x` faster on this host |
| Load | `80 rps` for `20s` | `0.303 ms avg`, `0.342 ms p95`, `0.426 ms p99` | `0.642 ms avg`, `0.754 ms p95`, `3.138 ms p99` | Both held target, `0` failures |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.264 ms avg`, `0.331 ms p95`, `0.527 ms p99` | `1.116 ms avg`, `1.397 ms p95`, `5.741 ms p99` | Both held target, `0` failures |
| Soak | `40 rps` for `45s` | `0.292 ms avg`, `0.346 ms p95`, `0.403 ms p99` | `1.153 ms avg`, `1.321 ms p95`, `2.789 ms p99` | Both held target, `0` failures |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.265 ms avg`, `0.351 ms p95`, `0.429 ms p99` | `1.056 ms avg`, `1.297 ms p95`, `2.309 ms p99` | Both recovered cleanly, `0` failures |

## What These Numbers Mean

- On this shared local harness, Rust started faster and served the current `/api/v1/state` implementation with lower latency in every scenario.
- Both implementations sustained the configured offered load without HTTP failures.
- The control-plane benchmark picture is now consistent across the four HTTP profiles instead of hinging on one anomalous soak run.

## Fairness Caveats

- This is a shared-workload comparison, not a full end-to-end agent benchmark.
- Both runtimes used the same fake tracker, same workflow file, same arrival-rate profiles, and the same `/api/v1/state` route.
- The `/api/v1/state` payloads are not byte-identical across implementations even at startup.
- The captured startup snapshots were `1871` bytes for Rust and `1541` bytes for Elixir.
- Rust returned `activity`, `health`, `issue_totals`, `summary`, and `task_maps` in addition to the shared fields that Elixir returned.
- That means these results are fair for "current implementation behavior under the same harness", but not yet fair for "pure runtime cost of the exact same serialized payload".
- The suite still does not emulate actual Codex sessions, streamed tool traffic, approvals, or transcript-heavy turns.
- There is still no matching Elixir Benchee suite, so pure reducer/workspace microbench parity is not complete.

## Fairness Guardrail

- Benchmark fairness is now backed by a shared golden fixture at [`fixtures/http_state/benchmark_state_small.json`](fixtures/http_state/benchmark_state_small.json).
- Rust and Elixir both project `/api/v1/state` into the same normalized benchmark contract before asserting exact equality.
- The golden currently covers the shared comparison surface: `counts`, `running`, `retrying`, `codex_totals`, and `rate_limits`.
- Dynamic fields such as `generated_at`, `started_at`, `last_event_at`, and `due_at` are intentionally excluded from the benchmark projection.

## Rust Microbenchmarks

These numbers are useful for tracking Rust progress, but they are not language-comparison results until Elixir gets matching Benchee coverage.

| Benchmark | Result |
| --- | --- |
| `reducer_claim/10` | `1.5688 us` |
| `reducer_claim/100` | `15.996 us` |
| `reducer_claim/1000` | `171.93 us` |
| `reducer_full_lifecycle` | `845.21 ns` |
| `state_clone_100_issues` | `43.316 ns` |
| `state_serialize_100_issues` | `3.9428 us` |
| `state_deserialize_100_issues` | `14.960 us` |
| `sanitize_workspace_key` | `295.07 ns` |

## What Is Still Missing

- Matching Elixir Benchee microbenchmarks driven from the same fixture corpus
- Full raw `/api/v1/state` wire parity
- A fake Codex app-server workload harness for end-to-end agent benchmarking
- CPU and RSS attribution via `/usr/bin/time`, flamegraphs, or Instruments
- Dedicated benchmark-host runs instead of a developer laptop

## V2

V2 should add fuzzy chaos monkey coverage on top of the current deterministic baseline:

- randomized tracker latency, jitter, pagination, and partial GraphQL failures
- transient `429` and `5xx` responses from the tracker stub
- process kill and restart during poll and dispatch windows
- workspace filesystem faults and cleanup races
- deterministic seeds so failures are reproducible

That work should be reported separately from the baseline performance numbers in this file.
