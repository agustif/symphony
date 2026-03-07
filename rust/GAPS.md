# Rust Implementation Gap Analysis vs SPEC.md and Elixir Tests

## Summary

**Status: Core conformance is strong; proof and release-policy gaps remain** ⚠️

After systematic analysis comparing the Rust implementation against:
1. **SPEC.md Section 17** (Test and Validation Matrix) - the authoritative conformance requirements
2. **SPEC.md Section 18** (Implementation Checklist) - definition of done criteria
3. **Elixir test suite** (11 test files in `/Users/af/symphony/elixir/test/symphony_elixir/`) - reference implementation coverage

The Rust implementation is **close to full Core Conformance**, but the proof program and release gates still have bounded, explicit follow-up work.

## SPEC Section 17 Conformance Matrix

### 17.1 Workflow and Config Parsing

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Workflow path precedence (explicit > cwd default) | ✅ | `cli_cases.rs` workflow path tests |
| Workflow file changes detected and trigger re-read | ✅ | `main.rs:765-906` workflow reload tests |
| Invalid reload keeps last good config | ✅ | `main.rs:1673,1885-1945` retained last good |
| Missing `WORKFLOW.md` returns typed error | ✅ | `startup_config.rs:95` |
| Invalid YAML front matter returns typed error | ✅ | `loader.rs` parse errors |
| Front matter non-map returns typed error | ✅ | `loader.rs` |
| Config defaults apply when optional values missing | ✅ | `model.rs` defaults |
| `tracker.kind` validation enforces `linear` | ✅ | `validate.rs` |
| `tracker.api_key` works with `$VAR` indirection | ✅ | `env.rs:26-76` `$VAR` resolution |
| `$VAR` resolution for tracker API key and paths | ✅ | `env.rs:105-135` tests |
| `~` path expansion works | ✅ | `env.rs:32-53,135` home expansion |
| `codex.command` preserved as shell command string | ✅ | `model.rs` |
| Per-state concurrency map normalizes state names | ✅ | `validate.rs:53-74` |
| Prompt template renders `issue` and `attempt` | ✅ | `worker.rs:263-275` |
| Prompt rendering fails on unknown variables (strict mode) | ✅ | `worker.rs:252` `UndefinedBehavior::Strict` |

**Gaps Found**: None

---

### 17.2 Workspace Manager and Safety

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Deterministic workspace path per issue identifier | ✅ | `lifecycle.rs:104-115` |
| Missing workspace directory is created | ✅ | `lifecycle.rs:118-145` |
| Existing workspace directory is reused | ✅ | `lifecycle.rs:196-210` |
| Non-directory path handled safely | ✅ | `lifecycle.rs:599-614` test |
| Optional workspace population errors surfaced | ✅ | `lifecycle.rs` hooks |
| Temporary artifacts (`.elixir_ls`, `tmp`) removed | ✅ | `lifecycle.rs:13` `TRANSIENT_WORKSPACE_ENTRIES` |
| `after_create` hook runs only on new workspace | ✅ | `lifecycle.rs:617-636` test |
| `before_run` hook runs before each attempt, failure aborts | ✅ | `lifecycle.rs:650-715`, `worker.rs:189` |
| `after_run` hook runs after each attempt, failure logged/ignored | ✅ | `worker.rs:233-244` best-effort |
| `before_remove` hook runs on cleanup, failures ignored | ✅ | `lifecycle.rs:759-787`, `orchestrator_cases.rs:707-765` |
| Workspace path sanitization invariants enforced | ✅ | `proofs/verus/specs/workspace_safety.rs` |
| Agent launch uses per-issue workspace as cwd | ✅ | `worker.rs:331` |
| Root containment invariants before agent launch | ✅ | `lifecycle.rs:317-328` symlink check |

**Gaps Found**: None

---

### 17.3 Issue Tracker Client

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Candidate fetch uses active states and project slug | ✅ | `lib.rs:213-240` Linear implementation |
| Linear query uses `slugId` filter | ✅ | `lib.rs:216` |
| Empty `fetch_issues_by_states([])` returns empty without API call | ✅ | `tracker_client.rs:98` |
| Pagination preserves order across pages | ✅ | `linear_adapter.rs:564,1034,1083,1121` |
| Blockers normalized from inverse relations of type `blocks` | ✅ | `lib.rs:428-443` normalize_blockers |
| Labels normalized to lowercase | ✅ | `lib.rs:419` to_ascii_lowercase |
| Issue state refresh by ID returns minimal normalized issues | ✅ | `lib.rs:370-382` |
| State refresh uses GraphQL ID typing `[ID!]` | ✅ | `lib.rs:563` |
| Error mapping for request errors, non-200, GraphQL errors | ✅ | `lib.rs:297-303` |

