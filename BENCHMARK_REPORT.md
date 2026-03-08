# Symphony Benchmark Report

Measured run date: 2026-03-08

This file replaces the earlier theoretical report with numbers from an actual local run.

## Scope

- Host: Apple Silicon MacBook Pro, macOS 25.4.0, arm64
- Rust toolchain: `cargo 1.95.0-nightly`
- Elixir toolchain: `mise`-managed Elixir/Erlang
- Shared tracker fixture: local fake Linear GraphQL server with 200 synthetic issues
- Shared workflow: [`benchmarks/workflows/common-workflow.md`](benchmarks/workflows/common-workflow.md)
- Startup benchmark: `hyperfine`
- HTTP benchmark: `k6` fixed arrival-rate scenarios against `/api/v1/state`
- Rust microbench: Criterion under [`benches/`](benches/)

Raw artifacts for this run were written to `/tmp/symphony-benchmark-results-20260308-024603`.

## Headline Results

| Scenario | Offered load | Rust | Elixir | Notes |
| --- | --- | --- | --- | --- |
| Startup | 5 runs, 1 warmup | `80.1 ms ± 3.0 ms` | `554.9 ms ± 6.0 ms` | Rust was `6.93x` faster on this host |
| Load | `80 rps` for `20s` | `0.360 ms avg`, `0.686 ms p95`, `1.035 ms p99` | `1.349 ms avg`, `2.633 ms p95`, `8.882 ms p99` | Both held target, `0` failures |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.324 ms avg`, `0.767 ms p95`, `1.185 ms p99` | `1.196 ms avg`, `2.793 ms p95`, `4.464 ms p99` | Both held target, `0` failures |
| Soak | `40 rps` for `45s` | `0.380 ms avg`, `0.693 ms p95`, `1.482 ms p99` | `1.138 ms avg`, `1.727 ms p95`, `2.185 ms p99` | Elixir tail improved sharply, `0` failures |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.295 ms avg`, `0.547 ms p95`, `0.962 ms p99` | `1.026 ms avg`, `1.850 ms p95`, `2.507 ms p99` | Both recovered cleanly, `0` failures |

## What These Numbers Mean

- On this shared local harness, Rust started faster and served the current `/api/v1/state` implementation with lower latency in every scenario.
- Both implementations sustained the configured offered load without HTTP failures.
- The biggest improvement from the latest tuning pass was Elixir soak behavior. The earlier long-tail blow-up disappeared once the non-interactive dashboard path stopped rendering during headless runs.

## Fairness Caveats

- This is a shared-workload comparison, not a schema-normalized benchmark.
- Both runtimes used the same fake tracker, same workflow file, same arrival-rate profiles, and the same `/api/v1/state` route.
- The `/api/v1/state` payloads are not byte-identical across implementations and appear to evolve differently over time.
- A manual spot-check after startup showed different response shapes even before long-running state accumulated.
- That means these results are fair for "current implementation behavior under the same harness", but not yet fair for "pure runtime cost of the exact same serialized payload".
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
| `reducer_claim/10` | `1.245 us` |
| `reducer_claim/100` | `12.232 us` |
| `reducer_claim/1000` | `133.87 us` |
| `reducer_full_lifecycle` | `602.63 ns` |
| `state_clone_100_issues` | `32.276 ns` |
| `state_serialize_100_issues` | `2.936 us` |
| `state_deserialize_100_issues` | `10.186 us` |
| `sanitize_workspace_key` | `209.22 ns` |

## What Is Still Missing

- Matching Elixir Benchee microbenchmarks driven from the same fixture corpus
- A schema-normalized API comparison path
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
