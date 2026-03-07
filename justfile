set shell := ["bash", "-euo", "pipefail", "-c"]

rust_dir := "rust"
elixir_dir := "elixir"
bench_dir := "benches"
load_test_dir := "load-tests"
compare_doc := "docs/elixir-vs-rust.md"
compare_script := "bench-compare.sh"

default: help

help:
  @echo "Symphony monorepo tasks"
  @echo ""
  @echo "Bootstrap"
  @echo "  just setup              # Install Elixir deps and build the Rust workspace"
  @echo "  just rust-setup         # Build the Rust workspace"
  @echo "  just elixir-setup       # Run mix setup"
  @echo ""
  @echo "Build"
  @echo "  just build              # Build the Rust workspace and Elixir escript"
  @echo "  just rust-build         # cargo build --workspace"
  @echo "  just elixir-build       # make -C elixir build"
  @echo ""
  @echo "Formatting"
  @echo "  just format             # Format Rust and Elixir code"
  @echo "  just rust-format        # cargo fmt --all --"
  @echo "  just rust-format-check  # cargo fmt --all --check"
  @echo "  just elixir-format      # make -C elixir fmt"
  @echo "  just elixir-format-check # make -C elixir fmt-check"
  @echo ""
  @echo "Quality Gates"
  @echo "  just quick              # Run fast repo checks"
  @echo "  just lint               # Run Rust clippy and Elixir lint"
  @echo "  just test               # Run Rust and Elixir test suites"
  @echo "  just validate           # Run full Rust + Elixir validation"
  @echo "  just check              # Alias for validate"
  @echo "  just ci                 # Alias for validate"
  @echo "  just full               # Alias for validate"
  @echo ""
  @echo "Rust"
  @echo "  just rust-lint          # cargo clippy --workspace --all-targets -- -D warnings"
  @echo "  just rust-test          # cargo test --workspace"
  @echo "  just rust-validate      # fmt check + clippy + test"
  @echo ""
  @echo "Elixir"
  @echo "  just elixir-lint        # make -C elixir lint"
  @echo "  just elixir-test        # make -C elixir test"
  @echo "  just elixir-validate    # make -C elixir all"
  @echo ""
  @echo "Benchmarking"
  @echo "  just bench-prereqs      # Check cargo/mix/k6/hyperfine availability"
  @echo "  just bench-rust-check   # Compile Rust Criterion benches"
  @echo "  just bench-rust         # Run Rust Criterion benches"
  @echo "  just bench-load         # Run load-tests/load-test.js via k6"
  @echo "  just bench-stress       # Run load-tests/stress-test.js via k6"
  @echo "  just bench-soak         # Run load-tests/soak-test.js via k6"
  @echo "  just bench-spike        # Run load-tests/spike-test.js via k6"
  @echo "  just bench-suite        # Run the full local comparison suite"
  @echo "  just bench-guide        # Print the fair benchmark methodology doc path"

setup: rust-setup elixir-setup

build: rust-build elixir-build

format: rust-format elixir-format

quick: rust-format-check elixir-format-check rust-test elixir-test

lint: rust-lint elixir-lint

test: rust-test elixir-test

validate: rust-validate elixir-validate

check: validate

ci: validate

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
  cd {{elixir_dir}} && mise exec -- mix setup

elixir-build:
  cd {{elixir_dir}} && mise exec -- make build

elixir-format:
  cd {{elixir_dir}} && mise exec -- make fmt

elixir-format-check:
  cd {{elixir_dir}} && mise exec -- make fmt-check

elixir-lint:
  cd {{elixir_dir}} && mise exec -- make lint

elixir-test:
  cd {{elixir_dir}} && mise exec -- make test

elixir-validate:
  cd {{elixir_dir}} && mise exec -- make all

bench-prereqs:
  @for tool in cargo k6 hyperfine python3 mise; do \
    if command -v "$tool" >/dev/null 2>&1; then \
      echo "$tool: found"; \
    else \
      echo "$tool: missing"; \
    fi; \
  done
  @if (cd {{elixir_dir}} && mise exec -- mix --version >/dev/null 2>&1); then \
    echo "mix (via mise): found"; \
  else \
    echo "mix (via mise): missing"; \
  fi

bench-rust-check:
  cargo check --manifest-path {{bench_dir}}/Cargo.toml --benches

bench-rust:
  cargo bench --manifest-path {{bench_dir}}/Cargo.toml

bench-load base_url="http://127.0.0.1:8080":
  cd {{load_test_dir}} && BASE_URL={{base_url}} k6 run load-test.js

bench-stress base_url="http://127.0.0.1:8080":
  cd {{load_test_dir}} && BASE_URL={{base_url}} k6 run stress-test.js

bench-soak base_url="http://127.0.0.1:8080":
  cd {{load_test_dir}} && BASE_URL={{base_url}} k6 run soak-test.js

bench-spike base_url="http://127.0.0.1:8080":
  cd {{load_test_dir}} && BASE_URL={{base_url}} k6 run spike-test.js

bench-suite:
  bash ./{{compare_script}}

bench-guide:
  @echo "See {{compare_doc}} for the fair Elixir vs Rust benchmark methodology."