**Gaps Found**: None

---

### 17.4 Orchestrator Dispatch, Reconciliation, and Retry

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Dispatch sort order: priority then oldest creation time | ✅ | `lib.rs:2271-2334` priority dispatch test |
| `Todo` issue with non-terminal blockers not eligible | ✅ | `lib.rs:2517-2543` test |
| `Todo` issue with terminal blockers is eligible | ✅ | `lib.rs:2546-2570` test |
| Active-state issue refresh updates running entry state | ✅ | `lib.rs` reconciliation tests |
| Non-active state stops agent without workspace cleanup | ✅ | `orchestrator_cases.rs` |
| Terminal state stops agent and cleans workspace | ✅ | `orchestrator_cases.rs` |
| Reconciliation with no running issues is no-op | ✅ | `lib.rs` |
| Normal worker exit schedules continuation retry (attempt 1) | ✅ | `lib.rs:2603-2620` |
| Abnormal exit increments retries with exponential backoff | ✅ | `lib.rs` backoff tests |
| Retry backoff cap uses `agent.max_retry_backoff_ms` | ✅ | `lib.rs:2618` |
| Retry queue entries include attempt, due time, identifier, error | ✅ | `domain.rs` RetryEntry |
| Stall detection kills stalled sessions and schedules retry | ✅ | `lib.rs:2335-2383` tests |
| Slot exhaustion requeues retries with explicit error | ✅ | `lib.rs` slot tests |
| Snapshot API returns running/retry rows, token totals, rate limits | ✅ | `state_snapshot.rs` |
| Snapshot API timeout/unavailable cases surfaced | ✅ | `main.rs:1476,1501`, `state_snapshot.rs:574` |

**Gaps Found**: None

---

### 17.5 Coding-Agent App-Server Client

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Launch command uses workspace cwd with `bash -lc` | ✅ | `worker.rs:331,978` |
| Startup handshake sends initialize/initialized/thread/start/turn/start | ✅ | `worker.rs` protocol sequence |
| `initialize` includes client identity/capabilities | ✅ | `startup_payload.rs` |
| Policy payloads use documented approval/sandbox settings | ✅ | `startup_payload.rs:95-96,131-132` |
| `thread/start` and `turn/start` parse nested IDs | ✅ | `worker.rs` |
| Request/response read timeout enforced | ✅ | `worker.rs` timeout handling |
| Turn timeout enforced | ✅ | `protocol_cases.rs` |
| Partial JSON lines buffered until newline | ✅ | `protocol_cases.rs` |
| Stdout/stderr handled separately | ✅ | `worker.rs:332-334` |
| Non-JSON stderr logged but doesn't crash parsing | ✅ | `protocol_cases.rs` |
| Approvals handled per documented policy | ✅ | `event_policy.rs` |
| Unsupported dynamic tool calls rejected without stalling | ✅ | `worker.rs:1195-1206` |
| User input requests handled per policy (hard fail) | ✅ | `worker.rs:2204-2256` tests |
| Usage/rate-limit payloads extracted | ✅ | `state_snapshot.rs` |
| Compatible payload variants accepted | ✅ | `payloads.rs` variant handling |
| Client-side tools advertised during handshake | ✅ | `worker.rs:971` linear_graphql |
| `linear_graphql` tool advertised | ✅ | `worker.rs:1930` |
| Valid `query`/`variables` execute against Linear auth | ✅ | `worker.rs:1250-1292` |
| GraphQL `errors` produce `success=false` preserving body | ✅ | `worker.rs:1312-1318` |
| Invalid arguments, missing auth, transport failures return structured failures | ✅ | `worker.rs` |
| Unsupported tool names fail without stalling | ✅ | `worker.rs` |
| Multi-operation GraphQL documents rejected | ✅ | `worker.rs:1242,1374-1395,1971-1978` |

**Gaps Found**: None

---

### 17.6 Observability

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Validation failures are operator-visible | ✅ | `main.rs` validation logging |
| Structured logging includes issue/session context | ✅ | `worker.rs:241-245` |
| Logging sink failures do not crash orchestration | ✅ | Error handling in logging layer |
| Token/rate-limit aggregation correct across updates | ✅ | `state_snapshot.rs` token aggregation |
| Human-readable status surface driven from state | ✅ | `state_handlers.rs` dashboard |
| Humanized event summaries cover key event classes | ✅ | `payloads.rs` event summarization |

