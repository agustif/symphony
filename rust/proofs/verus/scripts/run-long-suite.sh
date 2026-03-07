#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <interleavings|soak>" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
suite="$1"

cd "$RUST_ROOT"

case "$suite" in
  interleavings)
    command=(
      cargo test
      -p symphony-testkit
      --test interleavings_suite
      --
      --nocapture
    )
    ;;
  soak)
    command=(
      cargo test
      -p symphony-testkit
      --test soak_suite
      --
      --nocapture
    )
    ;;
  *)
    echo "unknown suite: $suite" >&2
    echo "supported suites: interleavings, soak" >&2
    exit 2
    ;;
esac

echo "=== Symphony Long Suite ==="
echo "Suite: $suite"
echo "Rust root: $RUST_ROOT"
echo "Command: ${command[*]}"
echo ""

"${command[@]}"
