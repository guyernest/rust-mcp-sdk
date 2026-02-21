# Project Research Summary

**Project:** pmcp-tasks (MCP Tasks for PMCP SDK)
**Domain:** Durable async task system integrated into existing Rust protocol SDK
**Researched:** 2026-02-21
**Confidence:** HIGH

## Executive Summary

The `pmcp-tasks` crate adds MCP Tasks support (spec 2025-11-25, experimental) to the existing PMCP Rust workspace. This is a brownfield addition, not a greenfield build: all core dependencies (`tokio`, `serde`, `async-trait`, `thiserror`, `uuid`, `chrono`, `parking_lot`) already exist in the workspace and must be reused at their current versions. The right model is a pluggable durable state machine with two storage backends: an in-memory store for development and a DynamoDB store for serverless production. Task variables are the key PMCP differentiator -- a bidirectional client/server scratchpad via `_meta` that neither the MCP spec, A2A, nor Temporal provide. The architecture is a strict layered crate (types -> store trait -> context/handler -> backends) with exactly two narrow integration points into the core `pmcp` crate: new `ClientRequest` variants and an optional `task_store()` builder method.

The recommended implementation approach is to build from the bottom up, in the dependency order that the architecture enforces: protocol types first, then the `TaskStore` trait, then the in-memory backend, then the handler and middleware integration, and finally the DynamoDB backend and workflow integration. This order means every phase delivers something testable and the most complex integration point (wiring the middleware into `ServerCore`) is deferred until the store layer is fully proven. The design should explicitly account for the Lambda execution model from the start: background tasks spawned with `tokio::spawn` are frozen when Lambda returns a response, so the execution model must either be two-invocation (SQS-backed) for Lambda or `JoinSet`-tracked for long-lived servers -- this is a configuration choice, not an afterthought.

The two highest-risk items are: (1) the experimental nature of the MCP Tasks spec, which has already seen multiple breaking changes and will change again -- the crate must stay on `0.x` semver and maintain a wire type / domain type separation so spec changes do not blow up user-facing APIs; and (2) the `ToolMiddleware` short-circuit problem -- the existing `ToolMiddleware` trait returns `Result<()>` and cannot return a `CreateTaskResult` value. The recommended mitigation is to bypass the middleware chain for the initial implementation (direct check in `handle_call_tool`) and revisit middleware trait extension once the baseline works.

## Key Findings

### Recommended Stack

All dependencies for `pmcp-tasks` are either already in the workspace or are additive with no conflicts. The only new runtime dependencies are `aws-sdk-dynamodb` and `aws-config`, both feature-gated behind `dynamodb`. The critical technical decision is that `async-trait` is required (not optional) because `TaskStore` must be `Arc<dyn TaskStore>` for pluggable backends, and native `async fn in trait` is not yet dyn-safe. One MSRV caution: `parking_lot 0.12.5` and `proptest 1.9` both raised their MSRV to 1.84, but the workspace is on 1.82.0 -- pin `parking_lot` to `<0.12.5` and `proptest` to `1.7` in dev-dependencies.

**Core technologies:**
- `pmcp` (path dep): Parent SDK, protocol types, server traits -- the whole point of the crate
- `async-trait 0.1`: Required for `Arc<dyn TaskStore>` dyn dispatch; native async traits are not dyn-safe
- `parking_lot 0.12`: Fast synchronous RwLock for in-memory store; correct for CPU-bound HashMap operations (not async)
- `aws-sdk-dynamodb 1` + `aws-config 1`: Feature-gated DynamoDB backend; official Rust AWS SDK, actively maintained
- `thiserror 2.0`, `uuid 1.17`, `chrono 0.4`, `tokio 1`, `tracing 0.1`, `serde 1.0`: All already in workspace at compatible versions

**Full `Cargo.toml` for `pmcp-tasks`** is specified in STACK.md with all version pins verified against crates.io.

### Expected Features

The MCP Tasks spec is explicit about MUST requirements. The v0.1.0 MVP must include all spec-required endpoints, capability negotiation, owner binding, security limits, and the PMCP-specific differentiators (task variables + `TaskContext` ergonomic API). Do not ship v0.1.0 without task variables -- they are the core value proposition and are not significantly harder to build than the raw store operations.

