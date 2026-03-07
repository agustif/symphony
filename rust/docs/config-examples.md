# Configuration Examples

This document provides concrete examples of WORKFLOW.md configuration files
and their parsed representations. These examples are aligned with the
`symphony-config` crate implementation.

## Minimal WORKFLOW.md

```markdown
---
tracker:
  kind: linear
  project_slug: eng/platform
---

You are an autonomous coding agent working on issues from Linear.
```

### Parsed Config

```rust
RuntimeConfig {
    tracker: TrackerConfig {
        kind: Some("linear".to_string()),
        project_slug: Some("eng/platform".to_string()),
        endpoint: None,  // defaults to https://api.linear.app/graphql
        api_key: None,   // must be set via LINEAR_API_KEY env var
        active_states: None,  // defaults to ["Todo", "In Progress"]
        terminal_states: None,  // defaults to ["Closed", "Cancelled", ...]
    },
    polling: PollingConfig {
        interval_ms: None,  // defaults to 30000
    },
    workspace: WorkspaceConfig {
        root: None,  // defaults to /tmp/symphony_workspaces
    },
    agent: AgentConfig {
        max_concurrent_agents: None,  // defaults to 10
        max_retry_backoff_ms: None,   // defaults to 300000
        max_concurrent_agents_by_state: None,
    },
    codex: CodexConfig {
        command: None,           // defaults to "codex app-server"
        turn_timeout_ms: None,   // defaults to 3600000
        read_timeout_ms: None,   // defaults to 5000
        stall_timeout_ms: None,  // defaults to 300000
        ..Default::default()
    },
    hooks: HooksConfig::default(),
    server: None,
}
```

## Full Configuration

```markdown
---
tracker:
  kind: linear
  endpoint: https://api.linear.app/graphql
  api_key: $LINEAR_API_KEY
  project_slug: eng/platform
  active_states:
    - Todo
    - In Progress
    - In Review
  terminal_states:
    - Done
    - Cancelled
    - Duplicate

polling:
  interval_ms: 60000

workspace:
  root: ~/symphony/workspaces

agent:
  max_concurrent_agents: 5
  max_retry_backoff_ms: 600000
  max_concurrent_agents_by_state:
    "In Review": 2

codex:
  command: codex app-server
  approval_policy: suggest
  turn_timeout_ms: 1800000
  read_timeout_ms: 10000
  stall_timeout_ms: 300000

hooks:
  after_create: |
    git clone https://github.com/org/repo.git .
    npm install
  before_run: |
    git pull origin main
  after_run: |
    echo "Run completed at $(date)" >> log.txt
  before_remove: |
    rm -rf node_modules
  timeout_ms: 120000

server:
  port: 8080
  enabled: true
---

# Issue Resolution Agent

You are an expert software engineer tasked with resolving issues.

## Context

You are working on issue {{ issue.identifier }}: {{ issue.title }}

{% if issue.description %}
### Description
{{ issue.description }}
{% endif %}

{% if issue.labels %}
### Labels
{% for label in issue.labels %}
- {{ label }}
{% endfor %}
{% endif %}

{% if attempt %}
**Note:** This is attempt #{{ attempt }}.
{% endif %}

## Instructions

1. Read the issue carefully
2. Explore the codebase
3. Implement the fix
4. Write tests
5. Create a PR
```

## Environment Variable Indirection

Configuration values can reference environment variables using `$VAR_NAME` syntax:

```yaml
---
tracker:
  api_key: $LINEAR_API_KEY
  project_slug: $LINEAR_PROJECT_SLUG

workspace:
  root: $WORKSPACE_ROOT

codex:
  command: $CODEX_COMMAND
---
```

If the environment variable is not set or is empty, the value is treated as missing.

## Per-State Concurrency Limits

Control how many agents can run for issues in specific states:

```yaml
---
agent:
  max_concurrent_agents: 10
  max_concurrent_agents_by_state:
    "In Review": 2
    "Testing": 3
---
```

This allows up to 10 total concurrent agents, but limits:
- "In Review" issues to 2 concurrent runs
- "Testing" issues to 3 concurrent runs

## Hook Examples

### After Create Hook

Runs only when a new workspace directory is created:

```yaml
---
hooks:
  after_create: |
    git clone git@github.com:org/repo.git .
    pip install -r requirements.txt
    pre-commit install
---
```

### Before Run Hook

Runs before each agent attempt:

```yaml
---
hooks:
  before_run: |
    git fetch origin
    git checkout main
    git pull origin main
    git checkout -b issue/${ISSUE_IDENTIFIER}
---
```

### After Run Hook

Runs after each agent attempt (failure is logged but ignored):

```yaml
---
hooks:
  after_run: |
    # Cleanup temporary files
    rm -rf /tmp/agent-cache/*
    # Log completion
    echo "$(date): Run completed for ${ISSUE_IDENTIFIER}" >> /var/log/symphony/runs.log
---
```

### Before Remove Hook

Runs before workspace deletion (failure is logged but ignored):

```yaml
---
hooks:
  before_remove: |
    # Archive any important artifacts
    cp -r test-results/ /archive/${ISSUE_IDENTIFIER}/ 2>/dev/null || true
---
```

## Template Variables

The prompt body is rendered with these variables:

### `issue` Object

```json
{
  "id": "uuid-123",
  "identifier": "SYM-123",
  "title": "Fix login bug",
  "description": "Users cannot log in...",
  "priority": 1,
  "state": "In Progress",
  "labels": ["bug", "auth"],
  "branch_name": "fix/login-bug",
  "url": "https://linear.app/issue/SYM-123",
  "blocked_by": [],
  "created_at": 1709600000,
  "updated_at": 1709610000
}
```

### `attempt` Variable

- `null` on first attempt
- Integer (2, 3, ...) on retry or continuation

### Example Template

```jinja2
Working on {{ issue.identifier }}: {{ issue.title }}

{% if issue.priority and issue.priority <= 2 %}
This is a high-priority issue!
{% endif %}

{% if issue.blocked_by %}
Blocked by:
{% for blocker in issue.blocked_by %}
- {{ blocker.identifier }} ({{ blocker.state }})
{% endfor %}
{% endif %}

{% if attempt %}
Retry attempt #{{ attempt }}. Previous run may have had issues.
{% endif %}
```

## Validation Errors

The configuration layer produces typed errors:

| Error Type | Cause |
|------------|-------|
| `missing_workflow_file` | WORKFLOW.md not found |
| `workflow_parse_error` | Invalid YAML syntax |
| `workflow_front_matter_not_a_map` | Front matter is not a YAML map |
| `template_parse_error` | Invalid Jinja2 syntax in prompt |
| `template_render_error` | Unknown variable or filter |

## Reference

See `symphony-config` crate for:
- `RuntimeConfig` - Typed configuration model
- `from_front_matter` - Parse YAML front matter to config
- `validate` - Configuration validation
- `apply_cli_overrides` - CLI override merging

See `symphony-workflow` crate for:
- `WorkflowDefinition` - Parsed workflow with config + prompt
- Template rendering with `minijinja`
