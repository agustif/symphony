# Elixir vs Rust Symphony: A Fairer Local Benchmark Pass

2026-03-08

The earlier benchmark story in this repo was not good enough.

It mixed theory with measurement, gave Rust real microbench coverage without Elixir parity, and leaned on load tests that were not strict enough about offered load. This pass fixes the harness first, then reports only what the harness can honestly support.

## What Changed In The Harness

- Startup is now measured with `hyperfine`.
- HTTP tests use `k6` arrival-rate executors instead of closed-loop VU plus `sleep()` scripts.
- Both implementations run against the same fake local Linear GraphQL server.
- The fake tracker serves a shared synthetic corpus of 200 issues.
- Both implementations read the same workflow file: [`benchmarks/workflows/common-workflow.md`](../benchmarks/workflows/common-workflow.md).
- The benchmark driver records structured JSON artifacts under `/tmp/symphony-benchmark-results-20260308-024603`.
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
| Rust | `80.1 ms` | `3.0 ms` | `77.3 ms` | `84.9 ms` |
| Elixir | `554.9 ms` | `6.0 ms` | `547.9 ms` | `563.0 ms` |

On this host, Rust started `6.93x` faster.

### Shared HTTP Path

All HTTP scenarios used the same route and held the offered load constant.

| Scenario | Offered load | Rust avg / p95 / p99 | Elixir avg / p95 / p99 | Failures |
| --- | --- | --- | --- | --- |
| Load | `80 rps` for `20s` | `0.360 / 0.686 / 1.035 ms` | `1.349 / 2.633 / 8.882 ms` | `0` for both |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.324 / 0.767 / 1.185 ms` | `1.196 / 2.793 / 4.464 ms` | `0` for both |
| Soak | `40 rps` for `45s` | `0.380 / 0.693 / 1.482 ms` | `1.138 / 1.727 / 2.185 ms` | `0` for both |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.295 / 0.547 / 0.962 ms` | `1.026 / 1.850 / 2.507 ms` | `0` for both |

The steady-state story is still straightforward: Rust was consistently lower-latency on the current state API implementation.

The interesting change from the previous run is Elixir soak behavior. The earlier long-tail blow-up disappeared after the headless-path cleanup, which strongly suggests the terminal dashboard path was contaminating non-interactive runs.

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
- The long-run `data_received` totals still diverged substantially, which suggests the returned state evolves differently over time as well.
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
| `reducer_claim/10` | `1.245 us` |
| `reducer_claim/100` | `12.232 us` |
| `reducer_claim/1000` | `133.87 us` |
| `reducer_full_lifecycle` | `602.63 ns` |
| `state_clone_100_issues` | `32.276 ns` |
| `state_serialize_100_issues` | `2.936 us` |
| `state_deserialize_100_issues` | `10.186 us` |
| `sanitize_workspace_key` | `209.22 ns` |

## The Real Takeaway

Three things are true at once.

Rust currently wins on startup time and on the shared state endpoint latency numbers from this run.

Elixir still remains the more mature reference runtime in this repo.

The benchmark harness is finally good enough to publish the measurements above, but not yet good enough to collapse the whole story into a single headline like "Rust wins" or "Elixir scales better."

## V2

The next version of this benchmark stack should add fuzzy chaos monkey testing rather than pretending the deterministic baseline is enough.

That means:

- randomized tracker response timing and pagination boundaries
- injected `429`, `500`, and malformed GraphQL payloads
- process kill and restart during polling and dispatch
- workspace cleanup races and filesystem permission faults
- deterministic seeds and artifact capture so failures are reproducible

That should become a separate resilience report, not a footnote under latency charts.
