# Project Research Summary

**Project:** Task-Prompt Bridge (PMCP SDK v1.1)
**Domain:** Protocol SDK — task-aware workflow prompts with partial execution and client continuation
**Researched:** 2026-02-21
**Confidence:** HIGH

## Executive Summary

The task-prompt bridge is an architectural integration milestone, not a technology problem. Every capability required already exists in the `pmcp` and `pmcp-tasks` crates — the work is entirely about wiring `WorkflowPromptHandler` to optionally create a `TaskContext` at execution start, persist step progress into task variables as it runs, and return structured handoff guidance in the `GetPromptResult` reply. No new crate dependencies are needed. The existing `serde_json::Value` type handles all serialization, the existing `TaskStore` trait handles all persistence, and the existing break-on-unresolvable pattern in `WorkflowPromptHandler::handle()` already implements partial execution implicitly.

The recommended approach is a composed `TaskWorkflowPromptHandler` that delegates step execution to the existing handler's internals but owns the task lifecycle and partial execution decisions. The original `WorkflowPromptHandler` must remain completely unchanged — it is the contract for all existing workflow registrations and any modification risks silent regressions in LLM-facing conversation trace formatting. The bridge is activated opt-in: `SequentialWorkflow::with_task_support(true)` plus injecting an `Arc<dyn TaskRouter>` into the server builder. Workflows that do not opt in execute identically to v1.0.

The primary risks are architectural, not technical: (1) creeping changes to `WorkflowPromptHandler` that break existing workflows, (2) dual-write inconsistency between the in-memory `ExecutionContext` and durable task variables if store writes fail silently, and (3) a variable schema that drifts into an unversioned implicit API between the execution engine and the prompt reply generator. All three are preventable by design decisions made in Phase 1 before any execution code is written. The structured handoff reply must include both a machine-readable `_meta` block (for smart clients) and natural language guidance (for any LLM), because MCP is client-agnostic and a structured-only format cannot be assumed parseable by all LLM clients.

## Key Findings

### Recommended Stack

No new dependencies. The entire bridge is achievable with the existing dependency graph. `serde` and `serde_json` handle step state serialization. `async-trait` supports the bridge trait that keeps crate dependencies one-directional. The `TaskRouter` trait in `pmcp` core uses `serde_json::Value` as its interface to `pmcp-tasks` implementations — this is the established v1.0 pattern that prevents circular dependencies (`pmcp-tasks` depends on `pmcp`, never the reverse).

**Core technologies:**
- `serde_json::Value`: Step state serialization — already used in `ExecutionContext`; handles all workflow variable storage without schema validation libraries
- `async-trait 0.1`: Bridge trait pattern — required for `WorkflowTaskBridge` async methods across crate boundaries; already a dependency
- `TaskRouter` trait (pmcp core): Cross-crate bridge — existing pattern from v1.0; extended with 3 new methods that have default error implementations (non-breaking to all existing implementors)
- `TaskStore` / `InMemoryTaskStore` (pmcp-tasks): Persistence — zero changes; all required operations (`create`, `set_variables`, `complete_with_result`) already exist
- `WorkflowPromptHandler` (pmcp core): Integration point — gains `Option<Arc<dyn TaskRouter>>` field; no changes to existing behavior when field is `None`

See `.planning/research/STACK.md` for the complete integration dependency graph and alternatives considered.

### Expected Features

The bridge introduces a new interaction pattern in the MCP ecosystem: a prompt that creates a durable task, executes resolvable steps server-side, and returns explicit machine-readable instructions for client continuation. No other MCP SDK has this. The closest analogue is A2A's multi-turn task model, but PMCP's approach uses MCP-native primitives (prompts + tasks) and provides more prescriptive continuation guidance. See `.planning/research/FEATURES.md` for the full competitor analysis.

**Must have (table stakes — v1.1.0 launch):**
- Task-aware prompt handler — `WorkflowPromptHandler` creates a task when invoked with `task_support` enabled; returns task ID in `GetPromptResult._meta`
- Partial server-side execution with persistence — steps that execute write bindings to task variables; step loop behavior unchanged (break on unresolvable), but progress is now durable
- Step state tracking via `_workflow.*` schema — standard flat-key schema: `wf.goal`, `wf.total_steps`, `wf.steps.{n}.status`, `wf.steps.{n}.result_summary`
- Structured handoff message — final assistant message in `GetPromptResult` includes task ID, completed step summaries, next step guidance with tool name and argument sources, remaining step list
- Task status lifecycle integration — task stays `working` at handoff; transitions to `completed` only when all steps finish server-side; `failed` on unrecoverable error
- Automatic step result persistence — dual-write on each step: in-memory `ExecutionContext` (for same-request chaining) AND task variables (for client continuation durability)
- Example `62_tasks_workflow.rs` — 4-step workflow, 2 server-executed steps, handoff, simulated client continuation, final task polling

