# MCP Tasks Support for PMCP SDK

## What This Is

A separate crate (`pmcp-tasks`) that implements the MCP Tasks specification (2025-11-25, experimental) for the PMCP SDK, with a task-prompt bridge that enables workflow prompts to create tasks, execute server-resolvable steps, and return structured handoff guidance for LLM client continuation.

## Core Value

Tool handlers can manage long-running operations through a durable task lifecycle (create, poll, complete) with shared variable state that persists across tool calls — giving servers memory without an LLM.

## Requirements

### Validated

- ✓ Core protocol types (Task, TaskStatus, CreateTaskResult, etc.) matching MCP spec 2025-11-25 — v1.0
- ✓ Task status state machine with validated transitions (5 states, 46 transition tests) — v1.0
- ✓ TaskStore trait with pluggable storage backends (11 async methods) — v1.0
- ✓ In-memory storage backend for dev/testing (DashMap, atomic transitions) — v1.0
- ✓ TaskContext for ergonomic handler integration (typed accessors, status transitions) — v1.0
- ✓ PMCP extension: task variables as shared client/server scratchpad via `_meta` — v1.0
- ✓ Server/client task capability types and negotiation via `experimental.tasks` — v1.0
- ✓ Tool-level task support declaration (forbidden/optional/required) — v1.0
- ✓ TaskRouter for routing tasks/get, tasks/result, tasks/list, tasks/cancel — v1.0
- ✓ Task interception for task-augmented tools/call requests — v1.0
- ✓ Owner binding security (OAuth sub, client ID, session ID fallback) — v1.0
- ✓ TaskSecurityConfig with configurable limits (max tasks, TTL, variable size) — v1.0
- ✓ Comprehensive test suite: unit (200+), property (13), integration (11), security (19) — v1.0
- ✓ Basic tasks example (60_tasks_basic.rs) — v1.0
- ✓ Task-aware workflow prompts that create tasks and bind step progress — v1.1
- ✓ Partial server-side execution with automatic pause on unresolvable steps — v1.1
- ✓ Structured prompt reply conveying completed steps, remaining steps, and task ID — v1.1
- ✓ Step state tracking in task variables (standard schema: goal, steps, completed, remaining) — v1.1
- ✓ Client continuation pattern via direct tool calls guided by prompt reply — v1.1
- ✓ Working example demonstrating task-prompt bridge with multi-step workflow — v1.1

### Active

(None — start `/gsd:new-milestone` to define next milestone requirements)

### Future

- [ ] DynamoDB storage backend behind `dynamodb` feature flag
- [ ] DynamoDB conditional writes for atomic state transitions
- [ ] DynamoDB TTL + read-time expiry filtering
- [ ] DynamoDB GSI for owner-scoped listing with cursor-based pagination
- [ ] CloudFormation template integrating with cargo-pmcp deployment plugin system
- [ ] Integration with cargo-pmcp deployment plugin system (DynamoDB table via CFN stack)
- [ ] Cross-server task sharing on pmcp.run — shared TaskStore backend + OAuth sub owner binding enables multi-server workflow continuity
- [ ] DataSource::TaskVariable for steps to read values from task variable store
- [ ] Workflow resume from task state (re-invoke prompt with task ID to continue from last step)
- [ ] StepExecution user API for runtime step mode customization
- [ ] Examples: code mode, DynamoDB backend

### Out of Scope

- Task status notifications — skip for now, rely on polling only (validated by v1.0: polling works well)
- Bounded blocking on tasks/result — polling-only behavior
- Redis or other non-DynamoDB backends — future phase
- Task progress streaming via SSE — future phase
- Moving types into core pmcp crate — wait for spec stabilization
- Namespaced variable keys — flat keys with convention recommendation in docs (validated by v1.0: flat keys sufficient)
- Variable size enforcement per-backend — trait-level configurable limit works (validated by v1.0)
- Automatic client execution — MCP clients decide when/how to call tools; server cannot drive client
- Per-step task statuses — single task status with variable-level step tracking suffices (validated by v1.1)
- Workflow branching/conditionals — sequential-only; branching is a different workflow engine

## Context

Shipped v1.1 with ~22,000 Rust LOC across `pmcp-tasks` crate and `pmcp` core modifications (v1.0: ~11,500 + v1.1: +10,697).
Tech stack: `pmcp-tasks` (serde, async-trait, dashmap, uuid, chrono, tokio, parking_lot) + `pmcp` core (protocol types, ServerCore routing, workflow system).

