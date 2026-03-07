#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="${RESULTS_DIR:-/tmp/symphony-benchmark-results-$(date +%Y%m%d-%H%M%S)}"
FAKE_LINEAR_PORT="${FAKE_LINEAR_PORT:-4010}"
FAKE_LINEAR_URL="http://127.0.0.1:${FAKE_LINEAR_PORT}/graphql"
WORKFLOW_PATH="${ROOT_DIR}/benchmarks/workflows/common-workflow.md"
RUST_PORT="${RUST_PORT:-4301}"
ELIXIR_PORT="${ELIXIR_PORT:-4302}"
BENCH_ISSUE_COUNT="${BENCH_ISSUE_COUNT:-200}"
STARTUP_SETTLE_SECONDS="${STARTUP_SETTLE_SECONDS:-2}"
CURRENT_IMPL_PID=""
FAKE_LINEAR_PID=""

mkdir -p "${RESULTS_DIR}"

cleanup() {
  if [[ -n "${CURRENT_IMPL_PID}" ]] && kill -0 "${CURRENT_IMPL_PID}" 2>/dev/null; then
    kill "${CURRENT_IMPL_PID}" 2>/dev/null || true
    wait "${CURRENT_IMPL_PID}" 2>/dev/null || true
  fi

  if [[ -n "${FAKE_LINEAR_PID}" ]] && kill -0 "${FAKE_LINEAR_PID}" 2>/dev/null; then
    kill "${FAKE_LINEAR_PID}" 2>/dev/null || true
    wait "${FAKE_LINEAR_PID}" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

require_tool() {
  local tool="$1"
  if ! command -v "${tool}" >/dev/null 2>&1; then
    echo "missing required tool: ${tool}" >&2
    exit 1
  fi
}

print_header() {
  local label="$1"
  printf '\n== %s ==\n' "${label}"
}

start_fake_linear_server() {
  print_header "Starting fake Linear server"
  python3 "${ROOT_DIR}/benchmarks/fake_linear_server.py" \
    --host 127.0.0.1 \
    --port "${FAKE_LINEAR_PORT}" \
    --issue-count "${BENCH_ISSUE_COUNT}" \
    >"${RESULTS_DIR}/fake-linear.log" 2>&1 &
  FAKE_LINEAR_PID=$!

  python3 "${ROOT_DIR}/benchmarks/startup_probe.py" \
    --url "http://127.0.0.1:${FAKE_LINEAR_PORT}/health" \
    --timeout-seconds 10
}

capture_environment() {
  print_header "Capturing benchmark environment"
  {
    echo "timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "uname=$(uname -a)"
    echo "cargo=$(cargo --version)"
    echo "rustc=$(cargo rustc -- --version 2>/dev/null | tail -n 1 || true)"
    echo "hyperfine=$(hyperfine --version)"
    echo "k6=$(k6 version | head -n 1)"
    echo "python3=$(python3 --version 2>&1)"
    echo "mise=$(mise --version)"
    echo "bench_issue_count=${BENCH_ISSUE_COUNT}"
    echo "workflow_path=${WORKFLOW_PATH}"
  } >"${RESULTS_DIR}/environment.txt"
}

build_targets() {
  print_header "Building benchmark targets"
  cargo build --release --manifest-path "${ROOT_DIR}/rust/Cargo.toml" --package symphony-cli
  (
    cd "${ROOT_DIR}/elixir"
    mise exec -- mix setup
    mise exec -- make build
  )
}

run_rust_criterion() {
  print_header "Running Rust Criterion benchmarks"
  cargo bench --manifest-path "${ROOT_DIR}/benches/Cargo.toml" -- --noplot \
    | tee "${RESULTS_DIR}/rust-criterion.txt"
}

run_startup_hyperfine() {
  print_header "Running startup hyperfine comparison"
  hyperfine \
    --warmup 1 \
    --runs 5 \
    --export-json "${RESULTS_DIR}/startup-hyperfine.json" \
    "${ROOT_DIR}/benchmarks/bin/run_rust_startup_probe.sh" \
    "${ROOT_DIR}/benchmarks/bin/run_elixir_startup_probe.sh"
}

start_impl() {
  local impl="$1"
  local port="$2"
  local log_path="${RESULTS_DIR}/${impl}-server.log"

  if [[ "${impl}" == "rust" ]]; then
    "${ROOT_DIR}/rust/target/release/symphony-cli" \
      "${WORKFLOW_PATH}" \
      --bind 127.0.0.1 \
      --port "${port}" \
      >"${log_path}" 2>&1 &
  else
    (
      cd "${ROOT_DIR}/elixir"
      mise exec -- ./bin/symphony \
        --i-understand-that-this-will-be-running-without-the-usual-guardrails \
        "${WORKFLOW_PATH}" \
        --port "${port}"
    ) >"${log_path}" 2>&1 &
  fi

  CURRENT_IMPL_PID=$!
  python3 "${ROOT_DIR}/benchmarks/startup_probe.py" \
    --url "http://127.0.0.1:${port}/api/v1/state" \
    --timeout-seconds 30
  sleep "${STARTUP_SETTLE_SECONDS}"

  local snapshot_path="${RESULTS_DIR}/${impl}-state-snapshot.json"
  if [[ ! -f "${snapshot_path}" ]]; then
    curl -fsS "http://127.0.0.1:${port}/api/v1/state" >"${snapshot_path}"
  fi
}

stop_impl() {
  if [[ -n "${CURRENT_IMPL_PID}" ]] && kill -0 "${CURRENT_IMPL_PID}" 2>/dev/null; then
    kill "${CURRENT_IMPL_PID}" 2>/dev/null || true
    wait "${CURRENT_IMPL_PID}" 2>/dev/null || true
  fi
  CURRENT_IMPL_PID=""
}

run_k6_profile() {
  local impl="$1"
  local port="$2"
  local profile="$3"
  local script="$4"
  shift 4

  print_header "Running ${profile} profile for ${impl}"
  start_impl "${impl}" "${port}"

  (
    cd "${ROOT_DIR}/load-tests"
    env \
      BASE_URL="http://127.0.0.1:${port}" \
      "$@" \
      k6 run \
        --summary-trend-stats="avg,min,med,max,p(90),p(95),p(99)" \
        --summary-export "${RESULTS_DIR}/${impl}-${profile}.json" \
        "${script}"
  ) | tee "${RESULTS_DIR}/${impl}-${profile}.txt"

  stop_impl
}

run_http_profiles() {
  run_k6_profile rust "${RUST_PORT}" load load-test.js \
    RATE=80 DURATION=20s PREALLOCATED_VUS=40 MAX_VUS=200
  run_k6_profile elixir "${ELIXIR_PORT}" load load-test.js \
    RATE=80 DURATION=20s PREALLOCATED_VUS=40 MAX_VUS=200

  run_k6_profile rust "${RUST_PORT}" stress stress-test.js \
    START_RATE=40 STAGE1_RATE=100 STAGE2_RATE=180 STAGE3_RATE=260 \
    STAGE1_DURATION=15s STAGE2_DURATION=15s STAGE3_DURATION=15s RAMP_DOWN_DURATION=10s \
    PREALLOCATED_VUS=80 MAX_VUS=400
  run_k6_profile elixir "${ELIXIR_PORT}" stress stress-test.js \
    START_RATE=40 STAGE1_RATE=100 STAGE2_RATE=180 STAGE3_RATE=260 \
    STAGE1_DURATION=15s STAGE2_DURATION=15s STAGE3_DURATION=15s RAMP_DOWN_DURATION=10s \
    PREALLOCATED_VUS=80 MAX_VUS=400

  run_k6_profile rust "${RUST_PORT}" soak soak-test.js \
    RATE=40 DURATION=45s PREALLOCATED_VUS=40 MAX_VUS=120
  run_k6_profile elixir "${ELIXIR_PORT}" soak soak-test.js \
    RATE=40 DURATION=45s PREALLOCATED_VUS=40 MAX_VUS=120

  run_k6_profile rust "${RUST_PORT}" spike spike-test.js \
    BASE_RATE=20 SPIKE_RATE=220 RECOVERY_RATE=20 \
    BASE_DURATION=10s SPIKE_DURATION=10s RECOVERY_DURATION=10s \
    PREALLOCATED_VUS=80 MAX_VUS=320
  run_k6_profile elixir "${ELIXIR_PORT}" spike spike-test.js \
    BASE_RATE=20 SPIKE_RATE=220 RECOVERY_RATE=20 \
    BASE_DURATION=10s SPIKE_DURATION=10s RECOVERY_DURATION=10s \
    PREALLOCATED_VUS=80 MAX_VUS=320
}

main() {
  require_tool cargo
  require_tool curl
  require_tool hyperfine
  require_tool k6
  require_tool mise
  require_tool python3

  printf 'Benchmark results will be written to %s\n' "${RESULTS_DIR}"
  capture_environment
  build_targets
  start_fake_linear_server
  run_rust_criterion
  run_startup_hyperfine
  run_http_profiles
  printf '\nCompleted benchmark suite. Raw results: %s\n' "${RESULTS_DIR}"
}

main "$@"