**Should have (v1.1.x — add after core bridge validation):**
- `DataSource::TaskVariable` variant — cross-session data flow where client-completed steps feed subsequent server steps
- `StepExecution` enum on `WorkflowStep` — explicit `ServerSide`/`ClientSide`/`BestEffort` declaration per step; replaces implicit inference from guidance presence
- Workflow resume from task state — `WorkflowPromptHandler::resume(task_id, extra)` reconstructs execution context from task variables

**Defer (v1.2+):**
- Nested workflow invocation (sub-workflow as a step)
- Conditional step branching (let LLM reasoning handle this; DAG execution massively increases complexity)
- Progress notifications per step (wait for SSE transport adoption)
- Client-side SDK helpers for continuation

### Architecture Approach

The bridge connects two previously disconnected paths in the server: `prompts/get` (which ran all steps synchronously and returned a full trace) and `tasks/*` (which tracked long-running `tools/call` operations). The connection goes through 3 new methods on the existing `TaskRouter` trait in `pmcp` core. Non-breaking default implementations return errors, so existing `TaskRouterImpl` compiles unchanged until the new methods are implemented. `WorkflowPromptHandler` gains an optional `task_router` field; when present and the workflow opts in via `with_task_support(true)`, it creates a task on `prompts/get` entry, syncs step results after each completion, and embeds structured handoff guidance in the reply. When absent, behavior is identical to v1.0.

See `.planning/research/ARCHITECTURE.md` for before/after diagrams, exact file locations, and the complete build order with LOC estimates per phase.

**Major components:**
1. `TaskRouter` trait extension (src/server/tasks.rs) — 3 new methods: `create_workflow_task`, `set_task_variables`, `complete_workflow_task`; default error impls keep this non-breaking
2. `WorkflowPromptHandler` modified (src/server/workflow/prompt_handler.rs) — gains `task_router: Option<Arc<dyn TaskRouter>>`; modified `handle()` creates task, syncs bindings, builds structured reply
3. `TaskRouterImpl` extended (crates/pmcp-tasks/src/router.rs) — implements the 3 new methods delegating to existing `TaskStore` operations
4. `SequentialWorkflow` extended (src/server/workflow/sequential.rs) — gains `task_support: bool` field with builder method
5. `ServerCoreBuilder` wired (src/server/builder.rs) — `prompt_workflow()` passes `task_router.clone()` to handler when configured (~10 LOC change)
6. `WorkflowProgress` struct (new, in pmcp-tasks) — typed variable schema with `schema_version: u32`; all `wf.*` variable keys go through this struct, never as string literals in production code

### Critical Pitfalls

1. **Modifying `WorkflowPromptHandler` breaks existing workflows** — Create a separate `TaskWorkflowPromptHandler` that composes with the original via delegation, not modification. The original handler file must have zero diff. If existing workflow tests require modification to pass, the isolation boundary has been breached. This is the most important architectural decision in the milestone and must be made before writing any code.

2. **Dual-write inconsistency between `ExecutionContext` and task variables** — Write to durable task variables FIRST, then populate in-memory `ExecutionContext`. If the durable write fails, treat the step as failed and stop execution. Never proceed to the next step with ephemeral-only data. The natural coding instinct (execute → bind in memory → persist) is the wrong order.

3. **Variable schema becomes an unversioned implicit API** — Define a `WorkflowProgress` struct with all `wf.*` keys as typed fields and a `schema_version: u32`. No string literal variable keys in production code outside the schema module. A single key name drift between the execution engine and the prompt reply generator causes silent data loss with no compile-time warning.

4. **Prompt reply too rigid for different LLM clients** — Use a hybrid approach: structured JSON in `_meta` (machine-readable, for smart clients) AND natural language in the final assistant message (for any LLM). MCP is client-agnostic. Testing only with Claude Code is insufficient — the format must also work as plain text for smaller models.

5. **Client continuation tools disconnected from workflow task state** — Plain `tools/call` requests have no built-in workflow context. Embed the task ID as a `_task_id` argument in remaining step guidance; implement `WorkflowStepMiddleware` to intercept tool calls with this argument, update workflow progress variables, and advance `current_step_index`. Without this, the task variables become stale after the first client-side tool call.

See `.planning/research/PITFALLS.md` for 8 critical pitfalls, technical debt patterns, security mistakes, and a "looks done but isn't" checklist.