- The MCP Tasks spec is experimental (2025-11-25). Most MCP clients don't support it yet, so the feature is optional and isolated in `pmcp-tasks`.
- PMCP extends the minimal spec with task variables — a shared scratchpad visible to both client and server via `_meta`. This is the key innovation for servers without LLM capabilities.
- v1.1 bridges the `SequentialWorkflow` system with tasks: workflows pause mid-execution and the client continues via structured handoff guidance.
- The workflow-as-prompt model: domain experts design MCP prompts that chain tools and resources. The prompt defines steps, the server executes what it can, the task tracks what's done, and the LLM client picks up the rest.
- `cargo-pmcp` has pluggable deployment targets (Lambda+CFN, Google Run+Docker, Cloudflare Workers+wrangler). Task storage backends should follow the same plugin pattern, starting with DynamoDB+CFN.
- Detailed design document: `docs/design/tasks-feature-design.md`

## Constraints

- **Isolation**: Must be a separate crate (`pmcp-tasks`) — experimental feature cannot destabilize core SDK
- **Spec compliance**: Protocol types must match MCP 2025-11-25 schema exactly
- **Feature gating**: DynamoDB backend behind `dynamodb` feature flag
- **Compatibility**: No breaking changes to existing `pmcp` crate API (validated: v1.0 and v1.1 only additive changes)
- **Testing**: Real DynamoDB in CI (cloud test table), no local docker dependency
- **Variable limits**: Trait-level configurable size limit enforced across all backends

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate crate (`pmcp-tasks`) | Experimental spec isolation, independent versioning | ✓ Good — clean separation, pmcp core unchanged for non-task users |
| Polling-only for tasks/result | Simpler implementation, Lambda-compatible, spec allows it | ✓ Good — 11 integration tests validate polling flow |
| Trait-level variable size limits | Consistent enforcement across backends, not just DynamoDB's 400KB | ✓ Good — `StoreConfig.max_variable_size_bytes` enforced in InMemoryTaskStore |
| Skip notifications for now | Simplifies initial implementation, polling sufficient | ✓ Good — TaskStatusNotification type defined but not wired; ready for v2 |
| Flat variable keys | Simplicity over structure, convention in docs | ✓ Good — top-level injection into `_meta` works cleanly |
| Capabilities via experimental field | Spec-compliant for experimental features, migrate when stabilized | ✓ Good — `experimental.tasks` auto-configured by `with_task_store()` |
| serde_json::Value for TaskRouter | Avoid circular crate dependency (pmcp-tasks depends on pmcp) | ✓ Good — clean trait boundary, pmcp has zero knowledge of pmcp-tasks types |
| DashMap for InMemoryTaskStore | Matches SessionManager pattern in existing codebase | ✓ Good — concurrent access tested with 10-thread proptest |
| Owner ID as structural key | NotFound on mismatch (never OwnerMismatch) — no info leakage | ✓ Good — 19 security tests verify isolation |
| TaskRouter in pmcp, impl in pmcp-tasks | One-directional dependency, builder accepts Arc\<dyn TaskRouter\> | ✓ Good — example only needs pmcp-tasks imports |
| Composition over modification (v1.1) | TaskWorkflowPromptHandler wraps WorkflowPromptHandler without changing it | ✓ Good — zero backward-compatibility issues, all existing tests pass unchanged |
| Hybrid handoff format (v1.1) | `_meta` JSON for machine parsing + natural language for LLM clients | ✓ Good — works with any MCP client regardless of structured output support |
| Fire-and-forget continuation (v1.1) | Continuation recording never fails the tool call | ✓ Good — tool results always returned to client; recording is best-effort |
| Cancel-as-completion (v1.1) | `tasks/cancel` with result transitions to Completed, not Cancelled | ✓ Good — enables clean workflow completion after all steps done client-side |
| Local mirror types (v1.1) | PauseReason/StepStatus mirrored in pmcp to avoid circular dependency | ✓ Good — same approach as TaskRouter; clean trait boundary preserved |
| Runtime best-effort execution (v1.1) | Dropped StepExecution enum; steps execute what they can at runtime | ✓ Good — simpler than static classification; PauseReason captures why stops |

---
*Last updated: 2026-02-23 after v1.1 milestone completion*
