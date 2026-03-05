#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/opencode/install_agents.sh [options]

Options:
  --config <path>     Opencode config path (default: ~/.config/opencode/opencode.json)
  --dry-run           Print merged JSON to stdout, do not modify files.
  --help              Show help.

This script adds reusable provider-specific subagent templates under `agent.*`.
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

CONFIG_PATH="${HOME}/.config/opencode/opencode.json"
DRY_RUN=0

while (($# > 0)); do
  case "$1" in
    --config)
      CONFIG_PATH="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

require_cmd jq

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "config not found: $CONFIG_PATH" >&2
  exit 2
fi

tmp_agents="$(mktemp)"
cat >"$tmp_agents" <<'JSON'
{
  "rust-zai-impl": {
    "description": "Rust implementation subagent optimized for deep coding tasks.",
    "mode": "subagent",
    "model": "zai-coding-plan/glm-4.7",
    "tools": {
      "bash": true,
      "read": true,
      "glob": true,
      "grep": true,
      "edit": true,
      "write": true,
      "task": true,
      "todowrite": true,
      "webfetch": true
    },
    "maxSteps": 120,
    "prompt": "You are a Rust implementation worker. Work in the assigned git worktree only. Keep changes scoped to the ticket. Run cargo fmt, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace before completion."
  },
  "rust-go-fast": {
    "description": "Fast Rust implementation subagent using opencode-go.",
    "mode": "subagent",
    "model": "opencode-go/glm-5",
    "tools": {
      "bash": true,
      "read": true,
      "glob": true,
      "grep": true,
      "edit": true,
      "write": true,
      "task": true,
      "todowrite": true,
      "webfetch": false
    },
    "maxSteps": 80,
    "prompt": "You are a high-throughput Rust coding worker. Execute bounded tasks only. Keep edits minimal and compile-safe. Always run target crate tests plus workspace checks if touched shared crates."
  },
  "rust-review-openrouter": {
    "description": "Rust reviewer subagent for strict bug/risk findings.",
    "mode": "subagent",
    "model": "openrouter/openai/gpt-5.2-codex",
    "tools": {
      "bash": true,
      "read": true,
      "glob": true,
      "grep": true,
      "edit": false,
      "write": false,
      "task": true,
      "todowrite": true,
      "webfetch": true
    },
    "maxSteps": 60,
    "prompt": "You are a Rust reviewer. Return findings first, ordered by severity with exact file and line references. Focus on behavioral bugs, regressions, missing tests, and safety/perf risks."
  },
  "rust-test-minimax": {
    "description": "Rust test expansion subagent.",
    "mode": "subagent",
    "model": "minimax/MiniMax-M2.5",
    "tools": {
      "bash": true,
      "read": true,
      "glob": true,
      "grep": true,
      "edit": true,
      "write": true,
      "task": true,
      "todowrite": true,
      "webfetch": false
    },
    "maxSteps": 90,
    "prompt": "You are a Rust test engineer. Add deterministic tests for conformance and edge cases. Prefer table-driven tests and property checks where appropriate."
  },
  "rust-research-google": {
    "description": "Research-oriented subagent for docs/spec/API checks.",
    "mode": "subagent",
    "model": "google/gemini-2.5-pro",
    "tools": {
      "bash": true,
      "read": true,
      "glob": true,
      "grep": true,
      "edit": false,
      "write": false,
      "task": true,
      "todowrite": true,
      "webfetch": true
    },
    "maxSteps": 50,
    "prompt": "You are a technical research subagent. Validate APIs and current docs before recommending dependencies or runtime architecture choices."
  }
}
JSON

merged="$(mktemp)"
jq --argjson newAgents "$(cat "$tmp_agents")" '
  .agent = ((.agent // {}) + $newAgents)
' "$CONFIG_PATH" >"$merged"

if ((DRY_RUN == 1)); then
  cat "$merged"
  rm -f "$tmp_agents" "$merged"
  exit 0
fi

backup="${CONFIG_PATH}.bak.$(date +%Y%m%d%H%M%S)"
cp "$CONFIG_PATH" "$backup"
cp "$merged" "$CONFIG_PATH"

echo "updated: $CONFIG_PATH"
echo "backup:  $backup"
echo "installed agents: rust-zai-impl, rust-go-fast, rust-review-openrouter, rust-test-minimax, rust-research-google"

rm -f "$tmp_agents" "$merged"