## Implications for Roadmap

Based on research, suggested phase structure (6 phases):

### Phase 1: Foundation — Schema, Trait Extension, Step Execution Mode

**Rationale:** Three design decisions must be locked before any execution code is written: (a) the `WorkflowProgress` typed variable schema with versioning, (b) the `TaskRouter` trait extension with default error impls, and (c) the `StepExecution` enum on `WorkflowStep`. These are the contracts that all downstream phases depend on. Changing any of them after Phase 2 is implemented cascades through every file that touches task variables or the execution loop. This is also the phase that establishes the "separate handler" isolation boundary — proving that `WorkflowPromptHandler` will remain unchanged.

**Delivers:** Compilable, non-breaking `TaskRouter` trait extension. `WorkflowProgress` struct with typed fields and `schema_version`. `StepExecution` enum (`ServerSide`, `ClientSide`, `BestEffort`) on `WorkflowStep`. All existing workflow tests pass without modification.

**Addresses features:** Step state tracking schema foundation, step execution mode declaration, `DataSource::TaskVariable` groundwork.

**Avoids pitfalls:** Variable schema as implicit API (Pitfall 4), pause logic becoming second engine (Pitfall 5), handler modification breaking existing workflows (Pitfall 1).

### Phase 2: Partial Execution Engine — Task Creation and Step Sync

**Rationale:** The core execution loop changes happen here. `WorkflowPromptHandler` (or `TaskWorkflowPromptHandler`) gains the task lifecycle: create on entry, sync each step result to task variables using durable-first write order, collect `StepSummary` structs for completed and remaining steps. This is the highest-complexity change in the milestone and must be isolated from the reply format changes in Phase 3. Extended `SequentialWorkflow::validate_for_partial_execution()` is also built here — it must exist before the execution loop can safely run.

**Delivers:** Working partial execution with durable task variable persistence. Tasks created and queryable via `tasks/get`. Dual-write semantics with failure isolation (durable-first). Extended validation for partial execution dependencies. Step failure produces valid `StepSummary` with `retryable` flag, not a terminal task error.

**Addresses features:** Task-aware prompt handler, partial execution with persistence, automatic step result persistence, task status lifecycle integration.

**Avoids pitfalls:** Dual-write inconsistency (Pitfall 2), error during partial execution leaving inconsistent state (Pitfall 6), validation not accounting for partial execution dependencies (Pitfall 8).

### Phase 3: Structured Prompt Reply and Client Continuation

**Rationale:** The handoff message format and client continuation mechanism can only be designed after Phase 2 proves what data is available in task variables. The reply format depends on how continuation works — specifically, whether remaining step guidance uses `_task_id` as a tool argument (recommended) or task-augmented calls (not broadly supported). This phase also implements `WorkflowStepMiddleware` for reconnecting plain `tools/call` requests back to workflow task state, and adds security enforcement for `_workflow.*` prefix write protection.

**Delivers:** Hybrid prompt reply with `_meta` (machine-readable JSON) and natural language guidance. `WorkflowStepMiddleware` for `_task_id` propagation. Step ordering enforcement (out-of-order tool calls rejected with clear error). Security: client writes to `_workflow.*` prefixed variables rejected.

**Addresses features:** Structured handoff message, client continuation protocol, task status lifecycle (task stays `working` at handoff).

**Avoids pitfalls:** Prompt reply too rigid for LLMs (Pitfall 3), client continuation disconnected from task state (Pitfall 7), workflow progress variables writable by client (security).

### Phase 4: TaskRouterImpl Implementation (pmcp-tasks crate)

**Rationale:** This phase provides the concrete implementation of the 3 new `TaskRouter` methods in `pmcp-tasks`. It is deliberately separated from Phase 1 (which adds the trait interface) to allow Phases 2 and 3 to work against default error implementations in integration testing. The implementation is straightforward delegation to existing `TaskStore` operations — the complexity is in the interface design (Phase 1), not the implementation.

**Delivers:** `TaskRouterImpl::create_workflow_task`, `set_task_variables`, `complete_workflow_task` fully implemented and tested. Full integration with `InMemoryTaskStore`. Follows existing pattern of `handle_task_call` delegation for DynamoDB compatibility.

**Addresses stack:** Completes the integration dependency graph. Validates that the `Value`-based interface translates cleanly to typed `TaskStore` operations.

**Avoids architecture anti-pattern:** `TaskStore` never imported into `pmcp` core. All access goes through the `TaskRouter` trait boundary.

### Phase 5: ServerCoreBuilder Wiring and Example

