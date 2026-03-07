# Symphony Benchmark Report

Measured run date: 2026-03-07

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

Raw artifacts for this run were written to `/tmp/symphony-benchmark-results-20260307-195048`.

## Headline Results

| Scenario | Offered load | Rust | Elixir | Notes |
| --- | --- | --- | --- | --- |
| Startup | 5 runs, 1 warmup | `114.9 ms ± 8.7 ms` | `806.6 ms ± 6.2 ms` | Rust was `7.02x` faster on this host |
| Load | `80 rps` for `20s` | `0.286 ms avg`, `0.350 ms p95`, `0.565 ms p99` | `0.987 ms avg`, `1.339 ms p95`, `2.884 ms p99` | Both held target, `0` failures |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.234 ms avg`, `0.308 ms p95`, `0.353 ms p99` | `1.528 ms avg`, `2.599 ms p95`, `7.285 ms p99` | Both held target, `0` failures |
| Soak | `40 rps` for `45s` | `0.312 ms avg`, `0.377 ms p95`, `1.096 ms p99` | `26.123 ms avg`, `109.817 ms p95`, `221.424 ms p99` | Largest divergence, `0` failures |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.293 ms avg`, `0.383 ms p95`, `0.774 ms p99` | `1.235 ms avg`, `1.666 ms p95`, `2.738 ms p99` | Both recovered cleanly, `0` failures |

## What These Numbers Mean

- On this shared local harness, Rust started faster and served the current `/api/v1/state` implementation with lower latency in every scenario.
- Both implementations sustained the configured offered load without HTTP failures.
- The soak profile deserves the most attention. Elixir stayed correct but showed much larger long-tail latency over time.

## Fairness Caveats

- This is a shared-workload comparison, not a schema-normalized benchmark.
- Both runtimes used the same fake tracker, same workflow file, same arrival-rate profiles, and the same `/api/v1/state` route.
- The `/api/v1/state` payloads are not byte-identical across implementations and appear to evolve differently over time.
- A manual spot-check after startup showed different response shapes even before long-running state accumulated.
- That means these results are fair for "current implementation behavior under the same harness", but not yet fair for "pure runtime cost of the exact same serialized payload".
- There is still no matching Elixir Benchee suite, so pure reducer/workspace microbench parity is not complete.

## Rust Microbenchmarks

These numbers are useful for tracking Rust progress, but they are not language-comparison results until Elixir gets matching Benchee coverage.

| Benchmark | Result |
| --- | --- |
| `reducer_claim/10` | `1.567 us` |
| `reducer_claim/100` | `16.022 us` |
| `reducer_claim/1000` | `169.37 us` |
| `reducer_full_lifecycle` | `874.33 ns` |
| `state_clone_100_issues` | `43.553 ns` |
| `state_serialize_100_issues` | `3.670 us` |
| `state_deserialize_100_issues` | `13.379 us` |
| `sanitize_workspace_key` | `280.09 ns` |

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
