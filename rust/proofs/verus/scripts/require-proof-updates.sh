#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

determine_range() {
  if [[ $# -gt 0 ]]; then
    printf '%s\n' "$1"
    return 0
  fi

  if [[ -n "${GITHUB_BASE_REF:-}" ]]; then
    git fetch --no-tags --depth=1 origin "${GITHUB_BASE_REF}"
    printf 'FETCH_HEAD...HEAD\n'
    return 0
  fi

  if git rev-parse HEAD^ >/dev/null 2>&1; then
    printf 'HEAD^..HEAD\n'
    return 0
  fi

  printf '\n'
}

range="$(determine_range "${1:-}")"
if [[ -z "$range" ]]; then
  echo "No comparison range available; skipping proof-update guard."
  exit 0
fi

mapfile -t changed_files < <(git diff --name-only "$range" --)
if [[ ${#changed_files[@]} -eq 0 ]]; then
  echo "No changed files detected for range ${range}; skipping proof-update guard."
  exit 0
fi

trigger_regex='^rust/(crates/symphony-domain/src/(agent_update|invariants|lib)\.rs|crates/symphony-testkit/src/trace\.rs)$'
proof_touch_regex='^rust/(proofs/verus/specs/.*\.rs|proofs/TASKS\.md|proofs/verus/TASKS\.md|docs/architecture/overview\.md)$'

triggered=false
proof_touched=false

for path in "${changed_files[@]}"; do
  if [[ "$path" =~ $trigger_regex ]]; then
    triggered=true
  fi
  if [[ "$path" =~ $proof_touch_regex ]]; then
    proof_touched=true
  fi
done

if [[ "$triggered" != true ]]; then
  echo "No reducer-invariant or trace-contract files changed in ${range}; proof-update guard passed."
  exit 0
fi

if [[ "$proof_touched" == true ]]; then
  echo "Reducer-invariant changes are accompanied by proof or traceability updates; proof-update guard passed."
  exit 0
fi

echo "Reducer-invariant or trace-contract files changed without proof updates."
echo "Changed files:"
printf '  - %s\n' "${changed_files[@]}"
echo
echo "Expected at least one of these to change as well:"
echo "  - rust/proofs/verus/specs/*.rs"
echo "  - rust/proofs/TASKS.md"
echo "  - rust/proofs/verus/TASKS.md"
echo "  - rust/docs/architecture/overview.md"
exit 1
