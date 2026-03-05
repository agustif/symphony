#!/usr/bin/env bash
set -euo pipefail

profile="quick"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      profile="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ "$profile" != "quick" && "$profile" != "full" ]]; then
  echo "unsupported profile: $profile" >&2
  echo "supported profiles: quick, full" >&2
  exit 2
fi

echo "[placeholder] verus proof checks are not implemented yet"
echo "selected profile: $profile"

echo "planned commands:"
if [[ "$profile" == "quick" ]]; then
  echo "  - verus --crate-name symphony-runtime proofs/verus/specs/runtime_quick.rs"
else
  echo "  - verus --crate-name symphony-runtime proofs/verus/specs/runtime_full.rs"
  echo "  - verus --crate-name symphony-runtime proofs/verus/specs/session_liveness.rs"
fi

echo "placeholder completed successfully"
