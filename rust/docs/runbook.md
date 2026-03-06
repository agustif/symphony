# Symphony Rust Runbook

Owner: Rust implementation maintainers
Last updated: 2026-03-06

This runbook covers local startup, health checks, and first-response incident
triage for the Rust orchestrator.

## Startup

Run from the Rust workspace root:

```bash
cargo run -p symphony-cli -- ../elixir/WORKFLOW.md --port 4051
```

If Linear auth is provided via environment, load it before startup:

```bash
set -a
source /Users/af/symphony/.env.local
set +a
cargo run -p symphony-cli -- ../elixir/WORKFLOW.md --port 4051
```

## Expected Healthy Signals

Healthy startup should show:

- a startup banner naming the workflow path
- `symphony_runtime: Starting poll loop`
- `symphony_cli: HTTP server started`
- successful responses from `/api/v1/state` and `/api/v1/refresh`

Useful checks:

```bash
curl -fsS http://127.0.0.1:4051/api/v1/state | jq .
curl -sS -D - -o /tmp/refresh.out -X POST http://127.0.0.1:4051/api/v1/refresh
```

## Health Checks

Use these checks in order:

1. Process health
   - confirm the CLI process is running
   - confirm the HTTP port is bound if HTTP is enabled
2. Snapshot health
   - fetch `/api/v1/state`
   - verify `counts`, `activity`, and `generated_at` update
3. Poll-loop health
   - confirm `activity.last_poll_completed_at` moves forward
   - confirm `activity.next_poll_due_at` is in the future
4. Refresh health
   - `POST /api/v1/refresh` should return `202 Accepted`
5. Tracker health
   - inspect logs for tracker status, payload, or transport errors

## Common Failure Modes

| Symptom | Likely cause | First response |
| --- | --- | --- |
| Startup fails before banner | invalid workflow or config | validate `WORKFLOW.md` and CLI overrides |
| HTTP bind failure | port already in use | stop the conflicting process or pick another port |
| `/api/v1/state` returns degraded/unavailable | runtime snapshot timeout or no last-good snapshot | inspect runtime logs and poll-loop state |
| Repeated tracker 4xx/5xx | auth, endpoint, or upstream failure | validate tracker endpoint and auth token |
| Worker restarts or retry buildup | protocol timeout, tool failure, or stall logic | inspect running/retrying entries and runtime logs |
| Dashboard looks stale | poll loop alive but snapshot path degraded | inspect `activity.last_poll_*` and runtime error logs |

## Incident Triage

When the runtime is unhealthy, gather this evidence:

1. latest `cargo run` or service logs
2. `/api/v1/state`
3. `/api/v1/refresh` response headers and body
4. current `WORKFLOW.md`
5. current git revision and local diff

Then classify the incident:

- Config incident: bad workflow, bad env, invalid reload
- Tracker incident: auth, endpoint, schema, or payload drift
- Runtime incident: reconciliation, retry, or worker lifecycle bug
- Host incident: bind failure, watcher death, or shutdown bug
- Observability incident: stale snapshot, malformed payload, bad dashboard render

## Stalled Session Response

If an issue appears stuck:

1. fetch `/api/v1/state` and find the running issue entry
2. inspect last activity timestamps and retry metadata
3. inspect worker logs or protocol errors
4. force a refresh through `/api/v1/refresh`
5. if the worker is stale and the runtime does not recover it, capture:
   - issue id
   - session/thread/turn ids
   - last protocol activity timestamp
   - retry metadata

## Safe Shutdown

Preferred shutdown:

```bash
Ctrl-C
```

Expected behavior:

- CLI begins shutdown
- runtime poll loop stops
- HTTP server stops accepting requests
- active worker tasks receive stop or abort handling

If shutdown hangs, capture logs before forcing process termination.

## Validation Before Closing an Incident

Before declaring recovery:

1. `cargo test --workspace`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. live smoke run with the target workflow
4. successful `/api/v1/state` and `/api/v1/refresh` responses