**Must have (table stakes for v0.1.0):**
- Core protocol types (`Task`, `TaskStatus`, `CreateTaskResult`, `TaskParams`) with exact spec-compliant serde
- Task status state machine with validated transitions (5 states, ~8 valid transitions)
- `TaskStore` trait (pluggable storage abstraction)
- `InMemoryTaskStore` (dev/test backend -- zero extra dependencies)
- Capability negotiation via `experimental.tasks` in `ServerCapabilities`
- Tool-level task support declaration (`forbidden`/`optional`/`required`)
- Task creation flow: `TaskMiddleware` intercepts `tools/call` with `task` field, returns `CreateTaskResult` immediately, spawns background execution
- All four task endpoints: `tasks/get`, `tasks/result`, `tasks/cancel`, `tasks/list`
- Related-task metadata (`_meta` correlation)
- TTL and resource management (timestamps, periodic cleanup)
- Spec-compliant error codes (`-32602`, `-32603`)
- Owner binding from OAuth sub / client ID / session ID
- Security limits (`TaskSecurityConfig`: max tasks per owner, max TTL)
- Task variables (PMCP extension, bidirectional shared state via `_meta.pmcp:variables`)
- `TaskContext` ergonomic API (`get_variable`, `set_variable`, `require_input`, `complete`, `fail`)
- Basic example (`60_tasks_basic.rs`)

**Should have (competitive, v0.2.0):**
- `DynamoDbTaskStore` with conditional writes, native TTL, GSI pagination
- CloudFormation template for DynamoDB table (integrates with `cargo-pmcp` deploy)
- `SequentialWorkflow` integration (steps read/write task variables for durability)
- Task status notifications (for SSE/Streamable HTTP transports)
- Model immediate response (`_meta["io.modelcontextprotocol/model-immediate-response"]`)
- Progress token continuity

**Defer (v0.3+ / post-spec-stabilization):**
- Move types into core `pmcp` crate (wait for spec to drop experimental status)
- Redis backend
- Task analytics / monitoring hooks

**Anti-features (explicitly excluded):**
- Built-in scheduler/job queue (the SDK is not a runtime)
- Automatic retry with exponential backoff (application concern; retry of non-idempotent ops causes damage)
- Task-to-task dependencies/DAG execution (use `SequentialWorkflow` instead)
- `serde(flatten)` on `TaskWithVariables` (silently eats unknown spec fields)

### Architecture Approach

The architecture is a strictly layered crate with four layers: (1) protocol types, (2) `TaskStore` trait, (3) `TaskContext` and handlers, (4) storage backends. Dependency inversion applies throughout: upper layers only see trait abstractions. Two narrow integration points touch the core `pmcp` crate: new `ClientRequest` enum variants for `tasks/*` methods (unavoidable spec requirement), and a `task_store()` builder method on `ServerCoreBuilder` (additive, non-breaking). Owner enforcement is baked into the store contract -- every operation takes `owner_id` and the store rejects cross-owner access, so security cannot be bypassed by caller error.

**Major components:**
1. **Protocol Types** (`src/types.rs`, `capabilities.rs`, `error.rs`) -- Zero business logic, pure spec representation; wire types separate from domain types
2. **`TaskStore` trait** (`src/store.rs`) -- Defines atomic CRUD + state machine operations; must include `complete_with_result` for atomicity (not separate `update_status` + `set_result` calls)
3. **`TaskContext`** (`src/context.rs`) -- Ergonomic handler-facing API wrapping `Arc<dyn TaskStore>` with a pinned `task_id` + `owner_id`; caches `TaskRecord` within a handler invocation to avoid redundant DynamoDB reads
4. **`TaskHandler`** (`src/handler.rs`) -- Routes `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`; resolves owner from `RequestHandlerExtra`
5. **`TaskMiddleware`** (`src/middleware.rs`) -- Intercepts `tools/call` with `task` field; returns `CreateTaskResult` immediately; spawns background execution via `tokio::task::JoinSet` (long-lived) or enqueues work (Lambda)
6. **`InMemoryTaskStore`** (`src/backends/memory.rs`) -- `HashMap` behind `parking_lot::RwLock`; all locks scoped to synchronous blocks (never held across `.await`)
7. **`DynamoDbTaskStore`** (`src/backends/dynamodb.rs`, feature-gated) -- Conditional writes for atomic state machine, native TTL, GSI for owner-scoped listing; read-time expiration check because DynamoDB TTL deletion lags up to 48h
8. **`TaskSecurityConfig`** (`src/security.rs`) -- Max tasks per owner, max TTL, `allow_anonymous` (defaults to false)

