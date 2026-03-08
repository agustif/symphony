# Elixir vs Rust Symphony Implementations

This document is intentionally comparative, not promotional.

Both implementations target the same language-agnostic contract in [`SPEC.md`](../SPEC.md). They are not at the same maturity level, and they should not be benchmarked casually.

## Current Positioning

| Implementation | Current role | Strengths | Current tradeoffs |
| --- | --- | --- | --- |
| `rust/` | Recommended implementation for new evaluation and new work in this fork | Reducer-first architecture, tighter crate boundaries, stronger invariant posture, lower-latency current state API, formal verification track | Still closing the last raw parity gaps with Elixir |
| `elixir/` | Reference behavior oracle | OTP supervision, production-shaped runtime behavior, Phoenix observability, broader operational maturity | More stateful/runtime-coupled internals, looser boundaries, not the implementation this fork is steering users toward |

## What Is Fair To Say Today

- In this fork, you should start with the Rust implementation unless you are validating parity against legacy Elixir behavior.
- Elixir remains the best reference for checking expected operator behavior when a parity question comes up.
- Rust is currently the cleaner architecture and the faster implementation on the shared local `/api/v1/state` benchmark harness.
- The current benchmark suite is still a control-plane benchmark, not a full end-to-end fake Codex workload benchmark.

## Comparison Areas

### Runtime model

- Elixir centers the runtime around OTP processes and supervision.
- Rust centers the runtime around a deterministic reducer core plus async adapters.

### Operational maturity

- Elixir is currently the safer choice when the goal is verifying historical behavior and operator expectations.
- Rust is currently the better choice when the goal is shipping against this fork's intended architecture.

### Observability

- Elixir exposes a Phoenix/LiveView-backed dashboard plus JSON API.
- Rust exposes a snapshot-driven dashboard/API surface and now has live terminal parity coverage.
- A fair comparison must separate shared API paths from implementation-specific UI rendering and from payload-shape drift.

### Safety posture

- Elixir relies on OTP supervision and runtime guardrails.
- Rust pushes more safety into typed boundaries, reducer invariants, and verification-oriented design.
- Both still require workload-level validation under real operational stress.

## Current Benchmark Snapshot

The current local benchmark run is documented in [`../BENCHMARK_REPORT.md`](../BENCHMARK_REPORT.md) and [`elixir-vs-rust-benchmark-2026-03-08.md`](./elixir-vs-rust-benchmark-2026-03-08.md).

Headline numbers from that run:

| Scenario | Rust | Elixir |
| --- | --- | --- |
| Startup | `118.9 ms ± 4.0 ms` | `816.1 ms ± 16.5 ms` |
| Load p95 on `/api/v1/state` | `0.342 ms` | `0.754 ms` |
| Stress p95 on `/api/v1/state` | `0.331 ms` | `1.397 ms` |
| Soak p95 on `/api/v1/state` | `0.346 ms` | `1.321 ms` |
| Spike p95 on `/api/v1/state` | `0.351 ms` | `1.297 ms` |

That benchmark is useful, but it is still narrower than a real agent workload:

- It measures startup plus the shared `/api/v1/state` control-plane path.
- It uses the same fake Linear server, same 200-issue corpus, same workflow file, and same arrival-rate profiles.
- It does not yet emulate fake Codex sessions, streamed tool events, approvals, or transcript-heavy turns.

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
| Agent-loop realism | Measure actual fake Codex session behavior | custom fake app-server harness | same | Same scripted session corpus across both runtimes |

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
9. `fake_codex_single_session`
10. `fake_codex_concurrent_sessions`

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

## Current Divergences

The remaining parity gaps are concrete, not abstract.

### `/api/v1/state` wire shape

- Elixir currently returns `generated_at`, `counts`, `running`, `retrying`, `codex_totals`, and `rate_limits`.
- Rust currently returns those fields plus `activity`, `health`, `issue_totals`, `summary`, and `task_maps`.
- On the current benchmark run, the captured startup snapshots were `1541` bytes for Elixir and `1871` bytes for Rust.
- That means the benchmark is measuring real implementation behavior, but not the cost of serializing the exact same payload.

### Degraded-state behavior

- Elixir returns a minimal `{generated_at, error}` style envelope when state capture fails or times out.
- Rust can serve a stale cached snapshot with an attached error instead.
- This is an operator-facing behavioral difference, not just a code-organization detail.

### Issue-detail status semantics

- The current Elixir snapshot source can surface both `running` and `retry` data for the same issue while still reporting `status: "running"`.
- Rust currently treats retry state as authoritative and collapses that shape to a retrying status.
- This should be fixed at the snapshot source contract, not by copying quirks back and forth at the presenter layer.

### Workspace path projection

- Elixir now projects the real workspace path through the workspace manager, including the same sanitization rules used for actual workspace creation.
- Rust still has a presenter-layer fallback that joins the workspace root with the issue identifier string directly.
- For ordinary issue identifiers this looks the same; for odd identifiers it does not.

### CLI contract

- `--help`, `--version`, and exit-code behavior are still not identical.
- Elixir still has the explicit no-guardrails acknowledgement flag.
- Rust still exposes a broader CLI override surface.
- This means the two binaries are not yet interchangeable at the exact command-contract level.

### Dashboard implementation

- Elixir web observability is Phoenix LiveView and PubSub driven.
- Rust web observability is snapshot-driven HTML with refresh behavior layered on top.
- Terminal parity is much closer now, but the web implementation is still behavior-equivalent rather than implementation-identical.

### Config contract

- Elixir's canonical config key is `observability.dashboard_enabled`.
- Rust still accepts the legacy alias `observability.enabled`.
- In a new fork, that alias is drift, not a migration feature, and it should be removed once the remaining config callers are updated.

### Event humanization coverage

- Rust now covers the dashboard-critical Codex event cases, but the humanization table has not yet been exhaustively proven case-for-case against Elixir.
- Until that audit is complete, message parity should be treated as strong but not fully closed.

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

If you are starting fresh in this fork, use Rust.

If you need to answer a parity question, compare against Elixir.

If you need to claim performance differences, build the shared harness first and publish the methodology with the numbers.
