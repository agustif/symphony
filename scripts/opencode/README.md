# Opencode Parallel Fan-Out with Provider Fallback

This directory provides a high-parallel task runner for `opencode` with model/provider fallback.

## What it solves
- Runs many tasks in parallel across isolated git worktrees.
- Retries failed tasks against the next provider/model in your priority list.
- Produces per-attempt logs plus a machine-readable summary.

## Files
- `fanout_retry.sh`: main dispatcher.
- `model-map.env.example`: provider-to-model overrides.

## Task input format
Use a TSV file with one task per line:

```text
TASK-ID<TAB>Your full prompt here
```

Example:

```text
RUST-RUNTIME-001\tImplement retry scheduling in symphony-runtime and run cargo test --workspace
RUST-TRACKER-002\tHarden Linear assignee parsing and add regression tests
```

## Run

```bash
scripts/opencode/fanout_retry.sh \
  --tasks-file ./tasks.tsv \
  --max-parallel 12 \
  --max-retries 6 \
  --providers "zai-coding-plan,opencode-go,opencode,google,openrouter,minimax,perplexity-agent,perplexity" \
  --model-map scripts/opencode/model-map.env.example \
  --agent build \
  --base-branch feat/rust-parallel-bootstrap \
  --worktree-root ../symphony-opencode-worktrees \
  --output-root .opencode-runs
```

## Output
- `.opencode-runs/logs/<task_id>/attempt-<n>-<provider>.jsonl`
- `.opencode-runs/logs/<task_id>/attempt-<n>-<provider>.log`
- `.opencode-runs/status/<task_id>.tsv`
- `.opencode-runs/summary.tsv`

## Custom agent templates
For unattended runs, use a dedicated agent template with non-interactive permissions and strict scope.
Then pass it via `--agent <name>`.

## Operational guidance
- Keep `--max-parallel` conservative first (for example `6`) and raise gradually.
- Keep fallback order deterministic and cost-aware.
- Use dedicated worktree roots to avoid branch collisions.
- Keep prompts bounded and task-specific to improve success rate per attempt.

## Install custom subagent templates

```bash
scripts/opencode/install_agents.sh
```

Then use them in fan-out runs:

```bash
scripts/opencode/fanout_retry.sh \
  --tasks-file ./tasks.tsv \
  --agent rust-zai-impl
```

For a safer first pass:

```bash
scripts/opencode/fanout_retry.sh \
  --tasks-file ./tasks.tsv \
  --max-parallel 4 \
  --max-retries 3 \
  --agent rust-go-fast
```
