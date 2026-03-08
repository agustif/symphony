# Elixir vs Rust Symphony Implementations

This document is intentionally comparative, not promotional.

Both implementations target the same language-agnostic contract in [`SPEC.md`](../SPEC.md). They are not at the same maturity level, and they should not be benchmarked casually.

## Current Positioning

| Implementation | Current role | Strengths | Current tradeoffs |
| --- | --- | --- | --- |
| `elixir/` | Stable reference runtime | OTP supervision, production-shaped runtime behavior, Phoenix observability, broader operational maturity | More stateful/runtime-coupled internals, less reducer-oriented isolation |
| `rust/` | Clean redesign in progress | Reducer-first architecture, tighter crate boundaries, stronger invariant posture, formal verification track | Still converging operationally; not yet the long-standing production reference |

## What Is Fair To Say Today

- The Elixir implementation is the current reference behavior for operating Symphony end to end.
- The Rust implementation is the cleaner architectural redesign and has a stronger correctness story at the reducer and boundary layers.
- The Rust implementation should not be described as categorically "faster" than Elixir without a shared benchmark harness.
- The Elixir implementation should not be described as categorically "more scalable" than Rust without controlled load, soak, and failure-recovery data.

## Comparison Areas

### Runtime model

- Elixir centers the runtime around OTP processes and supervision.
- Rust centers the runtime around a deterministic reducer core plus async adapters.

### Operational maturity

- Elixir is currently the safer choice when the goal is reference behavior and operator familiarity.
- Rust is currently the better choice when the goal is modularity, invariants, and long-term architecture cleanup.

### Observability

- Elixir exposes a Phoenix/LiveView-backed dashboard plus JSON API.
- Rust exposes an HTTP dashboard/API surface driven from runtime snapshots.
- A fair comparison must separate shared API paths from implementation-specific UI rendering.

### Safety posture

- Elixir relies on OTP supervision and runtime guardrails.
- Rust pushes more safety into typed boundaries, reducer invariants, and verification-oriented design.
- Both still require workload-level validation under real operational stress.

## Current Benchmarking Gap In This Repo

Today this repository does not yet provide a defensible Elixir-vs-Rust benchmark harness.

- Rust has Criterion microbenchmarks under [`benches/`](../benches/).
- The repo does not yet provide matching Elixir microbenchmarks.
- The current HTTP load tests under [`load-tests/`](../load-tests/) are useful for generic service probing, but not for fair cross-implementation claims.
- Benchmark results are not comparable if the harness mixes different endpoints, different fixture data, different warmup behavior, or different offered load models.

## Fair Benchmark Methodology

### Rules

1. Benchmark the same workload, not just the same repo.
2. Keep arrival rate constant for service comparisons.
3. Separate cold-start, warm-start, microbench, and saturation tests.
4. Use the same fixture corpus, same workflow file, same tracker fixture, and same deployment topology.
5. Disable live external dependencies during benchmarks.

### What to benchmark

| Layer | Goal | Rust tool | Elixir tool | Notes |
| --- | --- | --- | --- | --- |
| Reducer/domain microbench | Measure pure state transition cost | Criterion | Benchee | Drive both from the same event corpus |
| Workspace/path microbench | Measure workspace key/path and lifecycle primitives | Criterion | Benchee | Use identical identifier and filesystem fixtures |
| CLI end-to-end | Measure startup + one deterministic orchestration tick | Hyperfine | Hyperfine | Same command contract, separate cold and warm runs |
| HTTP latency/throughput | Measure shared API behavior | k6 `constant-arrival-rate`, `wrk2`, or `vegeta` | same | Hold offered load constant |
| Soak/stability | Measure leaks, retry buildup, and degradation | k6/vegeta + system metrics | same | Run long enough to expose scheduler/runtime drift |
| Profiling | Attribute CPU and memory cost | `perf`, flamegraph, `/usr/bin/time -v` | eflame/eep + `/usr/bin/time -v` | Use after, not instead of, workload benchmarking |

### Recommended tools

- `hyperfine` for CLI wall-clock benchmarking.
- Criterion for Rust microbenchmarks.
- Benchee for Elixir microbenchmarks.
- k6 with `constant-arrival-rate` or `ramping-arrival-rate` for HTTP offered-load tests.
- `wrk2` or `vegeta` when fixed-rate HTTP benchmarking is the priority.
- `/usr/bin/time -v` for RSS, CPU time, and page-fault data.
- `perf` plus flamegraphs on Linux for hotspot attribution.

## Benchmark Design Requirements

### Shared corpus

Check in a language-neutral fixture set:

- `fixtures/issues/*.json`
- `fixtures/events/*.json`
- `fixtures/runtime_snapshot.json`
- `fixtures/http_state/*.json`
- `fixtures/workflow/WORKFLOW.md`

Both implementations should deserialize or adapt the same corpus.

Until full `/api/v1/state` wire parity exists, benchmark fairness should be enforced on a normalized benchmark projection. The shared golden should cover only the fields both implementations promise to compare in performance reports.

### Shared scenarios

Use the same named scenarios for both implementations:

1. `reducer_claim_release_small`
2. `reducer_retry_heavy`
3. `workspace_prepare_existing`
4. `workspace_cleanup_large_tree`
5. `http_state_snapshot_small`
6. `http_state_snapshot_large`
7. `single_tick_no_work`
8. `single_tick_n_candidates`

### Control variables

Keep these fixed:

- CPU governor and host class
- core count exposed to the process
- memory limit
- OS image
- container image base
- tracker/agent fixture behavior
- dataset size
- request arrival rate

## What Not To Do

- Do not compare `GET /` between implementations and call it an orchestration benchmark.
- Do not use closed-loop VU tests with `sleep()` when claiming throughput superiority.
- Do not benchmark against live Linear or live Codex.
- Do not compare debug builds to release builds.
- Do not mix cold-start and warm-state numbers into one headline result.
- Do not publish single-run numbers without confidence intervals or repeated samples.

## Suggested Command Shapes

### CLI benchmarking with Hyperfine

```bash
hyperfine \
  --warmup 3 \
  --runs 20 \
  --export-json benchmark-results/cli.json \
  'cd rust && cargo run --release --package symphony-cli -- ../fixtures/workflow/WORKFLOW.md --port 0' \
  'cd elixir && MIX_ENV=prod ./bin/symphony ../fixtures/workflow/WORKFLOW.md --port 0'
```

Use this only after both commands are made to run against the same deterministic fixture harness. Record cold-start and warm-start separately.

### HTTP benchmarking with fixed offered load

Prefer:

- k6 `constant-arrival-rate`
- k6 `ramping-arrival-rate`
- `wrk2`
- `vegeta`

Target one shared endpoint per scenario, usually `/api/v1/state`.

## Interpretation Guidance

When results arrive, compare:

- p50, p95, and p99 latency
- throughput at fixed arrival rate
- max RSS
- user and system CPU time
- error rate
- startup time
- steady-state variance over time

Do not collapse these into a single score.

## Practical Conclusion

If you need the current reference runtime, use Elixir.

If you need the cleaner redesign and stronger long-term architecture, use Rust.

If you need to claim performance differences, build the shared harness first and publish the methodology with the numbers.