### Critical Pitfalls

1. **Non-atomic `complete()` creates zombie tasks** -- If `update_status(Completed)` succeeds but `set_result()` then fails (crash, timeout), tasks are permanently `Completed` with no result. Clients get `NotReady` on a terminal task forever. Fix: add `complete_with_result()` to the `TaskStore` trait as a single atomic operation. Must be in the trait definition before any backend implements it.

2. **`parking_lot::RwLock` held across `.await` deadlocks under load** -- `parking_lot` guards are `Send`, so the compiler does not warn when guards span `.await` points. Works fine at low concurrency (dev/test), deadlocks under production load. Fix: scope all lock acquisitions to synchronous blocks; enable `#![deny(clippy::await_holding_lock)]` in the crate root.

3. **`serde(flatten)` on `TaskWithVariables` silently eats unknown spec fields** -- When the experimental spec adds new Task fields (expected), they get captured into the `variables` HashMap instead of being preserved as protocol fields, corrupting both namespaces. Fix: keep variables in `_meta.pmcp:variables` on the wire (not flattened at the top level); known serde issue serde-rs/serde#1346.

4. **Background tasks orphaned on Lambda freeze / server shutdown** -- `tokio::spawn` tasks are frozen when Lambda returns a response, leaving tasks stuck in `Working`. `JoinHandle` drop detaches (does not cancel) tasks on server shutdown. Fix: for Lambda, use a two-invocation pattern (SQS or DynamoDB Streams trigger second invocation); for long-lived servers, use `tokio::task::JoinSet` and drain it during graceful shutdown.

5. **DynamoDB conditional write retries silently violate state machine invariants** -- The AWS SDK may retry `ConditionalCheckFailedException` by default. If the condition failed because another writer legitimately changed the state, retrying writes the wrong state. Fix: disable retries for conditional writes; always use `ReturnValuesOnConditionCheckFailure::ALL_OLD` to surface the actual current state in error context.

6. **Spec drift breaks published crate** -- The MCP Tasks spec is experimental and has a history of breaking changes. If types are published at `1.x`, every spec revision requires a semver-major release. Fix: stay on `0.x`; separate wire types from domain types; pin the spec schema version in CI tests.

## Implications for Roadmap

Based on the dependency chain in ARCHITECTURE.md and the pitfall-to-phase mapping in PITFALLS.md, the following phase structure is recommended:

### Phase 1: Foundation Types and Store Contract

**Rationale:** Everything in the system depends on correct spec-compliant types and a well-designed `TaskStore` trait. The trait API cannot change after backends implement it without breaking all implementations. All Phase 1 pitfalls (non-atomic complete, serde flatten, spec drift) must be prevented here because they cannot be safely retrofitted later. This is the lowest-risk phase technically but the highest-leverage design phase.

**Delivers:** Compilable `pmcp-tasks` crate with correct types, `TaskStore` trait (including `complete_with_result`), `TaskRecord`, `TaskError`, and `TaskSecurityConfig`. Full serialization test suite with snapshot tests (`insta`) verifying wire format against MCP spec JSON. Property tests for state machine transition invariants.

**Addresses features:** Core protocol types, task status state machine, `TaskStore` trait, error handling, security limits configuration.

**Avoids:** Non-atomic complete pitfall (trait includes `complete_with_result`), serde flatten pitfall (variables in `_meta`, not flattened), spec drift pitfall (wire/domain type separation, crate versioned `0.1.0`).

