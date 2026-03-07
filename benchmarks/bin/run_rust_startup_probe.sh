#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PORT="${STARTUP_PORT:-4301}"

python3 "${ROOT_DIR}/benchmarks/startup_probe.py" \
  --url "http://127.0.0.1:${PORT}/api/v1/state" \
  --timeout-seconds 30 \
  -- \
  "${ROOT_DIR}/rust/target/release/symphony-cli" \
  "${ROOT_DIR}/benchmarks/workflows/common-workflow.md" \
  --bind 127.0.0.1 \
  --port "${PORT}"
