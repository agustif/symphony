# Elixir vs Rust Symphony: A Fairer Local Benchmark Pass

2026-03-07

The earlier benchmark story in this repo was not good enough.

It mixed theory with measurement, gave Rust real microbench coverage without Elixir parity, and leaned on load tests that were not strict enough about offered load. This pass fixes the harness first, then reports only what the harness can honestly support.

## What Changed In The Harness

- Startup is now measured with `hyperfine`.
- HTTP tests use `k6` arrival-rate executors instead of closed-loop VU plus `sleep()` scripts.
- Both implementations run against the same fake local Linear GraphQL server.
- The fake tracker serves a shared synthetic corpus of 200 issues.
- Both implementations read the same workflow file: [`benchmarks/workflows/common-workflow.md`](../benchmarks/workflows/common-workflow.md).
- The benchmark driver records structured JSON artifacts under `/tmp/symphony-benchmark-results-20260307-195048`.

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
| Rust | `114.9 ms` | `8.7 ms` | `109.4 ms` | `130.0 ms` |
| Elixir | `806.6 ms` | `6.2 ms` | `799.0 ms` | `813.9 ms` |

On this host, Rust started `7.02x` faster.

### Shared HTTP Path

All HTTP scenarios used the same route and held the offered load constant.

| Scenario | Offered load | Rust avg / p95 / p99 | Elixir avg / p95 / p99 | Failures |
| --- | --- | --- | --- | --- |
| Load | `80 rps` for `20s` | `0.286 / 0.350 / 0.565 ms` | `0.987 / 1.339 / 2.884 ms` | `0` for both |
| Stress | `40 -> 100 -> 180 -> 260 rps` over `55s` | `0.234 / 0.308 / 0.353 ms` | `1.528 / 2.599 / 7.285 ms` | `0` for both |
| Soak | `40 rps` for `45s` | `0.312 / 0.377 / 1.096 ms` | `26.123 / 109.817 / 221.424 ms` | `0` for both |
| Spike | `20 -> 220 -> 20 rps` over `30s` | `0.293 / 0.383 / 0.774 ms` | `1.235 / 1.666 / 2.738 ms` | `0` for both |

The steady-state story is straightforward: Rust was consistently lower-latency on the current state API implementation.

The soak story is more interesting. Elixir stayed correct and kept the failure rate at zero, but its tail latency widened sharply over time.

## What I Trust

- Startup comparison: yes
- Shared route latency under fixed offered load: yes
- Zero-failure claim for these scenarios: yes
- Rust Criterion results as Rust-only regression tracking: yes

## What I Do Not Trust Yet

I do not trust these numbers as a pure language shootout.

Why not:

- `/api/v1/state` is not yet a schema-normalized contract across the two implementations.
- A manual payload spot-check after startup showed different response shapes.
- The long-run `data_received` totals diverged substantially, which suggests the returned state evolves differently over time as well.
- There is still no matching Elixir Benchee suite for reducer and workspace primitives.

So the right claim is:

Rust is faster on the current implementation and endpoint behavior measured here.

The wrong claim is:

Rust is inherently faster than Elixir for the exact same Symphony workload in every meaningful sense.

## Rust Microbench Supplement

These are useful, but only as Rust-internal tracking until Elixir gets matching Benchee coverage.

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