**Rationale:** The plumbing phase. `ServerCoreBuilder.prompt_workflow()` passes the configured task router to the new handler. The `62_tasks_workflow.rs` example demonstrates the full bridge end-to-end and serves as the integration test bed for all prior phases. This phase is intentionally last for the non-extension work — it locks in the user-facing API after all internal contracts are proven.

**Delivers:** Zero-boilerplate server configuration for task-backed workflows. Working example: 4-step workflow, 2 server-executed steps, structured handoff, simulated client continuation, final `tasks/get` polling showing completed variables. Graceful degradation when no task router is configured.

**Addresses features:** Example `62_tasks_workflow.rs` (P1 required). `SequentialWorkflow.with_task_support()` builder API.

**Avoids pitfalls:** "Looks done but isn't" checklist — backward compatibility, workflow-specific TTL configuration, graceful degradation with no task store.

### Phase 6: Post-Validation Extensions (v1.1.x)

**Rationale:** These features depend on Phases 2-3 being validated in practice before building. `DataSource::TaskVariable` requires step results already in task variables (Phase 2) and the continuation mechanism working (Phase 3). Workflow resume is the most complex feature and should only be built once the simpler partial execution is stable — it involves reconstructing `ExecutionContext` from stored variables across session boundaries, with edge cases for client-completed steps, schema migration, and partial variable sets.

**Delivers:** `DataSource::TaskVariable { key, field }` variant for cross-session data flow. `StepExecution` enum user-facing builder API on `WorkflowStep`. Workflow resume API (`WorkflowPromptHandler::resume(task_id, extra)`).

**Addresses features:** DataSource::TaskVariable (P2), guidance-aware step classification (P2), workflow resume (P2).

### Phase Ordering Rationale

- **Schema and trait before execution:** The `WorkflowProgress` schema and `TaskRouter` trait extension are contracts that all other phases import. Changing them after Phase 2 is written requires cascading changes across every file that touches task variables.
- **Execution before reply format:** The structured reply is built from what the execution engine produces. Designing the reply format before knowing what data the execution loop provides is speculative and usually wrong.
- **`pmcp` interface before `pmcp-tasks` implementation:** The one-directional dependency (`pmcp-tasks` → `pmcp`) means the trait must be defined first. `TaskRouterImpl` can be developed in Phase 4 once the trait is stable.
- **Builder wiring last among core changes:** The simplest change (~10 LOC). Doing it last prevents pre-wiring code that might lock in design assumptions before the handler API is finalized.
- **v1.1.x features gated on validation:** `DataSource::TaskVariable` and workflow resume are HIGH complexity with HIGH user value. Building them before the simpler bridge is validated in production adds risk with low marginal benefit at launch.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (Structured Reply + Client Continuation):** The `_task_id` argument propagation mechanism via `WorkflowStepMiddleware` is novel — no prior art in MCP ecosystem. The middleware design needs careful specification before implementation to avoid coupling tool call semantics to workflow context. Run `/gsd:research-phase` before Phase 3 planning.
- **Phase 6 (Workflow Resume):** Resume from task state involves reconstructing `ExecutionContext` across session boundaries. Edge cases (steps completed by client, schema migration between workflow versions, partial variable sets from crashed sessions) need detailed design before coding begins. Run `/gsd:research-phase` before Phase 6 planning.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Schema + Trait Extension):** Adding default-impl methods to a Rust trait is a well-documented evolution pattern (RFC 1105). The variable schema struct is standard `serde` serialization with a version field.
- **Phase 4 (TaskRouterImpl):** Follows the exact pattern of the existing `handle_task_call` implementation in `TaskRouterImpl`. Copy the pattern, adapt for workflow context.
- **Phase 5 (Builder Wiring):** Cloning an `Arc` in a builder method and passing it to a handler constructor. Already done for `with_task_store()`.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Based on direct codebase analysis of all affected files. No external dependencies to add. All required operations confirmed in existing types (`TaskStore`, `TaskRouter`, `serde_json::Value`). |
| Features | MEDIUM | MCP spec defines tasks and prompts as independent primitives. The bridge semantics are PMCP innovation, not spec-mandated. A2A and Temporal provide validated analogues for partial execution and structured handoff, but the specific interaction between `prompts/get` and task creation is novel. Feature prioritization is well-grounded; long-term feature set (v1.2+) is speculative. |
| Architecture | HIGH | Based entirely on direct codebase analysis. All component boundaries, file locations, and integration points confirmed by reading source. The `TaskRouter` trait bridge pattern is a validated v1.0 architectural decision with a working reference implementation. |
| Pitfalls | HIGH | Critical pitfalls derived from codebase analysis (existing handler structure, type constraints, dual-write semantics) combined with domain research on state machine serialization, Rust API evolution, and LLM prompt structuring. All 8 pitfalls have concrete "how to avoid" guidance grounded in existing code patterns. |

