#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
SNAPSHOT_FILE="$ROOT_DIR/rust/proofs/verus/reference/verus-guide-print.md"
COMMIT_FILE="$ROOT_DIR/rust/proofs/verus/reference/verus-upstream-commit.txt"
SYNC_SCRIPT="$ROOT_DIR/rust/proofs/verus/scripts/sync-verus-guide-print.sh"

if [[ ! -f "$SNAPSHOT_FILE" ]]; then
  echo "missing snapshot file: $SNAPSHOT_FILE" >&2
  exit 2
fi

if [[ ! -f "$COMMIT_FILE" ]]; then
  echo "missing commit pin file: $COMMIT_FILE" >&2
  exit 2
fi

expected_commit="$(tr -d '[:space:]' < "$COMMIT_FILE")"
if [[ -z "$expected_commit" ]]; then
  echo "commit pin file is empty: $COMMIT_FILE" >&2
  exit 2
fi

if ! grep -Fq -- "- Upstream commit: \`${expected_commit}\`" "$SNAPSHOT_FILE"; then
  echo "snapshot metadata does not match pinned commit ${expected_commit}" >&2
  exit 1
fi

clone_dir="$(mktemp -d /tmp/verus-upstream-verify-XXXXXX)"
tmp_out="$(mktemp /tmp/verus-guide-print-verify-XXXXXX.md)"

cleanup() {
  rm -rf "$clone_dir" "$tmp_out"
}
trap cleanup EXIT

git init "$clone_dir" >/dev/null
git -C "$clone_dir" remote add origin https://github.com/verus-lang/verus.git
git -C "$clone_dir" fetch --depth 1 origin "$expected_commit" >/dev/null
git -C "$clone_dir" checkout --detach FETCH_HEAD >/dev/null

"$SYNC_SCRIPT" "$clone_dir" --output "$tmp_out"

if ! cmp -s "$SNAPSHOT_FILE" "$tmp_out"; then
  echo "snapshot is stale; regenerate with:" >&2
  echo "  rust/proofs/verus/scripts/sync-verus-guide-print.sh $clone_dir" >&2
  echo "" >&2
  diff -u "$SNAPSHOT_FILE" "$tmp_out" | sed -n '1,200p' >&2 || true
  exit 1
fi

echo "verus guide snapshot verified against pinned commit ${expected_commit}"
