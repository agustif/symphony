#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PORT="${STARTUP_PORT:-4302}"

cd "${ROOT_DIR}/elixir"
mise exec -- python3 "${ROOT_DIR}/benchmarks/startup_probe.py" \
  --url "http://127.0.0.1:${PORT}/api/v1/state" \
  --timeout-seconds 30 \
  -- \
  ./bin/symphony \
  --i-understand-that-this-will-be-running-without-the-usual-guardrails \
  "${ROOT_DIR}/benchmarks/workflows/common-workflow.md" \
  --port "${PORT}"
