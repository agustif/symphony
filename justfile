set shell := ["bash", "-euo", "pipefail", "-c"]

rust_dir := "rust"
elixir_dir := "elixir"

default: help

help:
  @echo "Symphony monorepo tasks"
  @echo ""
  @echo "Bootstrap"
  @echo "  just setup              # Install Elixir deps and build the Rust workspace"
  @echo "  just rust-setup         # Build the Rust workspace"
  @echo "  just elixir-setup       # Run mix setup"
  @echo ""
  @echo "Formatting"
  @echo "  just format             # Format Rust and Elixir code"
  @echo "  just rust-format        # cargo fmt --all --"
  @echo "  just rust-format-check  # cargo fmt --all --check"
  @echo "  just elixir-format      # make -C elixir fmt"
  @echo ""
  @echo "Quality Gates"
  @echo "  just lint               # Run Rust clippy and Elixir lint"
  @echo "  just test               # Run Rust and Elixir test suites"
  @echo "  just validate           # Run full Rust + Elixir validation"
  @echo "  just full               # Alias for validate"
  @echo ""
  @echo "Rust"
  @echo "  just rust-build         # cargo build --workspace"
  @echo "  just rust-lint          # cargo clippy --workspace --all-targets -- -D warnings"
  @echo "  just rust-test          # cargo test --workspace"
  @echo "  just rust-validate      # fmt check + clippy + test"
  @echo ""
  @echo "Elixir"
  @echo "  just elixir-lint        # make -C elixir lint"
  @echo "  just elixir-test        # make -C elixir test"
  @echo "  just elixir-validate    # make -C elixir all"

setup: rust-setup elixir-setup

format: rust-format elixir-format

lint: rust-lint elixir-lint

test: rust-test elixir-test

validate: rust-validate elixir-validate

full: validate

rust-setup:
  cd {{rust_dir}} && cargo build --workspace

rust-build:
  cd {{rust_dir}} && cargo build --workspace

rust-format:
  cd {{rust_dir}} && cargo fmt --all --

rust-format-check:
  cd {{rust_dir}} && cargo fmt --all --check

rust-lint:
  cd {{rust_dir}} && cargo clippy --workspace --all-targets -- -D warnings

rust-test:
  cd {{rust_dir}} && cargo test --workspace

rust-validate:
  cd {{rust_dir}} && cargo fmt --all --check
  cd {{rust_dir}} && cargo clippy --workspace --all-targets -- -D warnings
  cd {{rust_dir}} && cargo test --workspace

elixir-setup:
  cd {{elixir_dir}} && mix setup

elixir-format:
  make -C {{elixir_dir}} fmt

elixir-lint:
  make -C {{elixir_dir}} lint

elixir-test:
  make -C {{elixir_dir}} test

elixir-validate:
  make -C {{elixir_dir}} all
