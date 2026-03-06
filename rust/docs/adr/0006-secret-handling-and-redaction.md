# ADR-0006: Secret Handling and Redaction

## Status
Accepted

## Date
2026-03-06

## Context

The Rust runtime consumes tracker credentials and may surface structured error
payloads, rate-limit payloads, and workflow-derived configuration. Those paths
must not leak secrets into logs, dashboards, or serialized snapshots.

Recent live testing also showed that configuration can contain accidental auth
formatting differences, such as a `Bearer ` prefix in a Linear API key. The
system needs a clear policy for normalization and redaction.

## Decision

Adopt the following policy:

1. Secrets may enter the runtime through workflow front matter, env
   indirection, or host environment variables.
2. Concrete adapters may normalize secret formatting for compatibility, but may
   not change the logical credential value exposed to operators.
3. Logs, summaries, dashboard surfaces, and snapshot payloads must redact
   obvious secret-bearing keys and values before serialization.
4. Error payloads may preserve structural context, but must not echo raw API
   keys, bearer tokens, or workflow-secret values.
5. Real integration tests may consume local secrets, but test output and
   checked-in fixtures must never print them.

## Consequences

- Live compatibility fixes can happen without relaxing the redaction bar.
- Operators retain enough context to debug auth failures without credential
  leakage.
- Snapshot and dashboard formatting code must maintain deterministic sanitizing
  behavior.

## Rejected Alternatives

- Pass secrets through all operator surfaces unchanged.
  This is unsafe and incompatible with production diagnostics.
- Forbid all credential normalization in adapters.
  This would let minor formatting differences create avoidable live failures.
