## Project Overview

Symphony is a service that orchestrates coding agents to autonomously complete tasks from an issue tracker (e.g., Linear). It is designed to turn software development work into isolated, repeatable, and observable implementation runs.

The core of the project is a detailed specification document, `SPEC.md`, which defines a language-agnostic architecture for the Symphony service. The repository contains two implementations of this specification:

1.  **Elixir:** The original reference implementation. It is considered a prototype.
2.  **Rust:** A newer, clean-room redesign focused on reliability, modularity, and formal verification, using a reducer-first architecture.

The goal of Symphony is to allow engineering teams to manage work at a higher level of abstraction, letting autonomous agents handle the coding, testing, and pull request process.

## Architecture

The system is composed of several key components as defined in `SPEC.md`:

*   **Orchestrator:** The central control plane that polls the issue tracker, dispatches work, and manages the state of all agent sessions.
*   **Workspace Manager:** Creates and manages isolated filesystem directories for each issue, ensuring agent actions are contained.
*   **Agent Runner:** Launches and communicates with the coding agent (e.g., OpenAI Codex in "app-server" mode) over a JSON-RPC protocol via stdio.
*   **Issue Tracker Client:** An adapter for fetching data from an issue tracker API (e.g., Linear).
*   **Workflow Definition:** A repository-local `WORKFLOW.md` file defines the agent's prompt, runtime configuration (concurrency, timeouts, etc.), and lifecycle hooks.
*   **Observability:** Provides structured logs and an optional HTTP server with a status dashboard and JSON API for monitoring.

The Rust implementation follows a particularly clean "reducer-first" pattern, where a pure `reduce` function is the only authority on state transitions, and all I/O is handled by isolated adapters.

## Building and Running

The repository is a monorepo containing distinct Elixir and Rust projects.

### Elixir Implementation

The Elixir implementation is located in the `elixir/` directory.

**Prerequisites:**

*   Elixir and Erlang, managed via `mise`.

**Setup and Build Commands:**

```bash
# Navigate to the elixir directory
cd elixir

# Install and trust the local tool versions
mise trust
mise install

# Install dependencies and build the project
mise exec -- mix setup
mise exec -- mix build
```

**Running the Service:**

```bash
# Run from the elixir/ directory
mise exec -- ./bin/symphony /path/to/your/WORKFLOW.md
```

**Testing:**

```bash
# Run all Elixir tests from the elixir/ directory
make all
```

### Rust Implementation

The Rust implementation is a workspace located in the `rust/` directory.

**Prerequisites:**

*   Rust toolchain (version specified in `rust/rust-toolchain.toml`).

**Build and Test Commands:**

All commands should be run from the `rust/` directory.

```bash
# Navigate to the rust directory
cd rust

# Check formatting
cargo fmt --all

# Lint the codebase (strict: warnings are errors)
cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
cargo test --workspace

# Build the project
cargo build --workspace
```

## Development Conventions

*   **Specification-Driven:** Both implementations adhere to the `SPEC.md`. The Rust implementation in particular has a strong focus on correctness and verification, with extensive architecture documentation in `rust/docs/adr`.
*   **Configuration in Repo:** The `WORKFLOW.md` file is a critical part of the project, defining the runtime behavior of the agent for a given repository. It is expected to be version-controlled alongside the project's source code.
*   **Isolated Workspaces:** All agent activity is strictly confined to a per-issue workspace directory, typically created under a root directory like `/tmp/symphony_workspaces`.
*   **Testing:** The project has a strong emphasis on testing. The Elixir implementation uses `make all` to run its test suite. The Rust project uses standard `cargo test` and includes a `testkit` crate for shared test fixtures. The Rust project also includes formal verification proofs using Verus.
