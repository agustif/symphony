# Symphony Library Integrations

This document tracks third-party crates that are meaningfully integrated into the Rust workspace today.
It intentionally avoids roadmap language and omits stale test totals.

## Current Integrations

| Library | Current crates | Current integration | Scope today |
|---|---|---|---|
| `im` | `symphony-domain` | Reducer state uses `im::HashMap` / `im::HashSet` | Active runtime model dependency |
| `minijinja` | `symphony-runtime` | Worker prompt rendering uses MiniJinja with `UndefinedBehavior::Strict` | Active |
| `mockall` | `symphony-tracker` tests | `MockTrackerClient` is generated for the tracker trait | Test-only, not adopted broadly across runtime/CLI tests |
| `reqwest-middleware` + `reqwest-retry` | `symphony-tracker-linear`, `symphony-runtime` | Retry-wrapped HTTP clients are used in the Linear tracker and the runtime `linear_graphql` tool path | Active |
| `figment` | `symphony-config` | `from_figment()` exists as an alternate config-loading helper | Supplemental API, not the primary CLI/runtime config path |
| `serde_path_to_error` | `symphony-config` | `parse_yaml_with_path()` and `parse_json_with_path()` expose path-aware parse helpers | Helper API only |
| `fake` | `symphony-testkit` | Fake issue / blocker generators live in `fake_generators.rs` | Test helper only |
| `metrics` + `metrics-exporter-prometheus` | `symphony-observability` | Public `init_metrics(SocketAddr) -> Result<(), MetricsInitError>` and exported metric macros | Available, not initialized by `symphony-cli` or `symphony-runtime` by default |
| `tracing-opentelemetry` + `opentelemetry-otlp` | `symphony-observability` | Public `init_telemetry(service_name, otlp_endpoint) -> Result<(), TelemetryInitError>` | Available, not initialized by `symphony-cli` by default |
| `governor` | `symphony-tracker-linear` | Optional Linear API rate limiting via `with_rate_limit(requests, per)` | Active when caller opts in |

## Public Helper APIs Present Today

- `symphony_config::from_figment`
- `symphony_config::parse_yaml_with_path`
- `symphony_config::parse_json_with_path`
- `symphony_observability::init_metrics`
- `symphony_observability::init_telemetry`

## Notes

- `init_metrics_noop()` and `init_telemetry_noop()` are test-only private helpers, not public integration points.
- The presence of a dependency in this file does not imply workspace-wide adoption. In particular, `mockall`, `fake`, `figment`, and `serde_path_to_error` are still partial integrations.
- For current validation status, use the workspace commands in `AGENTS.md` rather than relying on a hard-coded test count here.