**Research flag:** SKIP -- types are directly derived from the MCP 2025-11-25 spec schema; no ambiguity.

### Phase 2: In-Memory Backend and Owner Isolation

**Rationale:** The in-memory backend unblocks all subsequent testing without cloud dependencies. It must be built before the handler/middleware because integration tests require a real backend. Owner isolation must be enforced here at the store level so the security invariant is established before higher-level code assumes it. MSRV-sensitive dependencies (`parking_lot`, `proptest`) must be verified in this phase.

**Delivers:** `InMemoryTaskStore` with full CRUD, atomic state machine transitions, lazy TTL enforcement, owner isolation on all 7 store methods, and a periodic cleanup timer. Property tests for concurrent transition races, owner boundary isolation, and variable merge semantics (including null-deletion behavior). Stress test for 100 concurrent tasks with `#![deny(clippy::await_holding_lock)]` enforced.

**Addresses features:** In-memory storage backend, TTL and resource management, owner binding (in-memory enforcement path).

**Avoids:** `parking_lot` deadlock pitfall (deny lint + synchronous lock scoping), owner isolation gaps (property tests on all 7 methods), variable size limits inconsistency (same configurable limit as DynamoDB).

**Research flag:** SKIP -- `parking_lot::RwLock<HashMap>` is a standard pattern with well-documented requirements.

### Phase 3: Handler, Middleware, and Server Integration

**Rationale:** This is the highest-risk phase because it requires changes to the core `pmcp` crate (`ClientRequest` enum variants) and a design decision on the `ToolMiddleware` short-circuit mechanism. The execution model (Lambda two-invocation vs. long-lived `JoinSet`) must be decided here, not in Phase 4, because it affects the `TaskMiddleware` API surface. The `TaskContext` ergonomic API and owner resolution from `RequestHandlerExtra` are also built here.

**Delivers:** `TaskContext`, `TaskHandler` (all 4 endpoints), `TaskMiddleware` (with configurable execution model), `ServerCoreBuilder` integration via `task_store()` builder method, capability negotiation wired into `ServerCapabilities.experimental.tasks`, owner resolution from `RequestHandlerExtra`, typed `task_context: Option<TaskContext>` field on `RequestHandlerExtra`. Working end-to-end example (`60_tasks_basic.rs`) with in-memory backend.

**Addresses features:** Task creation flow, all four task endpoints (`tasks/get`, `tasks/result`, `tasks/cancel`, `tasks/list`), related-task metadata, capability negotiation, tool-level task support, owner binding (resolution from auth context), `TaskContext` ergonomic API, task variables (PMCP extension).

**Avoids:** Background task orphan pitfall (JoinSet tracking for long-lived servers, two-invocation design documentation for Lambda), `ToolMiddleware` type safety pitfall (direct check in `handle_call_tool` rather than forcing through error path), `RequestHandlerExtra` metadata type safety (typed field, not `HashMap<String, String>`).

**Research flag:** NEEDS RESEARCH -- the `ToolMiddleware::on_request` short-circuit mechanism requires codebase analysis to determine the lowest-risk integration point. The `ClientRequest` enum change is the one unavoidable core-crate modification; needs careful review to minimize blast radius.

### Phase 4: DynamoDB Backend and Production Readiness

**Rationale:** The DynamoDB backend is independent of the handler/middleware (it only depends on the `TaskStore` trait from Phase 1). It can be developed in parallel with Phase 3 by a second engineer once the trait is stable. However, it should not be shipped until Phase 3 is complete and validated, because the DynamoDB backend's conditional write semantics, TTL behavior, and GSI pagination all require integration tests that exercise the full stack.

**Delivers:** `DynamoDbTaskStore` with conditional writes (atomic state machine), native TTL + read-time expiration check, GSI for owner-scoped listing with cursor-based pagination, error mapping from `ConditionalCheckFailedException` to `TaskError::InvalidTransition { current_status, attempted_status }`, CloudFormation template for DynamoDB table. `cargo-pmcp` integration for automated table creation. All DynamoDB tests run separately with `--package pmcp-tasks --features dynamodb` (not `--workspace`) to avoid Cargo feature unification bloat.

