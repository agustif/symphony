# Rust Reimplementation Master Plan

## Status Snapshot (2026-03-05)
- Completion: 82%
- Done: parallel implementation batch merged across domain/runtime, config/workflow/workspace, tracker/linear, protocol/http/cli, docs, CI/proofs, test scaffolds, concrete Verus proof module rewrites, proof CI installation/guard wiring, explicit suite-gate CI jobs, completed conformance matrix coverage, runtime startup terminal cleanup, graceful shutdown hardening, CLI `--port`/`--logs-root` wiring, typed protocol policy mapping for approval/input-required/timeout plus normalized startup/runtime error categories, protocol startup payload builders with optional tool advertisement support, nested protocol payload extractors for session IDs/usage/tool-call metadata, live runtime handshake integration, buffered handshake stream preservation, unsupported dynamic tool-call fallback responses that keep sessions alive, concrete `linear_graphql` tool execution path, and serialization-safe domain rejection/violation diagnostic payloads.
- In Progress: tracker normalization parity, live-integration hardening, and formal proofs.
- Remaining: full production-grade completion gates from conformance through rollout.

## Scope
Deliver a production-grade Rust runtime for Symphony from first principles, with strict correctness guarantees, comprehensive tests, and formal verification.

## Linked Task Maps
- Docs program: [docs/TASKS.md](docs/TASKS.md)
- Test program: [tests/TASKS.md](tests/TASKS.md)
- Proof program: [proofs/TASKS.md](proofs/TASKS.md)

## Global Exit Criteria
- [ ] All crate-level TASKS epics complete.
- [ ] Conformance/interleaving/soak suites green at target depth.
- [ ] Verus proof suite green for required invariants.
- [ ] Rust runtime validated as primary orchestrator in production-like runs.

## Epic A: Program Management and Delivery
### Task A.1: Execution model and ownership
- [x] Subtask A.1.1: Keep this file as the authoritative dependency graph.
- [x] Subtask A.1.2: Assign one crate owner per active branch/agent batch.
- [x] Subtask A.1.3: Enforce no duplicated task definitions across child TASKS files.

### Task A.2: Release slicing
- [ ] Subtask A.2.1: Define `alpha`, `beta`, and `ga` milestones.
- [ ] Subtask A.2.2: Gate each milestone on explicit suite subsets from [tests/TASKS.md](tests/TASKS.md).
- [ ] Subtask A.2.3: Gate `ga` on [proofs/verus/TASKS.md](proofs/verus/TASKS.md).

## Epic B: Documentation Program
### Task B.1: Architecture docs completeness
- [x] Subtask B.1.1: Complete system architecture narrative in [docs/architecture/TASKS.md](docs/architecture/TASKS.md).
- [x] Subtask B.1.2: Keep ADR decisions current in [docs/adr/TASKS.md](docs/adr/TASKS.md).

## Epic C: Crate Implementation Program
### Task C.1: Core orchestration logic
- [x] Subtask C.1.1: Complete initial reducer invariants in [crates/symphony-domain/TASKS.md](crates/symphony-domain/TASKS.md).
- [ ] Subtask C.1.2: Complete runtime scheduler in [crates/symphony-runtime/TASKS.md](crates/symphony-runtime/TASKS.md).

### Task C.2: Input/config/workflow
- [x] Subtask C.2.1: Complete initial typed config in [crates/symphony-config/TASKS.md](crates/symphony-config/TASKS.md).
- [x] Subtask C.2.2: Complete initial workflow parser in [crates/symphony-workflow/TASKS.md](crates/symphony-workflow/TASKS.md).

### Task C.3: Tracker and protocol adapters
- [x] Subtask C.3.1: Complete initial tracker contract in [crates/symphony-tracker/TASKS.md](crates/symphony-tracker/TASKS.md).
- [x] Subtask C.3.2: Complete initial Linear adapter in [crates/symphony-tracker-linear/TASKS.md](crates/symphony-tracker-linear/TASKS.md).
- [x] Subtask C.3.3: Complete initial app-server protocol in [crates/symphony-agent-protocol/TASKS.md](crates/symphony-agent-protocol/TASKS.md).

