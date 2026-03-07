---
tracker:
  kind: linear
  endpoint: http://127.0.0.1:4010/graphql
  api_key: benchmark-token
  project_slug: "bench-project"
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Closed
polling:
  interval_ms: 1000
workspace:
  root: /tmp/symphony-bench-workspaces
agent:
  max_concurrent_agents: 1
  max_turns: 1
codex:
  command: echo "benchmark codex"
  approval_policy: never
---

Benchmark harness workflow used for local startup and HTTP comparison runs.
