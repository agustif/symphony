# Rust Cutover Checklist

Owner: Rust implementation maintainers
Last updated: 2026-03-06

This checklist is for shadow mode, primary cutover, and rollback planning.

## Preconditions

Do not declare cutover readiness until all of these are true:

- `cargo fmt --all --check` passes
- `cargo clippy --workspace --all-targets -- -D warnings` passes
- `cargo test --workspace` passes
- required conformance coverage for sections `17.1` through `17.7` and `18.1`
  is complete
- interleaving and soak suites meet their target depth, not smoke depth only
- live integration profile exists with explicit skip rules and evidence capture
- runbook is current
- rollback steps are rehearsed

## Shadow Mode Checklist

- Run Rust and Elixir against the same workflow and external dependencies.
- Compare startup behavior, refresh behavior, and degraded state behavior.
- Compare `/api/v1/state`, `/api/v1/{issue}`, `/api/v1/refresh`, and dashboard
  output for the same scenarios.
- Compare retry, stall, release, and shutdown semantics on representative issue
  lifecycles.
- Log every observed mismatch back into task maps and status docs.

## Primary Switch Checklist

- Confirm the target workflow and environment variables are identical to the
  shadowed configuration.
- Confirm tracker auth, endpoint, and project scope are validated with a live
  smoke run.
- Confirm HTTP bind port and host-owned reload settings match the deployment
  environment.
- Confirm operator dashboards and log collection point at the Rust process.
- Confirm on-call responders have the current runbook and incident commands.

## Cutover Execution Steps

1. Stop new work from being handed to the Elixir primary path.
2. Start the Rust runtime with the production-equivalent workflow.
3. Verify the startup banner, poll loop, and HTTP health endpoints.
4. Trigger a controlled refresh and observe queued/coalesced behavior.
5. Watch the first full poll cycle complete successfully.
6. Validate one real issue lifecycle end to end.

## Rollback Criteria

Rollback immediately if any of these are observed:

- startup succeeds but polling cannot sustain healthy tracker access
- repeated host task death or restart loops
- worker shutdown/reconciliation leaks child processes
- operator-visible state is materially wrong or missing
- Rust and Elixir diverge on required issue lifecycle behavior

## Rollback Steps

1. Stop the Rust primary process cleanly.
2. Restore the Elixir primary process with the last known-good workflow and
   environment.
3. Verify Elixir state and refresh routes are healthy.
4. Preserve Rust logs, snapshot payloads, and workflow/config inputs for
   follow-up.
5. Open or update the corresponding parity issue before the next cutover
   attempt.

## Evidence To Preserve

- exact git revision
- workflow path and effective config inputs
- startup logs
- `/api/v1/state` and `/api/v1/refresh` outputs
- issue/session identifiers involved in failures
- diff against the last known-good cutover candidate
