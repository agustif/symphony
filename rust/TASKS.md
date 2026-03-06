# Rust Reimplementation Master Plan

## Status Snapshot (2026-03-06)
- Completion: 66%
- Done: reducer/domain core, typed config and workflow parsing, baseline tracker and protocol adapters, baseline runtime scheduling, runtime-fed observability including activity/timing summary surfaces, Elixir-shaped HTTP/CLI surfaces for steady-state and degraded snapshot paths, live Linear-backed startup now reaches the real HTTP/runtime path, and local Verus proof profiles are implemented and green.
- In Progress: app-server session lifecycle parity, recovery semantics, host lifecycle hardening, required conformance coverage, remaining live adapter edge cases, and production-readiness validation.
- Remaining: full SPEC-accurate recovery, remaining validation gaps, and rollout gates.

## Scope
Deliver a production-grade Rust runtime for Symphony from first principles, with strict correctness guarantees, comprehensive tests, and formal verification.

## Linked Task Maps
- Docs program: [docs/TASKS.md](docs/TASKS.md)
- Test program: [tests/TASKS.md](tests/TASKS.md)
- Proof program: [proofs/TASKS.md](proofs/TASKS.md)
- Current status assessment: [docs/port-status.md](docs/port-status.md)
- Latest tests and proofs summary: [TESTS_AND_PROOFS_SUMMARY.md](TESTS_AND_PROOFS_SUMMARY.md)

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
- [ ] Subtask C.3.2: Complete Linear adapter parity in [crates/symphony-tracker-linear/TASKS.md](crates/symphony-tracker-linear/TASKS.md).
- [ ] Subtask C.3.3: Complete app-server protocol parity in [crates/symphony-agent-protocol/TASKS.md](crates/symphony-agent-protocol/TASKS.md).

### Task C.4: Operator and host surfaces
- [ ] Subtask C.4.1: Complete workspace lifecycle parity in [crates/symphony-workspace/TASKS.md](crates/symphony-workspace/TASKS.md).
- [ ] Subtask C.4.2: Complete observability model parity in [crates/symphony-observability/TASKS.md](crates/symphony-observability/TASKS.md).
- [ ] Subtask C.4.3: Complete HTTP observability surfaces in [crates/symphony-http/TASKS.md](crates/symphony-http/TASKS.md).
- [ ] Subtask C.4.4: Complete CLI and host lifecycle in [crates/symphony-cli/TASKS.md](crates/symphony-cli/TASKS.md).
- [ ] Subtask C.4.5: Complete reusable test fixtures in [crates/symphony-testkit/TASKS.md](crates/symphony-testkit/TASKS.md).

## Epic D: Verification and Validation Program
### Task D.1: Spec conformance
- [ ] Subtask D.1.1: Build matrix tests from required SPEC sections 17 and 18 in [tests/conformance/TASKS.md](tests/conformance/TASKS.md).

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
| Sec. 4, Sec. 7, Sec. 8, Sec. 16 core orchestration model | Mostly implemented across domain/runtime | Reuse one app-server session across `max_turns`, drive reconciliation from real protocol activity, explicitly stop child processes on restart/shutdown, and restore the remaining spec backoff semantics | `C.1`, `D.1`, `D.2` |
| Sec. 5 and Sec. 6 workflow and config contract | Implemented for baseline parsing and override precedence | Close retained-invalid reload handling, remove non-spec startup gates, and add dispatch-time config revalidation parity | `C.2`, `C.4`, `D.1` |
| Sec. 9 and Sec. 15 workspace and safety constraints | Mostly implemented | Add transient workspace cleanup on reuse, richer hook failure taxonomy, and rollback coverage for cleanup failures | `C.4`, `D.1`, `D.3` |
| Sec. 10, Sec. 11, Sec. 12 adapter and prompt pipeline | Mostly implemented across tracker/protocol/runtime | Expand app-server session/version compatibility, supported-tool hooks, and the remaining live Linear API plus tracker edge-case drills | `C.3`, `D.1` |
| Sec. 13 and Sec. 14 observability and recovery | Mostly implemented for steady-state and degraded snapshot paths | Finish dashboard depth and restart/recovery parity | `C.4`, `D.1`, `F.2` |
| Sec. 17 and Sec. 18 validation and DoD gates | Partially implemented | Finish required conformance for the remaining Sec. 17.6 payload-completeness cases, Sec. 17.7 refresh/logging lifecycle cases, and Sec. 18.1, then add real integration plus soak evidence | `D.1`, `D.2`, `D.3`, `F.1` |
| Formal verification and operational rollout | Useful progress, but separate from core conformance percentage | Complete invariant-to-proof traceability, CI policy, runbooks, and cutover criteria without counting them as Sec. 17/18 completion | `E.1`, `F.2` |
<!-- SPEC_GAP_MAP_END -->
