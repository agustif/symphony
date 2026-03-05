# Symphony AGENTS.md

Symphony is a monorepo containing two implementations of an autonomous agent orchestration service:
- **elixir/**: Reference Elixir implementation (stable)
- **rust/**: Clean Rust redesign (in progress, focused on formal verification)

This document provides coding agents with build commands, code style guidelines, and project conventions.

## Build/Lint/Test Commands

### Elixir

```bash
cd elixir
mix setup                           # Install dependencies
make all                            # Run all quality gates (fmt-check, lint, coverage, dialyzer)
make fmt                            # Format code
make lint                           # Run specs.check and credo --strict
make test                           # Run tests
mix test test/path/to/test.exs      # Run specific test file
mix test test/path/to/test.exs:123  # Run specific test at line 123
make coverage                       # Run tests with coverage (100% threshold with exemptions)
make dialyzer                       # Run dialyzer type checker
mix specs.check                     # Validate @spec annotations on public functions
```

### Rust

```bash
cd rust
cargo fmt --all --                  # Format code
cargo fmt --all --check             # Check formatting
cargo clippy --workspace --all-targets -- -D warnings  # Lint (warnings as errors)
cargo test --workspace              # Run all tests
cargo test --package symphony-domain # Run tests for specific crate
cargo test test_reduce_claim        # Run specific test
cargo test --test integration       # Run integration tests (in tests/ dir)
cargo build --workspace             # Build all crates
```

## Project Structure

```
symphony/
├── SPEC.md                 # Language-agnostic service specification (authoritative)
├── elixir/                 # Elixir implementation
│   ├── lib/symphony_elixir/
│   ├── test/
│   ├── WORKFLOW.md         # Elixir workflow configuration
│   └── AGENTS.md           # Elixir-specific conventions
└── rust/                   # Rust implementation
    ├── crates/             # Workspace crates (domain, config, runtime, etc.)
    ├── proofs/             # Verus formal verification proofs
    ├── tests/              # Integration/soak tests
    └── docs/adr/           # Architecture Decision Records
```

## Code Style Guidelines

### Elixir

**Imports and Dependencies:**
- Add dependencies to `mix.exs` under `deps/0`
- Import modules at the top of the file after `defmodule`
- Use alias for frequently referenced modules: `alias SymphonyElixir.Tracker`

**Formatting:**
- Line length: 200 characters (config in `.formatter.exs`)
- Run `mix format` before committing
- Use `mix format --check-formatted` in CI

**Types and Specs:**
- **Required**: All public functions (`def`) in `lib/` must have `@spec` annotations
- `defp` specs are optional
- `@impl` callback implementations are exempt from local `@spec` requirement
- Validate with: `mix specs.check`
- Example:
  ```elixir
  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    SymphonyElixir.Orchestrator.start_link(opts)
  end
  ```

**Naming Conventions:**
- Modules: PascalCase (`SymphonyElixir.WorkflowStore`)
- Functions: snake_case (`start_link`, `prepare_workspace`)
- Constants: SCREAMING_SNAKE_CASE (`DEFAULT_HOOK_TIMEOUT_MS`)
- Private functions: prefix with underscore if intentionally unused (`_format_output`)

**Error Handling:**
- Return tagged tuples: `{:ok, result}`, `{:error, reason}`
- Use `with` for chaining operations that can fail
- Raise only for truly exceptional/unrecoverable errors
- Log errors with context (issue_id, session_id) per `docs/logging.md`

**Testing:**
- Place tests in `test/` mirroring `lib/` structure
- Use `ExUnit.Case` with `async: false` for tests with shared state
- Coverage threshold: 100% (with exemptions in `mix.exs`)
- Use `setup` blocks for test fixtures
- Run targeted tests while iterating, full suite before handoff

### Rust

**Imports and Dependencies:**
- Add workspace dependencies to root `Cargo.toml` under `[workspace.dependencies]`
- Add crate dependencies to individual `Cargo.toml` under `[dependencies]`
- Group imports: std → external crates → internal modules
- Use `use` at module level, not function level

**Formatting:**
- Run `cargo fmt --all` before committing
- Uses nightly rustfmt (specified in `rust-toolchain.toml`)
- Check with: `cargo fmt --all --check`

**Types and Safety:**
- **Required**: Add `#![forbid(unsafe_code)]` at top of every `lib.rs`
- Use `thiserror` for error types, `anyhow` for applications
- Prefer `Result<T, E>` over panics
- Leverage type system: newtypes for IDs (`IssueId(pub String)`)
- Example:
  ```rust
  #![forbid(unsafe_code)]
  
  #[derive(Debug, Error, PartialEq, Eq)]
  pub enum InvariantError {
      #[error("issue cannot run without claim")]
      RunningWithoutClaim,
  }
  ```

**Naming Conventions:**
- Types/Structs/Enums: PascalCase (`OrchestratorState`, `TransitionRejection`)
- Functions/Variables: snake_case (`reduce`, `validate_invariants`)
- Constants: SCREAMING_SNAKE_CASE (`DEFAULT_HOOK_TIMEOUT_MS`)
- Modules: snake_case (`symphony_domain`, `symphony_config`)
- Newtypes: descriptive wrapper names (`IssueId`, not `String`)

**Error Handling:**
- Use `thiserror::Error` derive for error enums
- Return `Result<T, E>` from fallible functions
- Use `?` operator for error propagation
- Never panic in library code; return `Result`
- Provide descriptive error messages with `#[error("...")]` attributes

**Testing:**
- Place unit tests in `#[cfg(test)] mod tests` at bottom of file
- Place integration tests in `tests/` directory
- Property testing encouraged for reducer logic
- Run with: `cargo test --workspace`
- Example:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      
      #[test]
      fn test_reduce_claim() {
          let state = OrchestratorState::default();
          let (new_state, commands) = reduce(state, Event::Claim(IssueId("123".into())));
          assert!(new_state.claimed.contains(&IssueId("123".into())));
      }
  }
  ```

## Architecture and Design Principles

**Specification Alignment:**
- Keep implementations aligned with `SPEC.md` (root level)
- Implementation may be a superset of the spec
- Implementation must not conflict with the spec
- If behavior changes meaningfully, update `SPEC.md` in the same PR

**Elixir-Specific:**
- Runtime config loaded from `WORKFLOW.md` front matter via `SymphonyElixir.Workflow` and `SymphonyElixir.Config`
- Prefer adding config access through `SymphonyElixir.Config` instead of ad-hoc env reads
- Workspace safety is critical: never run Codex with cwd in source repo
- Orchestrator is stateful and concurrency-sensitive; preserve retry/reconciliation/cleanup semantics
- Follow `elixir/docs/logging.md` for logging conventions

**Rust-Specific:**
- Follow reducer-first architecture (see `rust/docs/adr/0001-reducer-first-runtime.md`)
- Pure reducer core: keep async/IO outside reducer transitions
- Leverage formal verification with Verus for runtime invariants
- Maintain clear adapter boundaries (see `rust/docs/adr/0003-adapter-boundary-contract.md`)

## PR Requirements

- PR body must follow `.github/pull_request_template.md` exactly
- Required sections: Context, TL;DR, Summary, Alternatives, Test Plan
- Run validation: `mix pr_body.check --file /path/to/pr_body.md` (Elixir)
- Keep changes narrowly scoped; avoid unrelated refactors
- Update documentation in the same PR:
  - Root `README.md` for project concepts
  - `elixir/README.md` or `rust/README.md` for implementation details
  - `WORKFLOW.md` for workflow/config contract changes

## CI/CD

**GitHub Actions Workflows:**
- `.github/workflows/make-all.yml` - Elixir quality gates
- `.github/workflows/rust-ci.yml` - Rust format, clippy, test
- `.github/workflows/rust-proofs.yml` - Verus verification (Rust)

**Required Checks:**
- All PRs must pass `make -C elixir all` (Elixir changes)
- All PRs must pass `cargo fmt`, `cargo clippy`, `cargo test` (Rust changes)
- PR description lint must pass

## Additional Resources

- [SPEC.md](SPEC.md) - Authoritative service specification
- [elixir/AGENTS.md](elixir/AGENTS.md) - Elixir-specific conventions
- [rust/README.md](rust/README.md) - Rust workspace overview
- [rust/docs/adr/](rust/docs/adr/) - Architecture Decision Records
