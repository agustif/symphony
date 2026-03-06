# Symphony x Coasts Plan

_Last updated: 2026-03-05_

## 1. Purpose

This document defines the full plan for evolving Symphony from a workspace-scoped agent orchestrator into a run-oriented control plane that composes cleanly with Coasts as its local runtime substrate.

The goal is not to bolt `coast` commands onto the current reference implementation. The goal is to redefine the core runtime boundary so that:

- Symphony owns the work graph.
- Coasts owns the local runtime graph.
- A Symphony run becomes a durable, addressable software-change environment with proof, not just a host workspace plus an agent subprocess.

## 2. Executive Summary

Today Symphony is explicitly scoped around:

- issue intake and orchestration
- per-issue host workspaces
- a locally spawned coding-agent session in the workspace
- retry, reconciliation, and basic observability

This is codified in:

- [`SPEC.md`](./SPEC.md)
- [`rust/docs/architecture/overview.md`](./rust/docs/architecture/overview.md)
- [`rust/crates/symphony-runtime/src/worker.rs`](./rust/crates/symphony-runtime/src/worker.rs)

Today Coasts is explicitly scoped around:

- build artifacts for local development environments
- coast instance lifecycle
- worktree assignment
- canonical vs dynamic ports
- runtime storage topology
- live logs, exec, and service inspection

This is codified in:

- [`Coastfile`](./Coastfile)
- [`coasts/README.md`](./coasts/README.md)
- [`coasts/docs/`](./coasts/docs)

The combined system should therefore be modeled as:

- Symphony = run controller
- coastd = runtime controller
- agent = implementation actor
- proof bundle = merge gate artifact

The key architectural change is to replace `workspace + subprocess` as the execution primitive with a `Run Capsule`:

```text
Run Capsule
= issue identity
+ attempt identity
+ worktree ref
+ runtime build ref
+ runtime lease
+ agent session
+ proof bundle
+ optional focus lease
```

## 3. Current State Assessment

### 3.1 Symphony Today

The current spec and implementations assume a host-owned execution model:

- Symphony creates a workspace directory under `workspace.root`.
- Hooks provision that workspace on the host.
- Symphony spawns the coding agent as a local child process.
- The app-server protocol is anchored to an absolute host `cwd`.
- Cleanup removes host directories directly.

Primary references:

- [`SPEC.md`](./SPEC.md)
- [`elixir/WORKFLOW.md`](./elixir/WORKFLOW.md)
- [`rust/crates/symphony-workspace/src/lifecycle.rs`](./rust/crates/symphony-workspace/src/lifecycle.rs)
- [`rust/crates/symphony-runtime/src/lib.rs`](./rust/crates/symphony-runtime/src/lib.rs)
- [`rust/crates/symphony-runtime/src/worker.rs`](./rust/crates/symphony-runtime/src/worker.rs)
- [`rust/crates/symphony-agent-protocol/src/startup_payload.rs`](./rust/crates/symphony-agent-protocol/src/startup_payload.rs)

The Rust redesign already has two strong advantages:

- a reducer-first control plane
- explicit adapter boundaries

That means the control-plane core is compatible with a deeper runtime integration if we change the execution model cleanly.

### 3.2 Coasts Today

Coasts already provides most of the runtime behavior Symphony is missing:

- build once, run many
- isolated local runtimes per project instance
- worktree assignment without full container rebuild
- canonical and dynamic port routing
- storage topology choices: shared services, shared volumes, isolated volumes
- runtime exec, logs, service status, and event surfaces
- a daemon protocol behind a thin CLI

Primary references:

- [`coasts/README.md`](./coasts/README.md)
- [`coasts/coast-core/src/protocol/mod.rs`](./coasts/coast-core/src/protocol/mod.rs)
- [`coasts/coast-daemon/src/server.rs`](./coasts/coast-daemon/src/server.rs)
- [`coasts/coast-cli/src/commands/mod.rs`](./coasts/coast-cli/src/commands/mod.rs)

The root repo also already includes a local Coast setup:

- [`Coastfile`](./Coastfile)

This is currently manual. It is not yet part of the Symphony runtime model.

### 3.3 What Is Missing

Neither project alone currently models the full combined primitive:

