export const minimalWorkflow = `---
tracker:
  kind: linear
  project_slug: "test-project"
  active_states:
    - Todo
  terminal_states:
    - Done
polling:
  interval_ms: 30000
workspace:
  root: /tmp/symphony-workspaces
agent:
  max_concurrent_agents: 2
  max_turns: 5
codex:
  command: echo "Mock codex"
  approval_policy: never
server:
  port: 8080
---

Test workflow for {{ issue.identifier }}
`;

export const fullWorkflow = `---
tracker:
  kind: linear
  project_slug: "symphony-test"
  active_states:
    - Todo
    - In Progress
    - Review
  terminal_states:
    - Done
    - Closed
    - Cancelled
polling:
  interval_ms: 10000
workspace:
  root: /tmp/symphony-workspaces
hooks:
  after_create: |
    echo "Workspace created for {{ issue.identifier }}"
  before_run: |
    echo "Starting work on {{ issue.identifier }}"
  after_run: |
    echo "Completed attempt {{ attempt }}"
  before_remove: |
    echo "Removing workspace for {{ issue.identifier }}"
agent:
  max_concurrent_agents: 5
  max_turns: 20
  max_retry_backoff_ms: 3600000
codex:
  command: codex --config shell_environment_policy.inherit=all app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy:
    type: workspaceWrite
server:
  port: 8080
---

You are working on ticket {{ issue.identifier }}

Title: {{ issue.title }}
Description: {{ issue.description }}
Priority: {{ issue.priority }}
State: {{ issue.state }}

This is attempt {{ attempt }}.

Please analyze the issue and implement a solution.
`;

export const invalidWorkflow = `---
tracker:
  kind: unknown_tracker
---

Invalid workflow
`;

export const emptyWorkflow = ``;