**Addresses features:** DynamoDB storage backend (production-ready, serverless), CloudFormation template, variable size limits enforcement.

**Avoids:** DynamoDB TTL stale read pitfall (read-time expiration check in `get()`), DynamoDB conditional write retry pitfall (disabled retries on `ConditionalCheckFailedException`, `ReturnValuesOnConditionCheckFailure::ALL_OLD`), `tasks/list` full table scan (GSI with `limit` parameter, paginated by default), per-variable DynamoDB write amplification (batch `set_variables` method).

**Research flag:** SKIP for DynamoDB patterns (well-documented via AWS docs and PITFALLS.md). NEEDS RESEARCH for `cargo-pmcp` deploy integration (depends on current `cargo-pmcp` extension mechanism).

### Phase 5: Workflow Integration and v0.2.0 Extras

**Rationale:** `SequentialWorkflow` integration depends on both the task system (Phase 3) and the existing workflow module being understood in depth. It is the last feature to build because mistakes here affect both systems. Model immediate response and progress token continuity are low-complexity additions that complete the spec compliance picture. Task status notifications are deferred to this phase because they require transport-layer knowledge (SSE availability).

**Delivers:** `SequentialWorkflow` steps backed by `TaskContext` (step state reads/writes task variables, `input_required` maps to elicitation steps, workflow survives Lambda cold starts), task status notifications on state transitions (transport-dependent), model immediate response field in `CreateTaskResult._meta`, progress token continuity from original request to background execution. Additional examples (workflow, code mode, DynamoDB).

**Addresses features:** `SequentialWorkflow` integration, task status notifications, model immediate response, progress token continuity.

**Research flag:** NEEDS RESEARCH -- the existing `SequentialWorkflow` / `DataSource::StepOutput` binding mechanism needs codebase analysis to determine how task variables should be integrated without breaking existing workflow behavior.

### Phase Ordering Rationale

- **Types and trait first** because every component depends on them and the trait API is impossible to change safely after backends exist.
- **In-memory backend before handler** because handlers need a real store to test against; mocks are not sufficient for integration tests.
- **Handler/middleware before DynamoDB** because the integration complexity is in the server-side wiring, not the storage; validating the full flow with in-memory storage first reduces risk.
- **DynamoDB parallel to handler, sequential to shipping** because the trait is stable after Phase 1, but DynamoDB tests need the full integration working before they're meaningful.
- **Workflow integration last** because it depends on both the new task system and the existing workflow module being stable.

The architecture's build order (ARCHITECTURE.md Phase 1-8) directly maps to this roadmap phase structure. Phases 4a (TaskContext) and 4b (TaskSecurityConfig) in the architecture are collapsed into Phase 3 here for roadmap clarity.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (Handler/Middleware/Server Integration):** The `ToolMiddleware` short-circuit mechanism and the `ClientRequest` enum change require codebase analysis. Run `/gsd:research-phase` before planning this phase. Key questions: what is the minimum change to `ServerCore` to route `tasks/*` methods, and how does the builder register task routes without breaking existing builder users?
- **Phase 5 (Workflow Integration):** The `SequentialWorkflow`/`DataSource::StepOutput` binding mechanism requires codebase analysis of `src/server/workflow/`. Run `/gsd:research-phase` before planning this phase. Key question: how do step outputs currently bind to inputs, and how does `input_required` map to the existing elicitation flow?

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation Types):** Directly spec-derived types. MCP 2025-11-25 schema is the authoritative source.
- **Phase 2 (In-Memory Backend):** `parking_lot::RwLock<HashMap>` is a canonical Rust pattern with well-understood requirements.
- **Phase 4 (DynamoDB Backend):** DynamoDB conditional writes, TTL, and GSI patterns are well-documented; all pitfalls already enumerated.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dependencies verified via crates.io; MSRV concerns explicitly enumerated; `Cargo.toml` ready to copy |
| Features | HIGH | MCP 2025-11-25 spec is authoritative and complete; A2A/Temporal comparison adds useful context; feature boundary between v0.1 and v0.2 is clear |
| Architecture | HIGH | Based on codebase analysis of existing PMCP SDK plus official spec; two integration points with core are identified precisely |
| Pitfalls | HIGH | All pitfalls independently sourced; DynamoDB and async Rust pitfalls are well-documented with cited references; recovery strategies included |