- Symphony has no first-class runtime lease, runtime build, or focus concept.
- Coasts has no issue tracker, retry, proof policy, or work-hand-off model.
- Symphony observability does not yet model runtime health, runtime topology, or proof artifacts.
- Coasts observability does not know what a run attempt, retry, or issue lifecycle means.

## 4. Design Principles

1. One controller per concern.
   Symphony should not reimplement coastd, and coastd should not absorb tracker orchestration.

2. Reducer authority stays in Symphony.
   Run state transitions, retries, and handoff remain reducer-governed.

3. Runtime truth stays in Coasts.
   Build, assign, checkout, ports, service status, and volume topology remain runtime-provider concerns.

4. Host agent by default.
   The default execution model is a host-side coding agent using Coasts for runtime-heavy operations.

5. Worktree, not clone, is the long-term code view.
   Current `git clone` workspace bootstrapping is a transitional implementation detail.

6. Live environment and proof share the same boundary.
   Validation must run against the same environment that can be debugged mid-flight.

7. CLI-first integration, protocol-second integration.
   The first usable version should integrate against the `coast` CLI. A later version should use the daemon protocol directly.

8. `WORKFLOW.md` and `Coastfile` must stay orthogonal.
   Workflow policy belongs to Symphony. Runtime topology belongs to Coasts.

## 5. Target Architecture

### 5.1 Responsibilities

| Layer | Owner | Responsibility |
| --- | --- | --- |
| Work policy | Symphony | tracker polling, issue eligibility, retries, proof gates, handoff |
| Runtime policy | Coasts | builds, instances, worktree assignment, ports, volumes, secrets |
| Session policy | Symphony | prompt rendering, session lifecycle, continuation rules |
| Runtime execution | Coasts | command exec, logs, service inspection, focus/checkout |
| Proof execution | Symphony using Coasts | ordered validation steps against a runtime lease |
| Operator UX | federated | Symphony run view plus linked runtime view |

### 5.2 New Canonical Runtime Object

Introduce `Run Capsule` as the canonical unit of execution:

```text
RunCapsule {
  run_id,
  issue_id,
  issue_identifier,
  attempt,
  project_root,
  worktree_ref,
  runtime_provider,
  runtime_build_ref,
  runtime_lease_ref,
  agent_session_ref,
  proof_bundle_ref,
  focus_lease_ref?,
  created_at,
  updated_at
}
```

### 5.3 New Supporting Types

Add the following first-class concepts to the spec and Rust domain model:

- `RunId`
- `AttemptId`
- `WorktreeRef`
- `RuntimeBuildRef`
- `RuntimeLeaseRef`
- `RuntimeProviderKind`
- `RuntimeView`
- `FocusLeaseRef`
- `ProofStep`
- `ProofBundleRef`
- `ProofStatus`
- `SessionRef`
- `RunArtifactRef`

### 5.4 Runtime Lifecycle

The desired run lifecycle becomes:

1. issue becomes eligible
2. Symphony claims issue
3. Symphony ensures worktree
4. Symphony ensures runtime build
5. Symphony acquires runtime lease
6. Symphony assigns runtime lease to worktree
7. Symphony starts or resumes agent session
8. Symphony executes proof steps against runtime lease
9. Symphony hands off, retries, or releases resources

This must be reducer-driven, not adapter-driven.

## 6. Configuration Model

### 6.1 `WORKFLOW.md`

`WORKFLOW.md` continues to define:

- tracker policy
- issue-state map
- prompt template
- proof requirements
- handoff rules
- retry and concurrency policy
- optional provider selection

It should stop being the place where host shell bootstrap does all runtime setup.

### 6.2 `Coastfile`

`Coastfile` defines:

- project runtime topology
- services and ports
- shared vs isolated storage
- secrets and injections
- worktree directory policy
- optional runtime setup packages

It should not contain tracker logic, proof policy, or handoff semantics.

### 6.3 New Bridge Config

Add a new runtime section to the Symphony config model:

```yaml
runtime:
  provider: coast
  mode: cli
  project_root: ~/code/my-repo
  coastfile_type: default
  build_policy: ensure
  lease_policy: dedicated
  focus_policy: explicit
  proof_exec_mode: runtime
  retention_policy: preserve_on_failure
```

The initial fields should stay minimal. The first version should not attempt to encode every Coast option into Symphony config.