**Gaps Found**: None

---

### 17.7 CLI and Host Lifecycle

| Requirement | Status | Evidence |
|-------------|--------|----------|
| CLI accepts optional positional workflow path | ✅ | `cli_cases.rs` |
| CLI uses `./WORKFLOW.md` when no path provided | ✅ | `cli_cases.rs` |
| CLI errors on nonexistent explicit workflow path | ✅ | `cli_cases.rs` |
| CLI surfaces startup failure cleanly | ✅ | `cli_cases.rs` |
| CLI exits success on normal start/shutdown | ✅ | `cli_cases.rs` |
| CLI exits nonzero on startup failure | ✅ | `cli_cases.rs` |

**Gaps Found**: None

---

## SPEC Section 18 Conformance Matrix

### 18.1 Required for Conformance

| Requirement | Status |
|-------------|--------|
| Workflow path selection (explicit + cwd default) | ✅ |
| `WORKFLOW.md` loader with YAML + prompt split | ✅ |
| Typed config layer with defaults and `$` resolution | ✅ |
| Dynamic `WORKFLOW.md` watch/reload/re-apply | ✅ |
| Polling orchestrator with single-authority mutable state | ✅ |
| Issue tracker client (candidate fetch + state refresh + terminal fetch) | ✅ |
| Workspace manager with sanitized per-issue workspaces | ✅ |
| Workspace lifecycle hooks (all four) | ✅ |
| Hook timeout config | ✅ |
| App-server subprocess client with JSON line protocol | ✅ |
| Codex launch command config | ✅ |
| Strict prompt rendering with issue/attempt variables | ✅ |
| Exponential retry queue with continuation retries | ✅ |
| Configurable retry backoff cap | ✅ |
| Reconciliation stops runs on terminal/non-active states | ✅ |
| Workspace cleanup for terminal issues | ✅ |
| Structured logs with issue_id/identifier/session_id | ✅ |
| Operator-visible observability | ✅ |

**Result**: 18/18 requirements met ✅

---

## Behavioral Divergences from Elixir (Acceptable)

The following divergences exist but are acceptable design choices:

### 1. Multi-operation GraphQL Rejection

- **Elixir**: Passes multi-operation documents to Linear API, relies on Linear to reject with "Must provide operation name if query contains multiple operations."
- **Rust**: Pre-rejects at client level with "query must contain exactly one GraphQL operation"
- **SPEC Alignment**: SPEC 17.5 says "If the provided document contains multiple operations, reject the tool call as invalid input" - Rust behavior is SPEC-compliant

---

## Formal Verification

The Rust implementation includes Verus proofs for:

- Reducer invariants and command taxonomy: `proofs/verus/specs/runtime_quick.rs`
- Multi-step reducer chains and slot accounting: `proofs/verus/specs/runtime_full.rs`
- Agent-update accounting and topology preservation: `proofs/verus/specs/agent_update_safety.rs`
- One-step dispatch/retry/release progress obligations: `proofs/verus/specs/session_liveness.rs`
- Workspace safety invariants: `proofs/verus/specs/workspace_safety.rs`

The remaining proof gaps are narrower:

- stronger multi-step fairness/starvation-freedom models
- broader runtime-assertion traceability for each proof artifact
- deeper workspace canonicalization modeling beyond the current string-level abstraction

---

## Test Coverage Summary

| Area | Test Files | Key Tests |
|------|------------|-----------|
| Runtime | `lib.rs` | ~3000 lines of tests |
| Protocol | `protocol_cases.rs` | ~472 lines |
| CLI | `cli_cases.rs` | ~888 lines |
| Workspace | `lifecycle.rs` | ~800 lines |
| Linear Tracker | `linear_adapter.rs` | ~1200 lines |
| Observability | `observability_cases.rs` | ~420 lines |
| Orchestrator | `orchestrator_cases.rs` | ~1039 lines |

---

## Conclusion

**The Rust implementation is close to SPEC.md Core Conformance for Sections 17.1-17.7 and 18.1, with the remaining gaps concentrated in proof depth and release-gate policy rather than missing runtime features.**

The implementation includes:
- Comprehensive test coverage matching SPEC requirements
- Formal verification proofs for reducer invariants, agent-update accounting, session liveness, and workspace safety
- All required features: hooks, pagination, blocker/label normalization, stall detection, retry backoff, snapshot API, etc.

**Date**: 2026-03-07