**Overall confidence:** HIGH

### Gaps to Address

- **`GetPromptResult._meta` field existence:** PITFALLS.md assumes `GetPromptResult` already has `_meta: Option<Map<String, Value>>` ("the existing `_meta` field is sufficient"). This needs verification against the actual type definition before Phase 3 design. If `_meta` is absent, adding it may require a protocol type change that has broader SDK impact.

- **`WorkflowStepMiddleware` design:** The mechanism for connecting a plain `tools/call` carrying `_task_id` back to workflow task state is conceptually described in PITFALLS.md (Pitfall 7) but not fully specified in ARCHITECTURE.md. The gap: does the middleware intercept at `ToolMiddleware` level, at `ServerCore::handle_call_tool`, or via a pre-handler hook? This needs a decision in Phase 3 planning before implementation starts.

- **TTL default for workflow tasks:** Research establishes that workflow tasks need longer TTL than tool tasks (hours vs seconds) but does not specify a concrete default value. This needs a number in Phase 1 schema design (it goes in `WorkflowProgress` or workflow builder configuration).

- **`WorkflowProgress` schema migration strategy:** PITFALLS.md recommends a `schema_version: u32` field but does not specify migration trigger timing (lazy on read, eager on write, background job). A concrete decision is needed in Phase 1 to avoid retrofitting migration logic later.

## Sources

### Primary (HIGH confidence)
- Codebase: `src/server/workflow/prompt_handler.rs` — WorkflowPromptHandler, ExecutionContext, step loop, break behavior (lines 838-961)
- Codebase: `src/server/tasks.rs` — TaskRouter trait, existing 7-method interface and Value-based design rationale
- Codebase: `crates/pmcp-tasks/src/router.rs` — TaskRouterImpl, handle_task_call delegation pattern
- Codebase: `crates/pmcp-tasks/src/context.rs` — TaskContext, variable read/write API
- Codebase: `crates/pmcp-tasks/src/store/mod.rs` — TaskStore trait, StoreConfig, variable size limits (1MB)
- Codebase: `src/server/builder.rs` — ServerCoreBuilder, with_task_store() pattern
- Codebase: `src/server/workflow/sequential.rs` — SequentialWorkflow, validate()
- Codebase: `src/server/workflow/data_source.rs` — DataSource enum (PromptArg, StepOutput, Constant)
- [MCP Tasks Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25) — Task states, polling, result retrieval
- [RFC 1105: API Evolution](https://rust-lang.github.io/rfcs/1105-api-evolution.html) — non-breaking trait method additions via default impls
- `.planning/PROJECT.md` — v1.1 milestone scope and validated decisions
- `docs/design/tasks-feature-design.md` — workflow integration vision (section 8.5)

### Secondary (MEDIUM confidence)
- [A2A Protocol: Life of a Task](https://a2a-protocol.org/latest/topics/life-of-a-task/) — multi-turn task lifecycle, artifact accumulation, input-required patterns (closest analogue to PMCP bridge)
- [WorkOS: MCP Async Tasks Guide](https://workos.com/blog/mcp-async-tasks-ai-agent-workflows) — client continuation patterns, polling best practices
- [MCP Prompts: Building Workflow Automation](http://blog.modelcontextprotocol.io/posts/2025-07-29-prompts-for-automation/) — prompt-driven automation patterns
- [SEP-1686: Tasks Proposal](https://github.com/modelcontextprotocol/modelcontextprotocol/issues/1686) — task lifecycle, input_required semantics

### Tertiary (LOW confidence — analogues and architectural patterns)
- [Restate: Building a Durable Execution Engine](https://www.restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles) — step checkpoint patterns
- [State Machine Executor Part 2 — Fault Tolerance](https://blog.adamfurmanek.pl/2025/10/13/state-machine-executor-part-2/) — serialization pitfalls in paused state machines
- [AWS Step Functions Variable Passing](https://docs.aws.amazon.com/step-functions/latest/dg/workflow-variables.html) — flat variable schema patterns for step data flow
- [Palantir: Best practices for LLM prompt engineering](https://www.palantir.com/docs/foundry/aip/best-practices-prompt-engineering) — structuring prompts for reliable tool use across LLM families

---
*Research completed: 2026-02-21*
*Ready for roadmap: yes*