## 7. Integration Strategy

### 7.1 Stage 1: CLI Adapter

Build a `CoastsRuntimeProvider` using the `coast` CLI.

Why:

- fastest path to a working system
- no need to vendor internal daemon contracts into Symphony immediately
- matches current operational usage
- keeps failure surface understandable

Capabilities to wrap:

- `coast build`
- `coast run`
- `coast assign`
- `coast unassign`
- `coast exec`
- `coast logs`
- `coast ps`
- `coast ports`
- `coast checkout`
- `coast rm`

Limitations:

- poorer structured error taxonomy
- weaker event streaming
- parsing CLI output where typed responses would be better

### 7.2 Stage 2: Daemon Protocol Adapter

Move to direct daemon IPC using the Coasts protocol definitions.

Why:

- typed request and response model already exists in [`coasts/coast-core/src/protocol/mod.rs`](./coasts/coast-core/src/protocol/mod.rs)
- better streaming semantics
- lower process-spawn overhead
- better mapping to Symphony observability

Risks:

- protocol stability is not yet a cross-repo compatibility contract
- direct crate dependency across repos may create version skew

Decision:

- do not start here
- design for this upgrade path from day one

### 7.3 Stage 3: Provider-Neutral Runtime Interface

Symphony should define its own provider traits and adapt Coasts into them.

Do not make Symphony depend semantically on Coasts internals.

## 8. Rust Implementation Plan

### 8.1 New Trait Boundaries

Add a new crate:

- `rust/crates/symphony-runtime-provider`

Initial traits:

```rust
pub trait WorkspaceProvider {
    async fn ensure(&self, spec: &WorkspaceSpec) -> Result<WorkspaceRef, WorkspaceError>;
    async fn cleanup(&self, workspace: &WorkspaceRef) -> Result<(), WorkspaceError>;
}

pub trait RuntimeProvider {
    async fn ensure_build(&self, spec: &RuntimeBuildSpec) -> Result<RuntimeBuildRef, RuntimeError>;
    async fn acquire_lease(&self, spec: &RuntimeLeaseSpec) -> Result<RuntimeLeaseRef, RuntimeError>;
    async fn assign(&self, lease: &RuntimeLeaseRef, worktree: &WorktreeRef) -> Result<RuntimeView, RuntimeError>;
    async fn exec(&self, lease: &RuntimeLeaseRef, spec: &ExecSpec) -> Result<ExecResult, RuntimeError>;
    async fn inspect(&self, lease: &RuntimeLeaseRef) -> Result<RuntimeView, RuntimeError>;
    async fn focus(&self, lease: &RuntimeLeaseRef) -> Result<FocusLeaseRef, RuntimeError>;
    async fn release_focus(&self, focus: &FocusLeaseRef) -> Result<(), RuntimeError>;
    async fn release_lease(&self, lease: RuntimeLeaseRef) -> Result<(), RuntimeError>;
}

pub trait SessionProvider {
    async fn start(&self, spec: &SessionSpec) -> Result<SessionRef, SessionError>;
    async fn stop(&self, session: &SessionRef) -> Result<(), SessionError>;
}
```

### 8.2 Crate-by-Crate Changes

| Crate | Changes |
| --- | --- |
| `symphony-domain` | add run/runtime/proof domain types and reducer commands/events |
| `symphony-runtime` | replace direct worker-centric execution with provider-driven run lifecycle |
| `symphony-workspace` | support worktree refs in addition to directory creation; host directory mode remains as compatibility backend |
| `symphony-config` | add `runtime` config section and de-bias `codex`-specific naming where required |
| `symphony-observability` | extend snapshots with runtime lease, ports, proof status, runtime health |
| `symphony-http` | expose run/runtime/proof views |
| `symphony-cli` | support project-root and runtime-provider startup model |
| `symphony-agent-protocol` | remain session-protocol-focused, not runtime-focused |
| new `symphony-runtime-provider` | provider traits, request/response types, shared runtime errors |
| new `symphony-runtime-provider-coast-cli` | initial Coasts adapter over CLI |
| later `symphony-runtime-provider-coastd` | daemon protocol adapter |

### 8.3 Worker Model Refactor

Current `WorkerLauncher` is too narrow. It only models:

- prepare workspace
- run hooks
- launch session
- return `WorkerOutcome`

