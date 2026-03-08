# Elixir vs Rust Symphony: A Fairer Local Benchmark Pass

2026-03-08

The earlier benchmark story in this repo was not good enough, and this updated pass replaces the previous 2026-03-08 numbers.

It mixed theory with measurement, gave Rust real microbench coverage without Elixir parity, and leaned on load tests that were not strict enough about offered load. This pass fixes the harness first, then reports only what the harness can honestly support.

## What Changed In The Harness

- Startup is now measured with `hyperfine`.
- HTTP tests use `k6` arrival-rate executors instead of closed-loop VU plus `sleep()` scripts.
- Both implementations run against the same fake local Linear GraphQL server.
- The fake tracker serves a shared synthetic corpus of 200 issues.
- Both implementations read the same workflow file: [`benchmarks/workflows/common-workflow.md`](../benchmarks/workflows/common-workflow.md).
- The benchmark driver records structured JSON artifacts under `/tmp/symphony-benchmark-results-20260308-061606`.
- The benchmark driver now also has a shared golden benchmark projection fixture at [`../fixtures/http_state/benchmark_state_small.json`](../fixtures/http_state/benchmark_state_small.json).

That gives us a much better answer to one narrow question:

How do the current Rust and Elixir Symphony implementations behave on the same machine, against the same synthetic tracker, under the same offered HTTP load?

## Setup

| Item | Value |
| --- | --- |
| Host | Apple Silicon MacBook Pro, macOS 25.4.0 |
| Architecture | `arm64` |
| Rust | `cargo 1.95.0-nightly` |
| Elixir | `mise`-managed local toolchain |
| Tracker | local fake Linear GraphQL server |
| Tracker corpus | `200` synthetic issues |
| Shared route | `/api/v1/state` |
| Startup tool | `hyperfine` |
| HTTP tool | `k6` |
| Rust microbench tool | Criterion |

## Results

### Startup

`hyperfine` ran each implementation five times after one warmup run.

| Implementation | Mean | Stddev | Min | Max |
| --- | --- | --- | --- | --- |
| Rust | `118.9 ms` | `4.0 ms` | `112.6 ms` | `122.4 ms` |
| Elixir | `816.1 ms` | `16.5 ms` | `798.2 ms` | `840.3 ms` |

On this host, Rust started `6.86x` faster.

### Shared HTTP Path

All HTTP scenarios used the same route and held the offered load constant.

| Scenario | Offered load | Rust avg / p95 / p99 | Elixir avg / p95 / p99 | Failures |
| --- | --- | --- | --- | --- |
| Load | `80 rps` for `20s` | `0.303 / 0.342 / 0.426 ms` | `0.642 / 0.754 / 3.138 ms` | `0` for both |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.264 / 0.331 / 0.527 ms` | `1.116 / 1.397 / 5.741 ms` | `0` for both |
| Soak | `40 rps` for `45s` | `0.292 / 0.346 / 0.403 ms` | `1.153 / 1.321 / 2.789 ms` | `0` for both |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.265 / 0.351 / 0.429 ms` | `1.056 / 1.297 / 2.309 ms` | `0` for both |

The steady-state story is still straightforward: Rust was consistently lower-latency on the current state API implementation.

The interesting change from the previous run is that the control-plane story is now stable across all four HTTP profiles. There is no longer a single soak outlier carrying the narrative.

## What I Trust

- Startup comparison: yes
- Shared route latency under fixed offered load: yes
- Zero-failure claim for these scenarios: yes
- Rust Criterion results as Rust-only regression tracking: yes

## What I Do Not Trust Yet

I do not trust these numbers as a pure language shootout.

Why not:

- `/api/v1/state` is not yet a schema-normalized contract across the two implementations.
- A manual payload spot-check after startup still showed different response shapes.
- The startup snapshots were different sizes: `1871` bytes for Rust and `1541` bytes for Elixir.
- Rust still returns extra top-level fields such as `activity`, `health`, `issue_totals`, `summary`, and `task_maps`.
- The suite still does not emulate actual Codex session traffic, streamed tool events, or transcript-heavy agent turns.
- There is still no matching Elixir Benchee suite for reducer and workspace primitives.
- The new shared golden only covers the common benchmark projection, not the full raw wire payload.

So the right claim is:

Rust is faster on the current implementation and endpoint behavior measured here.

The wrong claim is:

Rust is inherently faster than Elixir for the exact same Symphony workload in every meaningful sense.

## Rust Microbench Supplement

These are useful, but only as Rust-internal tracking until Elixir gets matching Benchee coverage.

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

## The Real Takeaway

Three things are true at once.

Rust currently wins on startup time and on the shared state endpoint latency numbers from this run.

Elixir still remains the more mature behavior reference runtime in this repo.

The benchmark harness is good enough to publish the measurements above, but not yet good enough to collapse the whole story into a single headline like "Rust wins everything" or "Elixir scales better."

## V2

The next version of this benchmark stack should add fuzzy chaos monkey testing rather than pretending the deterministic baseline is enough.

That means:

- randomized tracker response timing and pagination boundaries
- injected `429`, `500`, and malformed GraphQL payloads
- process kill and restart during polling and dispatch
- workspace cleanup races and filesystem permission faults
- deterministic seeds and artifact capture so failures are reproducible

That should become a separate resilience report, not a footnote under latency charts.