**Overall confidence:** HIGH

### Gaps to Address

- **`ToolMiddleware` short-circuit mechanism:** The exact implementation choice (Option A: `MiddlewareAction` enum, Option B: metadata injection, Option C: direct check in `handle_call_tool`) is deferred to Phase 3 planning. The recommendation is Option C for initial implementation, but this requires codebase review of `ServerCore::handle_call_tool` to confirm it is the right insertion point. Address in Phase 3 research.

- **`parking_lot` MSRV pin:** `parking_lot 0.12.5` requires MSRV 1.84; the workspace is on 1.82.0. The Cargo lockfile may already constrain this. Verify with `cargo check` during Phase 2 setup. Mitigation: pin to `>=0.12.3, <0.12.5` if needed.

- **`mockall 0.14` compatibility:** STACK.md notes the workspace uses `mockall 0.14` but crates.io latest is `0.13.1`. This may be a version tracking error in the research or a workspace-internal fork. Verify during Phase 2 dev-dependency setup.

- **Lambda two-invocation pattern details:** The exact SQS/DynamoDB Streams integration for the Lambda execution model is out of scope for v0.1 but needs to be documented as a deployment pattern before Phase 3 ships. This prevents users from deploying a broken `tokio::spawn` pattern to Lambda unknowingly.

- **Owner isolation with anonymous fallback:** `resolve_owner_id` falls back to `"anonymous"` when no auth context exists. `TaskSecurityConfig::allow_anonymous` defaults to `false` per the pitfalls recommendation, but the builder default needs to be set explicitly and tested. Validate in Phase 3 integration tests.

## Sources

### Primary (HIGH confidence)
- [MCP Tasks Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks) -- Complete task spec; all MUST/SHOULD/MAY requirements
- [Rust Blog: Stabilizing async fn in traits](https://blog.rust-lang.org/inside-rust/2023/05/03/stabilizing-async-fn-in-trait.html) -- Native async traits not dyn-safe; confirms `async-trait` requirement
- [AWS: DynamoDB TTL](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/TTL.html) -- Epoch seconds requirement; 48h deletion lag
- [AWS: DynamoDB Conditional Writes](https://aws.amazon.com/blogs/database/handle-conditional-write-errors-in-high-concurrency-scenarios-with-amazon-dynamodb/) -- `ConditionalCheckFailedException` handling
- [Tokio shared state tutorial](https://tokio.rs/tokio/tutorial/shared-state) -- `parking_lot` vs `tokio::sync` guidance
- Existing PMCP SDK codebase analysis (`traits.rs`, `tool_middleware.rs`, `cancellation.rs`, `workflow/`, `core.rs`, `builder.rs`)
- PMCP Tasks Design Document (`docs/design/tasks-feature-design.md`)

### Secondary (MEDIUM confidence)
- [WorkOS: MCP 2025-11-25 Spec Update](https://workos.com/blog/mcp-2025-11-25-spec-update) -- Ecosystem context, spec history
- [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/) -- Task model comparison
- [Restate Documentation](https://docs.restate.dev/use-cases/async-tasks/) -- Async task patterns, Rust SDK reference
- [Alex DeBrie: Single-Table Design](https://www.alexdebrie.com/posts/dynamodb-single-table/) -- DynamoDB design patterns
- [Alex DeBrie: DynamoDB Condition Expressions](https://www.alexdebrie.com/posts/dynamodb-condition-expressions/) -- Conditional write state machine

### Tertiary (LOW confidence)
- [serde-rs/serde#1346](https://github.com/serde-rs/serde/issues/1346) -- `serde(flatten)` + HashMap interaction (pattern, not fix)
- [Cargo Workspace Feature Unification Pitfall](https://nickb.dev/blog/cargo-workspace-and-the-feature-unification-pitfall/) -- Feature flag isolation for DynamoDB tests

---
*Research completed: 2026-02-21*
*Ready for roadmap: yes*