This must be refactored into:

- worktree provisioning
- runtime provisioning
- session control
- proof execution

`WorkerLauncher` should either be:

- removed in favor of `RunExecutor`, or
- retained as the session subcomponent under a broader runtime model

Recommendation:

- keep `WorkerLauncher` only as a compatibility shim during migration
- introduce `RunExecutor` as the real long-term abstraction

### 8.4 Domain Command Expansion

Replace the current minimal command set:

- `Dispatch`
- `ScheduleRetry`
- `ReleaseClaim`

with a richer internal command model:

- `EnsureWorkspace`
- `EnsureRuntimeBuild`
- `AcquireRuntimeLease`
- `AssignRuntime`
- `StartSession`
- `RunProofStep`
- `AcquireFocus`
- `ReleaseFocus`
- `ReleaseRunResources`
- `PublishArtifact`
- `TransitionRejected`

Not all of these must ship in one reducer revision. The migration should be phased.

## 9. Spec Changes Required

### 9.1 Scope

The spec must change from:

- `Execution Layer` = workspace + agent subprocess

to:

- `Workspace Layer`
- `Runtime Layer`
- `Session Layer`
- `Proof Layer`

### 9.2 New Domain Entities

Add:

- `RuntimeBuild`
- `RuntimeLease`
- `ProofBundle`
- `FocusLease`
- `RunArtifact`
- `Worktree`

### 9.3 New Invariants

Add invariants such as:

- a run with an active session must have a valid workspace ref
- a proof step executed in runtime mode must reference a live runtime lease
- a focus lease may only be held by one run at a time
- release of a run must clear session, retry, and runtime-lease bindings
- proof bundle lineage must include exact run attempt and runtime build

### 9.4 New Failure Classes

Add failure classes:

- runtime build failure
- runtime lease acquisition failure
- runtime assign failure
- runtime exec failure
- runtime health degradation
- focus acquisition failure
- proof artifact publication failure

## 10. Phased Delivery Plan

### Phase 0: Baseline Documentation and Guardrails

Objective:

- align repo docs and local tooling around the target direction without changing core behavior

Deliverables:

- this `PLAN.md`
- spec delta outline
- architecture note updates in `PROJECT_ATLAS.md`
- explicit declaration that root `Coastfile` is transitional manual support, not yet orchestration-owned

Exit criteria:

- plan accepted
- target architecture agreed

### Phase 1: Runtime Boundary Extraction

Objective:

- isolate runtime concerns behind traits without yet integrating Coasts

Deliverables:

- `symphony-runtime-provider` crate
- `HostRuntimeProvider` compatibility implementation
- `RunExecutor` abstraction
- no behavior change for current tests

Exit criteria:

- current Rust runtime still passes with host execution backend
- runtime-provider seams exist

### Phase 2: Worktree Realignment

Objective:

- move from clone-per-workspace assumptions toward project-root plus worktree refs

Deliverables:

- worktree-aware workspace provider
- migration path for old `workspace.root` semantics
- updated startup config model for project-root mode

Exit criteria:

- Symphony can target a single project root and derive per-run worktree refs

### Phase 3: Coasts CLI Provider

Objective:

- make Symphony actually capable of provisioning and using Coasts at runtime

Deliverables:

- `symphony-runtime-provider-coast-cli`
- command wrappers for build, run, assign, exec, ports, ps, logs, checkout, rm
- structured error mapping
- runtime observability projection

Exit criteria:

- one Symphony run can acquire a Coast lease and execute proof commands inside it
- failure preserves the environment for inspection

### Phase 4: Proof Bundle Model

Objective:

- formalize proof as a first-class output instead of ad hoc workpad text

Deliverables:

- proof step model in config
- proof bundle schema
- artifact capture from runtime exec
- run summary linking workpad, proof, runtime view, and issue

Exit criteria:

- a run produces structured evidence tied to exact runtime/build/session refs

### Phase 5: Live Run Operations

Objective:

- expose the real value of Coasts inside Symphony

Deliverables:

- runtime view in observability API
- port exposure in run status
- optional focus acquisition workflow
- explicit preserve-on-failure retention behavior

Exit criteria:

- operators can inspect a live run and switch focus intentionally

### Phase 6: Coasts Daemon Protocol Integration

