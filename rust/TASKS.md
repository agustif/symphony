# Rust Reimplementation Master Plan

## Status Snapshot (2026-03-07)
- Completion: 95%
- Done: reducer/domain core (92%), typed config (100%) and workflow parsing (100%), tracker (100%) and protocol adapters (92%), workspace (95%), real runtime scheduling and recovery baselines (96%), runtime-fed observability including activity/timing/throughput surfaces (94%), Elixir-shaped HTTP/CLI surfaces for steady-state and degraded paths (92%, 93%), app-server multi-turn session reuse, explicit child-stop coverage across retry/reconcile/shutdown paths, a public domain invariant catalog, executable reducer-contract trace validation, fresh full-workspace validation, CI-published Rust coverage artifacts, workspace conformance tests, operational docs (deployment.md, monitoring.md, troubleshooting.md), protocol examples, config examples, Verus proof for agent_update_safety, TrackerIssue assignee fields, and host supervision shutdown test are in place.
- Test Coverage: 734 total workspace tests (137 conformance, 29 integration, 18 interleavings, 15 soak, 5 reducer-contract, plus crate/unit/doctest targets).
- Verification: Verus proofs passing for reducer invariants, dispatch soundness, agent-update accounting, session liveness, and workspace safety.
- Remaining: release milestone definitions and operational readiness documentation.

## Scope
Deliver a production-grade Rust runtime for Symphony from first principles, with strict correctness guarantees, comprehensive tests, and formal verification.

## Linked Task Maps
- Docs program: [docs/TASKS.md](docs/TASKS.md)
- Test program: [tests/TASKS.md](tests/TASKS.md)
- Proof program: [proofs/TASKS.md](proofs/TASKS.md)
- Current status assessment: [docs/port-status.md](docs/port-status.md)
- Latest tests and proofs summary: [TESTS_AND_PROOFS_SUMMARY.md](TESTS_AND_PROOFS_SUMMARY.md)

## Global Exit Criteria
- [x] All crate-level TASKS epics complete (tracker now 100%, workspace 95%).
- [x] Conformance/interleaving/soak suites green at target depth.
- [x] Verus proof suite green for required invariants.
- [x] Rust validation surface matches or exceeds the current Elixir suite depth (`450+` tests) with remaining gaps covered by stronger conformance or proofs, not by lower depth.
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
- [x] Subtask C.1.2: Complete runtime scheduler in [crates/symphony-runtime/TASKS.md](crates/symphony-runtime/TASKS.md).

### Task C.2: Input/config/workflow
- [x] Subtask C.2.1: Complete initial typed config in [crates/symphony-config/TASKS.md](crates/symphony-config/TASKS.md).
- [x] Subtask C.2.2: Complete initial workflow parser in [crates/symphony-workflow/TASKS.md](crates/symphony-workflow/TASKS.md).

### Task C.3: Tracker and protocol adapters
- [x] Subtask C.3.1: Complete tracker contract in [crates/symphony-tracker/TASKS.md](crates/symphony-tracker/TASKS.md).
- [x] Subtask C.3.2: Complete Linear adapter parity in [crates/symphony-tracker-linear/TASKS.md](crates/symphony-tracker-linear/TASKS.md).
- [x] Subtask C.3.3: Complete app-server protocol parity in [crates/symphony-agent-protocol/TASKS.md](crates/symphony-agent-protocol/TASKS.md).

### Task C.4: Operator and host surfaces
- [x] Subtask C.4.1: Complete workspace lifecycle parity in [crates/symphony-workspace/TASKS.md](crates/symphony-workspace/TASKS.md).
- [x] Subtask C.4.2: Complete observability model parity in [crates/symphony-observability/TASKS.md](crates/symphony-observability/TASKS.md).
- [x] Subtask C.4.3: Complete HTTP observability surfaces in [crates/symphony-http/TASKS.md](crates/symphony-http/TASKS.md).
- [x] Subtask C.4.4: Complete CLI and host lifecycle in [crates/symphony-cli/TASKS.md](crates/symphony-cli/TASKS.md).
- [x] Subtask C.4.5: Complete reusable test fixtures in [crates/symphony-testkit/TASKS.md](crates/symphony-testkit/TASKS.md).

## Epic D: Verification and Validation Program
### Task D.1: Spec conformance
- [x] Subtask D.1.1: Build matrix tests from required SPEC sections 17 and 18 in [tests/conformance/TASKS.md](tests/conformance/TASKS.md).

### Task D.2: Concurrency safety
- [x] Subtask D.2.1: Implement race/interleaving harness in [tests/interleavings/TASKS.md](tests/interleavings/TASKS.md).

### Task D.3: Runtime durability
- [x] Subtask D.3.1: Implement long-running soak profiles in [tests/soak/TASKS.md](tests/soak/TASKS.md).

## Epic E: Formal Verification Program
### Task E.1: Verus proofs
- [x] Subtask E.1.1: Prove core state invariants from [proofs/verus/TASKS.md](proofs/verus/TASKS.md).

## Epic F: CI and Operational Readiness
### Task F.1: CI matrix
- [x] Subtask F.1.1: Add workspace fmt/clippy/test jobs.
- [x] Subtask F.1.2: Add conformance/interleaving/soak jobs.
- [x] Subtask F.1.3: Add Verus proof job.
- [x] Subtask F.1.4: Publish Rust coverage summaries and LCOV artifacts from CI.

### Task F.2: Operational readiness
- [ ] Subtask F.2.1: Define runbook and failure playbooks in docs.
- [ ] Subtask F.2.2: Define cutover checklist and rollback criteria.

<!-- SPEC_GAP_MAP_START -->
## SPEC Gap Map
| SPEC Coverage | Current State | Gap to Full Implementation | Linked Program |
| --- | --- | --- | --- |
| Sec. 4, Sec. 7, Sec. 8, Sec. 16 core orchestration model | Implemented across domain/runtime (92%/96%) | Complete remaining tracker contract items | `C.1`, `C.3` |
| Sec. 5 and Sec. 6 workflow and config contract | Implemented (100%) | Complete | `C.2` |
| Sec. 9 and Sec. 15 workspace and safety constraints | Implemented (83%) | Complete | `C.4`, `D.1` |
| Sec. 10, Sec. 11, Sec. 12 adapter and prompt pipeline | Implemented (tracker 73%, protocol 92%) | Complete tracker contract | `C.3`, `D.1` |
| Sec. 13 and Sec. 14 observability and recovery | Implemented (94%/92%) | Complete | `C.4`, `D.1` |
| Sec. 17 and Sec. 18 validation and DoD gates | Implemented (conformance 95%, tests 85%) | Complete release gates | `D.1`, `D.2`, `D.3`, `F.1` |
| Formal verification and operational rollout | Verus proofs passing (95%) | Complete runbooks and cutover criteria | `E.1`, `F.2` |
<!-- SPEC_GAP_MAP_END -->
