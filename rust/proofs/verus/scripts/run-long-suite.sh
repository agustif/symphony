#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <interleavings|soak>" >&2
  exit 2
fi

suite="$1"

case "$suite" in
  interleavings)
    planned_command="cargo test --test interleavings -- --nocapture"
    ;;
  soak)
    planned_command="cargo test --test soak -- --ignored --nocapture"
    ;;
  *)
    echo "unknown suite: $suite" >&2
    echo "supported suites: interleavings, soak" >&2
    exit 2
    ;;
esac

echo "[placeholder] long suite '$suite' is not implemented yet"
echo "planned command: $planned_command"
echo "placeholder completed successfully"
