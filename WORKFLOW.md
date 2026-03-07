---
tracker:
  kind: linear
  project_slug: "test-project"
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Closed
polling:
  interval_ms: 30000
workspace:
  root: /tmp/symphony-workspaces
hooks:
  after_create: |
    echo "Workspace created for {{ issue.identifier }}"
agent:
  max_concurrent_agents: 5
  max_turns: 10
codex:
  command: echo "Mock codex command"
  approval_policy: never
server:
  port: 8080
---

You are working on ticket {{ issue.identifier }}

Issue: {{ issue.title }}
Description: {{ issue.description }}

This is attempt {{ attempt }}.
