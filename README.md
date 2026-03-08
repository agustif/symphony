# Symphony

This repository is an unofficial fork of OpenAI Symphony. It keeps the Elixir codebase as the
reference implementation in this fork and carries an in-progress Rust reimplementation focused on
spec alignment, observability parity, and stricter runtime engineering. It is not the upstream
OpenAI repository, and the Rust implementation does not yet provide 100% 1:1 parity with Elixir.

Symphony turns project work into isolated, autonomous implementation runs, allowing teams to manage
work instead of supervising coding agents.

[![Symphony demo video preview](.github/media/symphony-demo-poster.jpg)](.github/media/symphony-demo.mp4)

_In this [demo video](.github/media/symphony-demo.mp4), Symphony monitors a Linear board for work and spawns agents to handle the tasks. The agents complete the tasks and provide proof of work: CI status, PR review feedback, complexity analysis, and walkthrough videos. When accepted, the agents land the PR safely. Engineers do not need to supervise Codex; they can manage the work at a higher level._

> [!WARNING]
> Symphony is a low-key engineering preview for testing in trusted environments.

## Fork Status

- `rust/` is the recommended implementation to try first in this fork.
- `elixir/` is the reference implementation used to compare behavior and close parity gaps.
- `rust/` is still an unofficial reimplementation, not an upstream-supported port.
- The repository tracks the language-agnostic contract in [SPEC.md](SPEC.md), but the Rust runtime
  still diverges from Elixir in several operator-facing areas.

## Current Rust Divergences

- `GET /api/v1/state` is still not wire-identical. Elixir returns the narrower baseline contract:
  `generated_at`, `counts`, `running`, `retrying`, `codex_totals`, and `rate_limits`, plus an
  `error` envelope on timeout or unavailability. Rust publishes those same baseline fields but also
  includes additive operator data such as `activity`, `health`, `issue_totals`, `task_maps`, and
  `summary`. That means tooling written against the Elixir JSON shape will not see a byte-for-byte
  identical Rust payload today.
- Degraded snapshot behavior still differs at the source contract. In Elixir, a timed-out or
  unavailable snapshot becomes `{generated_at, error}`. In Rust, a stale cached snapshot can still
  be served with the previous state payload plus an attached `error` object, and only the
  no-snapshot case collapses fully to the smaller error envelope.
- Issue-detail status semantics still diverge in a way that comes from the snapshot source, not just
  the serializer. Elixir snapshots can carry the same issue in both `running` and `retrying`
  collections because `state.running` and `state.retry_attempts` are tracked independently, and the
  presenter currently resolves that by returning `status: "running"` while still surfacing both
  sections. Rust treats `retry_attempts > 0` as authoritative and emits `status: "retrying"` with
  only the retry block.
- `workspace.path` is now intended to reflect the real workspace path in both implementations, but
  the sanitization algorithm is still not the same. Elixir’s workspace root logic replaces
  characters outside `[A-Za-z0-9._-]` with `_`. Rust’s workspace crate uses a stricter reversible
  encoding strategy for non-safe bytes and underscores. So unusual identifiers can still end up in
  different on-disk workspace directories even when both implementations are reporting the real
  path.
- CLI contracts are still not 1:1. Elixir currently falls back to its simple usage surface for
  `--help` and `--version`, uses different exit codes, and keeps an explicit
  `--i-understand-that-this-will-be-running-without-the-usual-guardrails` acknowledgement flag.
  Rust still exposes a broader flag/config override surface and different help/version behavior.
- The web dashboard is behaviorally similar but not implementation-identical. Elixir uses Phoenix
  LiveView and PubSub with its own static assets and event loop. Rust uses server-rendered HTML plus
  live refresh from the same HTTP surface. The operator information is close, but the DOM shape,
  transport, and update model are different.
- Codex event humanization is closer than before, but it is still not exhaustively matched.
  Common dashboard events now line up more often, but long-tail protocol methods, account/session
  lifecycle events, and some failure wrappers can still produce different operator-facing text
  between Elixir and Rust.
- Config parsing still has some Rust-only drift. The documented canonical Elixir key is
  `observability.dashboard_enabled`, while Rust still accepts some extra aliases such as
  `observability.enabled`. That is current contract drift in the reimplementation, not a public
  behavior we want to preserve long term.

## Running Symphony

### Requirements

Symphony works best in codebases that have adopted
[harness engineering](https://openai.com/index/harness-engineering/). Symphony is the next step --
moving from managing coding agents to managing work that needs to get done.

### Option 1. Make your own

Tell your favorite coding agent to build Symphony in a programming language of your choice:

> Implement Symphony according to the following spec:
> https://github.com/openai/symphony/blob/main/SPEC.md

### Option 2. Use the Rust implementation in this fork

Check out [rust/README.md](rust/README.md) for instructions on how to set up your environment and
run the Rust implementation. Use [elixir/README.md](elixir/README.md) as the reference baseline
when you need to compare behavior or investigate a parity gap. You can also ask your favorite
coding agent to help with the setup:

> Set up Symphony for my repository based on
> https://github.com/openai/symphony/blob/main/rust/README.md

For a fair implementation comparison, see [docs/elixir-vs-rust.md](docs/elixir-vs-rust.md).
For the latest measured benchmark write-up, see [docs/elixir-vs-rust-benchmark-2026-03-08.md](docs/elixir-vs-rust-benchmark-2026-03-08.md).

---

## License

This project is licensed under the [Apache License 2.0](LICENSE).