### Task C.4: Operator and host surfaces
- [x] Subtask C.4.1: Complete initial workspace lifecycle in [crates/symphony-workspace/TASKS.md](crates/symphony-workspace/TASKS.md).
- [x] Subtask C.4.2: Complete initial observability model in [crates/symphony-observability/TASKS.md](crates/symphony-observability/TASKS.md).
- [x] Subtask C.4.3: Complete initial HTTP surfaces in [crates/symphony-http/TASKS.md](crates/symphony-http/TASKS.md).
- [x] Subtask C.4.4: Complete initial CLI entrypoint in [crates/symphony-cli/TASKS.md](crates/symphony-cli/TASKS.md).
- [x] Subtask C.4.5: Complete initial reusable test fixtures in [crates/symphony-testkit/TASKS.md](crates/symphony-testkit/TASKS.md).

## Epic D: Verification and Validation Program
### Task D.1: Spec conformance
- [x] Subtask D.1.1: Build matrix tests from SPEC sections 17/18 in [tests/conformance/TASKS.md](tests/conformance/TASKS.md).

### Task D.2: Concurrency safety
- [ ] Subtask D.2.1: Implement race/interleaving harness in [tests/interleavings/TASKS.md](tests/interleavings/TASKS.md).

### Task D.3: Runtime durability
- [ ] Subtask D.3.1: Implement long-running soak profiles in [tests/soak/TASKS.md](tests/soak/TASKS.md).

## Epic E: Formal Verification Program
### Task E.1: Verus proofs
- [ ] Subtask E.1.1: Prove core state invariants from [proofs/verus/TASKS.md](proofs/verus/TASKS.md).

## Epic F: CI and Operational Readiness
### Task F.1: CI matrix
- [x] Subtask F.1.1: Add workspace fmt/clippy/test jobs.
- [x] Subtask F.1.2: Add conformance/interleaving/soak jobs.
- [x] Subtask F.1.3: Add Verus proof job.

### Task F.2: Operational readiness
- [ ] Subtask F.2.1: Define runbook and failure playbooks in docs.
- [ ] Subtask F.2.2: Define cutover checklist and rollback criteria.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Program |
| --- | --- | --- | --- |
| Sec. 4, Sec. 7, Sec. 8, Sec. 16 core orchestration model | Implemented across domain/runtime | Complete edge-case parity for every transition trigger and recovery branch | `C.1`, `D.1`, `D.2` |
| Sec. 5 and Sec. 6 workflow and config contract | Implemented in workflow/config crates with port override precedence coverage | Close live-run reload drift and exhaustive override matrix coverage under fault scenarios | `C.2`, `D.1`, `F.2` |
| Sec. 9 and Sec. 15 workspace and safety constraints | Implemented with path containment and hooks | Add real-integration fault drills for hook failures and filesystem anomalies | `C.4`, `D.3` |
| Sec. 10, Sec. 11, Sec. 12 adapter and prompt pipeline | Mostly implemented across protocol/tracker/runtime with typed policy mapping, startup payload builders, handshake wiring, nested extractor compatibility, unsupported tool-call fallback responses, and concrete `linear_graphql` handler support | Complete full tracker normalization parity and expand live compatibility coverage for tool/session variants | `C.3`, `D.1` |
| Sec. 13, Sec. 14 observability and recovery | Partially implemented with optional HTTP host wiring | Finish dashboard parity, failure playbooks, and restart recovery contract docs/tests | `C.4`, `F.2` |
| Sec. 17 and Sec. 18 validation and DoD gates | Conformance matrix is green; full workspace tests are green | Expand interleaving/soak depth to target profiles and make branch-protection requirements explicit | `D.2`, `D.3`, `F.1` |
<!-- SPEC_GAP_MAP_END -->
