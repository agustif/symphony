#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/opencode/fanout_retry.sh --tasks-file <path> [options]

Tasks file format:
  One task per line, tab-separated:
    <task_id>\t<prompt>
  Blank lines and lines starting with # are ignored.

Options:
  --tasks-file <path>         Required TSV input file.
  --max-parallel <n>          Concurrent task runs (default: 6).
  --max-retries <n>           Max attempts per task (default: 4).
  --providers <csv>           Provider priority order.
  --model-map <path>          Optional provider=model mapping file.
  --agent <name>              Opencode agent template (default: build).
  --base-branch <name>        Base git branch for worktrees (default: current branch).
  --worktree-root <path>      Worktree root dir (default: ../symphony-opencode-worktrees).
  --output-root <path>        Output dir for logs/status (default: .opencode-runs).
  --dry-run                   Print planned commands without executing.
  --help                      Show help.

Environment overrides:
  OPENCODE_MAX_PARALLEL
  OPENCODE_MAX_RETRIES
  OPENCODE_PROVIDERS
  OPENCODE_AGENT
  OPENCODE_BASE_BRANCH
  OPENCODE_WORKTREE_ROOT
  OPENCODE_OUTPUT_ROOT
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"
}

trim() {
  printf '%s' "$1" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

sanitize_id() {
  printf '%s' "$1" | tr -cs 'A-Za-z0-9._-' '-'
}

lookup_array_map() {
  local key="$1"
  local i=0
  while ((i < ${#MAP_KEYS[@]})); do
    if [[ "${MAP_KEYS[$i]}" == "$key" ]]; then
      printf '%s' "${MAP_VALUES[$i]}"
      return 0
    fi
    i=$((i + 1))
  done
  return 1
}

set_array_map() {
  local key="$1"
  local value="$2"
  local i=0
  while ((i < ${#MAP_KEYS[@]})); do
    if [[ "${MAP_KEYS[$i]}" == "$key" ]]; then
      MAP_VALUES[$i]="$value"
      return 0
    fi
    i=$((i + 1))
  done
  MAP_KEYS+=("$key")
  MAP_VALUES+=("$value")
}

lookup_resolved_model() {
  local provider="$1"
  local i=0
  while ((i < ${#RESOLVED_KEYS[@]})); do
    if [[ "${RESOLVED_KEYS[$i]}" == "$provider" ]]; then
      printf '%s' "${RESOLVED_VALUES[$i]}"
      return 0
    fi
    i=$((i + 1))
  done
  return 1
}

set_resolved_model() {
  local provider="$1"
  local model="$2"
  local i=0
  while ((i < ${#RESOLVED_KEYS[@]})); do
    if [[ "${RESOLVED_KEYS[$i]}" == "$provider" ]]; then
      RESOLVED_VALUES[$i]="$model"
      return 0
    fi
    i=$((i + 1))
  done
  RESOLVED_KEYS+=("$provider")
  RESOLVED_VALUES+=("$model")
}

resolve_model() {
  local provider="$1"
  local selected=""

  if selected="$(lookup_array_map "$provider" 2>/dev/null)"; then
    if ! opencode models "$provider" | grep -Fxq "$selected"; then
      log "model map has unavailable model for provider=$provider model=$selected"
      return 1
    fi
  else
    selected="$(opencode models "$provider" | head -n1 || true)"
    if [[ -z "$selected" ]]; then
      log "provider has no available models: $provider"
      return 1
    fi
  fi

  printf '%s' "$selected"
}

ensure_worktree() {
  local task_id="$1"
  local wt_path="$2"
  local branch="oc/$task_id"

  if [[ -e "$wt_path/.git" || -d "$wt_path/.git" ]]; then
    return 0
  fi

  if git show-ref --verify --quiet "refs/heads/$branch"; then
    git worktree add "$wt_path" "$branch" >/dev/null
  else
    git worktree add -b "$branch" "$wt_path" "$BASE_BRANCH" >/dev/null
  fi
}

TASKS_FILE=""
MAX_PARALLEL="${OPENCODE_MAX_PARALLEL:-6}"
MAX_RETRIES="${OPENCODE_MAX_RETRIES:-4}"
PROVIDERS_CSV="${OPENCODE_PROVIDERS:-zai-coding-plan,opencode-go,opencode,google,openrouter,minimax,perplexity-agent,perplexity}"
AGENT_NAME="${OPENCODE_AGENT:-build}"
BASE_BRANCH="${OPENCODE_BASE_BRANCH:-$(git rev-parse --abbrev-ref HEAD)}"
WORKTREE_ROOT="${OPENCODE_WORKTREE_ROOT:-../symphony-opencode-worktrees}"
OUTPUT_ROOT="${OPENCODE_OUTPUT_ROOT:-.opencode-runs}"
MODEL_MAP_FILE=""
DRY_RUN=0

TASK_IDS=()
TASK_PROMPTS=()
TASK_WORKTREES=()
PROVIDERS=()

MAP_KEYS=()
MAP_VALUES=()
RESOLVED_KEYS=()
RESOLVED_VALUES=()

while (($# > 0)); do
  case "$1" in
    --tasks-file)
      TASKS_FILE="$2"
      shift 2
      ;;
    --max-parallel)
      MAX_PARALLEL="$2"
      shift 2
      ;;
    --max-retries)
      MAX_RETRIES="$2"
      shift 2
      ;;
    --providers)
      PROVIDERS_CSV="$2"
      shift 2
      ;;
    --model-map)
      MODEL_MAP_FILE="$2"
      shift 2
      ;;
    --agent)
      AGENT_NAME="$2"
      shift 2
      ;;
    --base-branch)
      BASE_BRANCH="$2"
      shift 2
      ;;
    --worktree-root)
      WORKTREE_ROOT="$2"
      shift 2
      ;;
    --output-root)
      OUTPUT_ROOT="$2"
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

if [[ -z "$TASKS_FILE" ]]; then
  echo "--tasks-file is required" >&2
  usage
  exit 2
fi

if [[ ! -f "$TASKS_FILE" ]]; then
  echo "tasks file not found: $TASKS_FILE" >&2
  exit 2
fi

if ! [[ "$MAX_PARALLEL" =~ ^[0-9]+$ ]] || ((MAX_PARALLEL < 1)); then
  echo "--max-parallel must be a positive integer" >&2
  exit 2
fi

if ! [[ "$MAX_RETRIES" =~ ^[0-9]+$ ]] || ((MAX_RETRIES < 1)); then
  echo "--max-retries must be a positive integer" >&2
  exit 2
fi

require_cmd opencode
require_cmd git
require_cmd awk
require_cmd sed
require_cmd grep

if [[ -n "$MODEL_MAP_FILE" ]]; then
  if [[ ! -f "$MODEL_MAP_FILE" ]]; then
    echo "model map file not found: $MODEL_MAP_FILE" >&2
    exit 2
  fi

  while IFS= read -r raw_line; do
    local_line="$(printf '%s' "$raw_line" | sed 's/[[:space:]]*$//')"
    [[ -z "$local_line" || "$local_line" =~ ^# ]] && continue

    if [[ "$local_line" != *=* ]]; then
      echo "invalid model-map line (expected provider=model): $local_line" >&2
      exit 2
    fi

    provider="$(trim "${local_line%%=*}")"
    model="$(trim "${local_line#*=}")"

    if [[ -z "$provider" || -z "$model" ]]; then
      echo "invalid model-map line (empty provider/model): $local_line" >&2
      exit 2
    fi

    set_array_map "$provider" "$model"
  done <"$MODEL_MAP_FILE"
fi

IFS=',' read -r -a PROVIDERS <<<"$PROVIDERS_CSV"
if ((${#PROVIDERS[@]} == 0)); then
  echo "--providers must not be empty" >&2
  exit 2
fi

i=0
while ((i < ${#PROVIDERS[@]})); do
  provider="$(trim "${PROVIDERS[$i]}")"
  if [[ -z "$provider" ]]; then
    echo "--providers includes an empty value" >&2
    exit 2
  fi
  PROVIDERS[$i]="$provider"
  i=$((i + 1))
done

while IFS= read -r raw_line; do
  line="$(printf '%s' "$raw_line" | sed 's/[[:space:]]*$//')"
  [[ -z "$line" || "$line" =~ ^# ]] && continue

  if [[ "$line" != *$'\t'* ]]; then
    echo "invalid task line (expected tab separator): $line" >&2
    exit 2
  fi

  task_id_raw="${line%%$'\t'*}"
  prompt_raw="${line#*$'\t'}"

  task_id="$(sanitize_id "$(trim "$task_id_raw")")"
  prompt="$(printf '%s' "$prompt_raw" | sed 's/^[[:space:]]*//')"

  if [[ -z "$task_id" || -z "$prompt" ]]; then
    echo "invalid task line (empty id or prompt): $line" >&2
    exit 2
  fi

  TASK_IDS+=("$task_id")
  TASK_PROMPTS+=("$prompt")
done <"$TASKS_FILE"

if ((${#TASK_IDS[@]} == 0)); then
  echo "no tasks found in $TASKS_FILE" >&2
  exit 2
fi

mkdir -p "$WORKTREE_ROOT" "$OUTPUT_ROOT/status" "$OUTPUT_ROOT/logs"

log "resolving provider model map"
ACTIVE_PROVIDERS=()
for provider in "${PROVIDERS[@]}"; do
  if model="$(resolve_model "$provider")"; then
    set_resolved_model "$provider" "$model"
    log "provider ready: $provider -> $model"
    ACTIVE_PROVIDERS+=("$provider")
  else
    log "provider skipped: $provider"
  fi
done

if ((${#ACTIVE_PROVIDERS[@]} == 0)); then
  echo "no usable providers after model resolution" >&2
  exit 2
fi

PROVIDERS=("${ACTIVE_PROVIDERS[@]}")

if ((DRY_RUN == 0)); then
  log "preparing worktrees for ${#TASK_IDS[@]} tasks from base branch: $BASE_BRANCH"
fi

i=0
while ((i < ${#TASK_IDS[@]})); do
  task_id="${TASK_IDS[$i]}"
  wt_path="$WORKTREE_ROOT/$task_id"
  TASK_WORKTREES+=("$wt_path")
  if ((DRY_RUN == 0)); then
    ensure_worktree "$task_id" "$wt_path"
  fi
  i=$((i + 1))
done

run_task() {
  local index="$1"
  local task_id="${TASK_IDS[$index]}"
  local prompt="${TASK_PROMPTS[$index]}"
  local wt_path="${TASK_WORKTREES[$index]}"
  local task_dir="$OUTPUT_ROOT/logs/$task_id"
  local provider_idx=0
  local provider=""
  local model=""
  local attempt=0
  local out_file=""
  local err_file=""
  local status_line=""

  mkdir -p "$task_dir"

  attempt=1
  while ((attempt <= MAX_RETRIES)); do
    provider_idx=$(((index + attempt - 1) % ${#PROVIDERS[@]}))
    provider="${PROVIDERS[$provider_idx]}"
    model="$(lookup_resolved_model "$provider")"

    out_file="$task_dir/attempt-${attempt}-${provider}.jsonl"
    err_file="$task_dir/attempt-${attempt}-${provider}.log"

    if ((DRY_RUN == 1)); then
      log "dry-run task=$task_id attempt=$attempt provider=$provider model=$model agent=$AGENT_NAME wt=$wt_path"
      status_line="$task_id\tSUCCESS\t$attempt\t$provider\t$model\tDRY_RUN"
      printf '%b\n' "$status_line" >"$OUTPUT_ROOT/status/$task_id.tsv"
      return 0
    fi

    log "run task=$task_id attempt=$attempt provider=$provider model=$model"
    if opencode run \
      --dir "$wt_path" \
      --agent "$AGENT_NAME" \
      --model "$model" \
      --format json \
      --title "$task_id-attempt-$attempt-$provider" \
      "$prompt" >"$out_file" 2>"$err_file"; then
      status_line="$task_id\tSUCCESS\t$attempt\t$provider\t$model\t$out_file"
      printf '%b\n' "$status_line" >"$OUTPUT_ROOT/status/$task_id.tsv"
      return 0
    fi

    log "failed task=$task_id attempt=$attempt provider=$provider (see $err_file)"
    attempt=$((attempt + 1))
  done

  status_line="$task_id\tFAILED\t$MAX_RETRIES\tNONE\tNONE\t$task_dir"
  printf '%b\n' "$status_line" >"$OUTPUT_ROOT/status/$task_id.tsv"
  return 1
}

log "dispatching ${#TASK_IDS[@]} tasks with max_parallel=$MAX_PARALLEL retries=$MAX_RETRIES"

failures=0
PIDS=()

i=0
while ((i < ${#TASK_IDS[@]})); do
  run_task "$i" &
  PIDS+=($!)

  if ((${#PIDS[@]} >= MAX_PARALLEL)); then
    pid="${PIDS[0]}"
    if ! wait "$pid"; then
      failures=$((failures + 1))
    fi
    NEW_PIDS=()
    j=1
    while ((j < ${#PIDS[@]})); do
      NEW_PIDS+=("${PIDS[$j]}")
      j=$((j + 1))
    done
    if ((${#NEW_PIDS[@]} > 0)); then
      PIDS=("${NEW_PIDS[@]}")
    else
      PIDS=()
    fi
  fi

  i=$((i + 1))
done

if ((${#PIDS[@]} > 0)); then
  for pid in "${PIDS[@]}"; do
    if ! wait "$pid"; then
      failures=$((failures + 1))
    fi
  done
fi

summary_file="$OUTPUT_ROOT/summary.tsv"
{
  printf 'task_id\tstatus\tattempt\tprovider\tmodel\tartifact\n'
  cat "$OUTPUT_ROOT"/status/*.tsv | sort
} >"$summary_file"

success_count="$(awk -F'\t' 'NR > 1 && $2 == "SUCCESS" {count++} END {print count + 0}' "$summary_file")"
failed_count="$(awk -F'\t' 'NR > 1 && $2 == "FAILED" {count++} END {print count + 0}' "$summary_file")"

log "summary: success=$success_count failed=$failed_count file=$summary_file"
if ((failed_count > 0)); then
  exit 1
fi
