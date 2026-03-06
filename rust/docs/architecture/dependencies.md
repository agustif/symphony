# Rust Crate Dependency Map

Owner: Rust implementation maintainers
Last updated: 2026-03-06

This document records the intended workspace layering and the current crate
dependency graph derived from `Cargo.toml` files.

## Layering Rules

1. `symphony-domain` is the innermost crate and must stay free of runtime,
   HTTP, CLI, and concrete adapter dependencies.
2. Contract crates may depend on `symphony-domain`, but not on CLI or HTTP.
3. Concrete adapters may depend on contract crates and shared utility crates,
   but not on `symphony-cli`.
4. `symphony-runtime` coordinates contracts and adapters, but must not depend
   on `symphony-http` or `symphony-cli`.
5. `symphony-observability` shapes runtime-facing read models and is consumed
   by `symphony-http`, `symphony-cli`, and tests.
6. `symphony-cli` is the composition root.

## Workspace Dependency Graph

| Crate | Depends on workspace crates |
| --- | --- |
| `symphony-domain` | none |
| `symphony-config` | none |
| `symphony-workflow` | none |
| `symphony-tracker` | `symphony-domain` |
| `symphony-tracker-linear` | `symphony-domain`, `symphony-tracker` |
| `symphony-workspace` | none |
| `symphony-agent-protocol` | none |
| `symphony-observability` | `symphony-domain` |
| `symphony-runtime` | `symphony-agent-protocol`, `symphony-config`, `symphony-domain`, `symphony-observability`, `symphony-tracker`, `symphony-workflow`, `symphony-workspace` |
| `symphony-http` | `symphony-observability` |
| `symphony-cli` | `symphony-config`, `symphony-domain`, `symphony-http`, `symphony-observability`, `symphony-runtime`, `symphony-tracker`, `symphony-tracker-linear`, `symphony-workflow` |
| `symphony-testkit` | `symphony-agent-protocol`, `symphony-domain`, `symphony-observability`, `symphony-tracker` |

## Composition Notes

- `symphony-runtime` intentionally targets traits and shared contracts. The CLI
  chooses the concrete tracker adapter.
- `symphony-http` is a serialization layer. It does not own runtime polling or
  tracker fetch behavior.
- `symphony-testkit` depends on core contracts so conformance and interleaving
  suites can share deterministic fixtures without depending on the CLI.

## External Dependency Hotspots

| External crate | Primary consumers | Why it exists |
| --- | --- | --- |
| `tokio` | `symphony-runtime`, `symphony-cli`, `symphony-testkit`, `symphony-tracker-linear` | async tasks, process management, timers, channels |
| `reqwest` | `symphony-runtime`, `symphony-tracker-linear` | tracker and tool-call HTTP transport |
| `axum` | `symphony-http`, `symphony-cli` | HTTP routing and host wiring |
| `serde` / `serde_json` | most crates | typed config, protocol, snapshots, JSON payloads |
| `tracing` | `symphony-runtime`, `symphony-cli` | structured diagnostics |
| `wiremock` | runtime and Linear adapter tests | deterministic HTTP mocking |

## Drift Checks

When crate boundaries change, update this file and confirm:

1. no inner crate starts depending on `symphony-cli` or `symphony-http`
2. `symphony-runtime` remains free of outer-surface dependencies
3. concrete adapter choice remains outside the domain crate
4. test-only conveniences stay in `symphony-testkit`, not production crates
