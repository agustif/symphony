# Symphony Rust Runtime (Clean Redesign)

This workspace is the new long-term Rust implementation of Symphony, designed from first principles around a deterministic orchestrator core and strong correctness guarantees.

## Goals
- Preserve the Symphony spec intent while redesigning internals for stronger modularity and reliability.
- Keep one authoritative orchestrator reducer as the control plane.
- Enforce test coverage and documentation at every layer.
- Add formal verification for runtime invariants via Verus.

## Workspace Layout
- `crates/symphony-domain`: domain model and reducer invariants.
- `crates/symphony-config`: typed config and validation.
- `crates/symphony-workflow`: `WORKFLOW.md` parsing and prompt extraction.
- `crates/symphony-tracker`: tracker traits and contracts.
- `crates/symphony-tracker-linear`: Linear implementation.
- `crates/symphony-workspace`: workspace lifecycle and safety.
- `crates/symphony-agent-protocol`: app-server protocol codec/contract.
- `crates/symphony-runtime`: orchestrator runtime loop.
- `crates/symphony-http`: observability API surface.
- `crates/symphony-observability`: snapshot and metrics view model.
- `crates/symphony-cli`: executable entrypoint.
- `crates/symphony-testkit`: reusable fixtures/fakes for integration tests.

## Architecture Docs
- [Architecture Overview](docs/architecture/overview.md)
- [ADR-0001: Reducer-First Runtime](docs/adr/0001-reducer-first-runtime.md)
- [ADR-0002: Runtime Execution Model](docs/adr/0002-runtime-execution-model.md)
- [ADR-0003: Adapter Boundary Contract](docs/adr/0003-adapter-boundary-contract.md)
- [ADR-0004: Retry Semantics](docs/adr/0004-retry-semantics.md)
- [ADR-0005: Observability Contract](docs/adr/0005-observability-contract.md)

## Local Quality Gates
```bash
cd rust
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Proof and soak suites are tracked under `proofs/` and `tests/`.
