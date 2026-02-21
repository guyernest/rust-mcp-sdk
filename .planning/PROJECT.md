# MCP Tasks Support for PMCP SDK

## What This Is

A separate crate (`pmcp-tasks`) that implements the MCP Tasks specification (2025-11-25, experimental) for the PMCP SDK. Tasks enable durable state machines for long-running operations with polling-based result retrieval, task variables as shared client/server state, and pluggable storage backends starting with DynamoDB.

## Core Value

Tool handlers can manage long-running operations through a durable task lifecycle (create, poll, complete) with shared variable state that persists across tool calls — giving servers memory without an LLM.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Core protocol types (Task, TaskStatus, CreateTaskResult, etc.) matching MCP spec 2025-11-25
- [ ] Task status state machine with validated transitions (working, input_required, completed, failed, cancelled)
- [ ] TaskStore trait with pluggable storage backends
- [ ] In-memory storage backend for dev/testing
- [ ] DynamoDB storage backend behind `dynamodb` feature flag
- [ ] TaskContext for ergonomic handler integration (get/set variables, status transitions)
- [ ] PMCP extension: task variables as shared client/server scratchpad via `_meta`
- [ ] Server/client task capability types and negotiation via `experimental.tasks`
- [ ] Tool-level task support declaration (forbidden/optional/required)
- [ ] TaskHandler for routing tasks/get, tasks/result, tasks/list, tasks/cancel
- [ ] TaskMiddleware for intercepting task-augmented tools/call requests
- [ ] Owner binding security (OAuth sub, client ID, session ID fallback)
- [ ] TaskSecurityConfig with configurable limits (max tasks per owner, max TTL, trait-level variable size limits)
- [ ] Integration with existing SequentialWorkflow system (task-backed workflows)
- [ ] Integration with cargo-pmcp deployment plugin system (DynamoDB table via CFN stack)
- [ ] Comprehensive test suite: unit, property, integration, security, DynamoDB (real cloud table in CI)
- [ ] Examples: basic tasks, workflow integration, code mode, DynamoDB backend

### Out of Scope

- Task status notifications — skip for now, rely on polling only
- Bounded blocking on tasks/result — polling-only behavior
- Redis or other non-DynamoDB backends — future phase
- Task progress streaming via SSE — future phase
- Moving types into core pmcp crate — wait for spec stabilization
- Namespaced variable keys — flat keys with convention recommendation in docs
- Variable size enforcement per-backend — use trait-level configurable limit instead

## Context

- The MCP Tasks spec is experimental (2025-11-25). Most MCP clients don't support it yet, so the feature must be optional and isolated.
- PMCP extends the minimal spec with task variables — a shared scratchpad visible to both client and server via `_meta`. This is the key innovation for servers without LLM capabilities.
- The SDK already has a workflow system (`SequentialWorkflow`) that benefits from task-backed durable state.
- `cargo-pmcp` has pluggable deployment targets (Lambda+CFN, Google Run+Docker, Cloudflare Workers+wrangler). Task storage backends should follow the same plugin pattern, starting with DynamoDB+CFN.
- Detailed design document: `docs/design/tasks-feature-design.md`

## Constraints

- **Isolation**: Must be a separate crate (`pmcp-tasks`) — experimental feature cannot destabilize core SDK
- **Spec compliance**: Protocol types must match MCP 2025-11-25 schema exactly
- **Feature gating**: DynamoDB backend behind `dynamodb` feature flag
- **Compatibility**: No breaking changes to existing `pmcp` crate API
- **Testing**: Real DynamoDB in CI (cloud test table), no local docker dependency
- **Variable limits**: Trait-level configurable size limit enforced across all backends

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate crate (`pmcp-tasks`) | Experimental spec isolation, independent versioning | -- Pending |
| Polling-only for tasks/result | Simpler implementation, Lambda-compatible, spec allows it | -- Pending |
| Trait-level variable size limits | Consistent enforcement across backends, not just DynamoDB's 400KB | -- Pending |
| Skip notifications for now | Simplifies initial implementation, polling sufficient | -- Pending |
| Flat variable keys | Simplicity over structure, convention in docs | -- Pending |
| Real DynamoDB in CI | Fast enough, no docker dependency, matches production behavior | -- Pending |
| Pluggable storage via cargo-pmcp pattern | Consistent with existing deployment target architecture | -- Pending |
| Capabilities via experimental field | Spec-compliant for experimental features, migrate when stabilized | -- Pending |

---
*Last updated: 2026-02-21 after initialization*