Objective:

- replace fragile CLI orchestration with typed daemon IPC

Deliverables:

- `symphony-runtime-provider-coastd`
- typed request/response mapping
- progress streaming where useful
- lower-latency richer status integration

Exit criteria:

- Symphony no longer needs to shell out to the `coast` binary for core runtime operations

### Phase 7: Combined Product Surface

Objective:

- turn the composed system into a coherent product and not just an internal integration

Deliverables:

- explicit run/runtime/proof UX
- repository templates for `WORKFLOW.md` plus `Coastfile`
- policy docs
- reference walkthrough

Exit criteria:

- users can understand the system as one product with two controllers

## 11. Proof Model

The combined system must make proof a first-class concept.

Initial proof bundle contents:

- run identity
- issue identity
- attempt number
- workspace/worktree ref
- runtime provider kind
- runtime build ref
- runtime lease ref
- commands executed
- exit codes
- stdout/stderr captures
- test reports
- port map at proof time
- service health summary
- commit SHA and diff summary
- timestamps

Optional later additions:

- screenshots
- traces
- walkthrough video
- CI results
- PR review state

## 12. Observability Plan

Extend `RuntimeSnapshot` into a richer run-focused view.

Add fields:

- `provider_kind`
- `runs_active`
- `proofs_running`
- `runtime_builds_active`
- `focus_owner`
- `runtime_health_summary`
- `exec_failures`
- `ports_exposed`
- `proof_pass_rate`

Add per-run view:

- run state
- session state
- runtime lease state
- dynamic ports
- canonical focus status
- latest proof step
- artifact links

## 13. Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Coasts protocol drift | adapter churn | start with CLI adapter; isolate provider contract |
| Overcoupling to Coasts internals | portability loss | Symphony owns its own runtime-provider traits |
| Keeping clone-based workspace semantics too long | architectural confusion | explicitly deprecate clone-first mode once worktree mode is stable |
| Mixing workflow and runtime concerns | config sprawl | keep `WORKFLOW.md` and `Coastfile` orthogonal |
| Agent shell temptation | auth/lifecycle complexity | host-agent default policy in docs and code |
| Proof bundle becomes afterthought | trust gap | make proof schema and persistence part of early phases |
| Resource explosion from live environments | laptop instability | concurrency caps, retention policy, optional shared-service modes |

## 14. Non-Goals

The first implementation is not trying to:

- build a hosted cloud runtime product
- replace Coastguard
- fully abstract every possible runtime backend at once
- ship containerized agent shells as the default model
- solve remote fleet scheduling in the first milestone

## 15. Migration and Compatibility

During migration, Symphony must support two execution modes:

1. legacy host mode
2. Coasts-backed local runtime mode

Compatibility rules:

- existing host-only workflows must continue to run
- host mode remains the testable baseline until Coasts mode is stable
- the reducer should not special-case provider internals
- `WORKFLOW.md` files that rely on clone-time shell bootstrap remain valid temporarily

Deprecation path:

- deprecate clone-first `after_create` bootstrap once worktree mode is stable
- deprecate runtime setup in hooks once runtime-provider setup is authoritative

## 16. Immediate Next Steps

1. Accept this plan as the root architecture document.
2. Draft the `SPEC.md` delta for runtime, worktree, and proof concepts.
3. Add `symphony-runtime-provider` with a host compatibility backend.
4. Refactor `symphony-runtime` away from direct `WorkerLauncher` ownership of the whole execution story.
5. Implement `symphony-runtime-provider-coast-cli` against the local `coast` binary.
6. Add run/runtime observability fields before building a richer UI.

## 17. Acceptance Criteria for This Plan

This plan is complete when it is good enough to drive implementation without re-litigating the core architecture.

That means:

- the boundary between Symphony and Coasts is explicit
- the migration path is staged and realistic
- the Rust crate landing zones are identified
- the spec delta is clear
- runtime, session, and proof are separated conceptually
- compatibility with existing host mode is preserved during migration

## 18. Final Position

The correct composed system is:

- not Symphony with some runtime hooks
- not Coasts with some issue-tracker glue
- but a new Symphony scope where a run is a durable software-change capsule

Symphony should become the reducer-governed controller of these capsules.
Coasts should remain the local runtime engine that realizes them.
