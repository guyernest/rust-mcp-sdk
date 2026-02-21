# Feature Research: MCP Tasks for PMCP SDK

**Domain:** Protocol SDK -- durable task system for long-running MCP operations
**Researched:** 2026-02-21
**Confidence:** HIGH (MCP spec is authoritative, A2A and durable execution frameworks are well-documented)

## Feature Landscape

### Table Stakes (Users Expect These)

These are features that any MCP Tasks implementation MUST have. If missing, the SDK fails spec compliance or is unusable for its intended purpose. An SDK that advertises "tasks support" without these will be rejected by integrators.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Core protocol types** (Task, TaskStatus, CreateTaskResult, TaskParams) | Spec compliance. Types must serialize to match MCP 2025-11-25 schema exactly. Without correct wire format, no client will interop. | LOW | Straightforward serde structs. Design doc already has these. Map 1:1 to spec. |
| **Task status state machine** with validated transitions | Spec MUST requirement. `working` -> terminal states, `input_required` <-> `working`. Terminal states are immutable. Invalid transitions MUST be rejected. | LOW | 5 states, ~8 valid transitions. Well-defined in spec. Property-testable. |
| **TaskStore trait** (pluggable storage abstraction) | SDK consumers need to bring their own storage. Without a trait, the SDK is locked to one backend. Every comparable system (Temporal, Restate, Azure Durable Functions) uses pluggable storage. | MEDIUM | Async trait with create/get/update_status/set_result/get_result/list/cancel/cleanup. Design doc has solid API. |
| **In-memory storage backend** | Dev/test is blocked without this. Nobody can try the feature without spinning up DynamoDB. Every SDK ships a local-first backend. | LOW | HashMap + RwLock. Already designed in detail. |
| **Capability negotiation** (server + client task capabilities) | Spec MUST. Servers/clients MUST declare `tasks` capability during init. Without this, no client knows the server supports tasks. Two-layer: global capabilities + per-tool `execution.taskSupport`. | MEDIUM | Needs integration with existing `ServerCapabilities`/`ClientCapabilities`. Currently via `experimental` field (correct for experimental spec). |
| **Tool-level task support declaration** (forbidden/optional/required) | Spec defines three levels per tool. Clients MUST respect these. Missing = clients cannot determine which tools support task augmentation. | LOW | Enum + field on ToolInfo. Already designed. |
| **Task creation flow** (task-augmented tools/call -> CreateTaskResult) | The core protocol flow. Client sends `tools/call` with `task: {}`, server returns `CreateTaskResult` immediately, processes in background. Without this, tasks don't work at all. | HIGH | Requires middleware intercepting tools/call, detecting `task` field, creating task, spawning background execution, returning handle. Most complex integration point. |
| **tasks/get** (polling endpoint) | Spec MUST. Primary mechanism for requestors to check task status. Spec says polling is authoritative -- notifications are optional. | LOW | Route JSON-RPC method to store.get(). Return current Task state. |
| **tasks/result** (result retrieval) | Spec MUST. Returns the actual operation result (e.g., CallToolResult) once task reaches terminal status. MUST block for non-terminal tasks. MUST include `_meta` with related-task. | MEDIUM | Blocking semantics need careful implementation. Must return exactly what the original request would have returned. |
| **tasks/cancel** (cancellation) | Spec expects this when `tasks.cancel` capability is declared. MUST reject for terminal tasks with `-32602`. MUST transition to `cancelled`. | LOW | Validate state, update store, return cancelled task. |
| **tasks/list** (enumeration with pagination) | Spec expects this when `tasks.list` capability is declared. Cursor-based pagination. MUST scope to requestor's authorization context. | MEDIUM | Needs cursor-based pagination. Straightforward for in-memory, needs GSI for DynamoDB. |
| **Related-task metadata** (`_meta` with `io.modelcontextprotocol/related-task`) | Spec MUST. All task-related requests/responses/notifications MUST carry this. Without it, multi-step task flows cannot correlate messages. | LOW | Helper function to inject/extract metadata. Already designed. |
| **TTL and resource management** | Spec requires `createdAt`, `lastUpdatedAt` timestamps. Receivers MAY override requested TTL. After TTL, receivers MAY delete task. Without TTL, tasks accumulate forever. | LOW | Timestamp management + periodic cleanup. DynamoDB has native TTL. |
| **Error handling** (protocol errors for task operations) | Spec defines specific error codes: `-32602` for invalid taskId, expired task, terminal cancellation. `-32603` for internal errors. | LOW | Map TaskError variants to JSON-RPC error codes. |
| **Owner binding / access control** | Spec MUST: "When an authorization context is provided, receivers MUST bind tasks to said context." Tasks without auth MUST use cryptographically secure IDs. | MEDIUM | Resolve owner from OAuth sub / client ID / session ID. Enforce on every operation. Design doc has owner resolution hierarchy. |
| **Security limits** (max tasks per owner, max TTL) | Spec SHOULD: "Enforce limits on concurrent tasks per requestor" and "Enforce maximum ttl durations." Without limits, any client can exhaust server resources. | LOW | Configuration struct with sensible defaults. Already designed as `TaskSecurityConfig`. |

### Differentiators (Competitive Advantage)

These features go beyond the MCP spec or beyond what other MCP SDKs provide. They are what makes PMCP's task implementation worth choosing over alternatives.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Task variables (PMCP extension)** | The key innovation. Shared client/server scratchpad via `_meta`. Servers without LLMs need memory across tool calls -- task variables ARE that memory. A2A has artifacts for output but no equivalent of a shared mutable scratchpad. No other MCP SDK has this. | MEDIUM | HashMap<String, Value> per task. Merge semantics (new keys added, existing overwritten, null deletes). Surfaced to client via `_meta`. Needs size limits. |
| **TaskContext ergonomic API** | Tool handlers get a `TaskContext` with `get_variable()`, `set_variable()`, `require_input()`, `complete()`, `fail()`. Developers never touch the raw store. Comparable to Temporal's workflow context or Restate's handler context. Far better DX than raw task store operations. | MEDIUM | Wrapper around Arc<dyn TaskStore> + task_id + owner_id. Already designed in detail. |
| **DynamoDB storage backend** | Production-ready serverless storage. PMCP's primary deployment target is AWS Lambda. DynamoDB native TTL, conditional writes for atomic transitions, GSI for owner-scoped listing. No other Rust MCP SDK ships a cloud-native backend. | HIGH | AWS SDK integration, table schema design, conditional expressions, GSI pagination. Feature-gated behind `dynamodb`. |
| **SequentialWorkflow integration** | Existing workflow system gets task-backed durability. Steps read/write task variables instead of ephemeral state. DataSource::StepOutput resolves from task variables. Workflows survive Lambda cold starts. | HIGH | Needs careful integration with existing workflow module. Step bindings map to task variables. `input_required` status maps to elicitation steps. |
| **Separate crate isolation** (`pmcp-tasks`) | Experimental spec feature in its own crate with independent versioning. Core SDK stability unaffected. Users opt in explicitly. Follows the same pattern as `cargo-pmcp` plugins. | LOW | Workspace member, optional re-export via feature flag. Already decided. |
| **Task status notifications** | Spec says receivers MAY send `notifications/tasks/status`. Proactive push reduces polling overhead. Valuable for SSE/Streamable HTTP transports (not Lambda). | MEDIUM | Send notification on status transition. Transport-dependent (SSE yes, Lambda no). Requestors MUST NOT rely on them -- purely supplementary. |
| **Model immediate response** | Spec provisional pattern: `_meta["io.modelcontextprotocol/model-immediate-response"]` in CreateTaskResult. Lets the model continue processing while task runs. Reduces perceived latency for LLM clients. | LOW | Optional string field in CreateTaskResult `_meta`. Server provides, host decides whether to use. |
| **Progress token continuity** | Spec says progressToken from original request remains valid throughout task lifetime. Enables progress reporting during long-running tasks. | LOW | Thread progressToken from original request through to background execution. Compatible with existing progress notification system. |
| **CloudFormation template for DynamoDB table** | Ship-ready infrastructure-as-code. `cargo-pmcp` deploy plugin creates the table automatically. Eliminates manual DynamoDB setup. | LOW | Static YAML/JSON file + `create_table()` helper for dev. Plugs into existing cargo-pmcp CFN stack pattern. |
| **Configurable variable size limits** | Prevent DynamoDB 400KB item limit from causing cryptic errors. Trait-level enforcement across all backends gives consistent behavior regardless of storage. | LOW | Check serialized variable size before write. Configurable limit (default 100KB). |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem appealing but create problems. These are deliberate exclusions with rationale.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Built-in task scheduler / job queue** | "The SDK should schedule and run background tasks automatically" | An SDK is not a runtime. Task execution is the server's responsibility. A scheduler would impose concurrency models, thread pools, and runtime assumptions that conflict with Lambda, containers, and custom runtimes. Temporal, Restate, Celery exist for this. | Provide `TaskMiddleware` that spawns a tokio task for background execution. Server authors control their own concurrency. Document integration patterns with external schedulers. |
| **Redis / PostgreSQL / SQLite backends** | "We need more storage options beyond in-memory and DynamoDB" | Scope creep. Each backend needs maintenance, testing, CI infrastructure. The 80/20 is in-memory (dev) + DynamoDB (prod on AWS). The trait exists for users to implement their own. | Ship the `TaskStore` trait as the extension point. Document how to implement a custom backend. Add community backends via separate crates later. |
| **Task progress streaming via SSE** | "I want real-time progress updates streamed to the client" | Conflates two MCP features. Progress notifications already exist in the base protocol (spec Section: Progress). Tasks have progress token continuity. Adding a separate SSE streaming layer duplicates functionality and complicates the transport layer. | Use existing MCP progress notifications with the task's progressToken. Progress is orthogonal to tasks. |
| **Bounded blocking on tasks/result for Lambda** | "Lambda has a max execution time, so tasks/result should timeout" | The spec says tasks/result MUST block until terminal status. Adding a timeout parameter creates spec non-compliance and ambiguous behavior (what does the client do on timeout?). | Polling-only pattern works perfectly for Lambda. Client polls tasks/get, calls tasks/result only when status is terminal. Document this pattern explicitly. |
| **Namespaced variable keys** (e.g., `step.validate.region`) | "Variables should be organized by step or namespace" | Adds structural complexity without clear benefit. Flat keys are simpler to reason about, query, and merge. Namespacing can be done by convention (`validate_region`) without protocol enforcement. | Flat keys with convention recommendation in docs. Example: prefix with step name if disambiguation needed. |
| **Moving task types into core pmcp crate** | "Types should be in the main crate for convenience" | Spec is experimental. If types are in core and the spec changes, core gets a breaking change. Isolation protects stability. | Keep in `pmcp-tasks` until spec stabilizes. Re-export via `pmcp = { features = ["tasks"] }` for convenience. Migrate to core in Phase 2 (post-stabilization). |
| **Task history / event log** | "Track every state transition with timestamps for debugging" | A2A and Temporal have event history because they're workflow engines. MCP tasks are simpler -- they're durable state machines, not event-sourced workflows. Adding event history increases storage requirements and complexity for marginal debugging benefit. | Store `statusMessage` on each transition. Use tracing/logging for audit trail. `lastUpdatedAt` shows when last change happened. Recommend external observability (OpenTelemetry) for production debugging. |
| **Task-to-task dependencies / DAG execution** | "Tasks should be able to depend on other tasks" | MCP tasks are independent state machines per the spec. Dependencies would make PMCP a workflow engine, which is not the goal. The `SequentialWorkflow` system already handles step ordering. | Use `SequentialWorkflow` for ordered steps. Each step can be backed by a task. The workflow itself handles ordering and data flow between steps. |
| **Automatic retry with exponential backoff** | "Failed tasks should retry automatically" | The SDK cannot know whether a failed operation is idempotent. Automatic retry of non-idempotent operations (deploy, delete, payment) causes duplicate side effects. Retry policy is an application concern. | Provide hooks for server authors to implement retry logic. Document idempotency requirements. Let the application decide retry strategy. |

## Feature Dependencies

```
[Core Protocol Types]
    |
    +--requires--> [Task Status State Machine]
    |
    +--requires--> [TaskStore Trait]
    |                   |
    |                   +--requires--> [In-Memory Backend]
    |                   |
    |                   +--requires--> [DynamoDB Backend] (optional, behind feature flag)
    |
    +--requires--> [Capability Negotiation]
    |                   |
    |                   +--requires--> [Tool-Level Task Support]
    |
    +--requires--> [Task Creation Flow (TaskMiddleware)]
    |                   |
    |                   +--requires--> [Owner Binding / Access Control]
    |                   |
    |                   +--requires--> [Security Limits]
    |                   |
    |                   +--enables--> [tasks/get (Polling)]
    |                   |
    |                   +--enables--> [tasks/result (Result Retrieval)]
    |                   |
    |                   +--enables--> [tasks/cancel]
    |                   |
    |                   +--enables--> [tasks/list]
    |
    +--enables--> [Related-Task Metadata]
    |
    +--enables--> [TTL & Resource Management]
    |
    +--enables--> [Error Handling]

[TaskStore Trait] + [Core Protocol Types]
    |
    +--enables--> [Task Variables (PMCP Extension)]
    |                   |
    |                   +--enables--> [TaskContext Ergonomic API]
    |                   |
    |                   +--enables--> [Variable Size Limits]
    |
    +--enables--> [DynamoDB Backend]
                        |
                        +--enables--> [CloudFormation Template]

[TaskContext] + [SequentialWorkflow]
    |
    +--enables--> [Workflow Integration]
                        |
                        +--requires--> [Task Variables]
                        +--requires--> [Task Creation Flow]

[Task Creation Flow]
    |
    +--enables--> [Model Immediate Response]
    |
    +--enables--> [Progress Token Continuity]
    |
    +--enables--> [Task Status Notifications]
```

### Dependency Notes

- **Core Protocol Types are the foundation:** Everything depends on having correct, spec-compliant types. Build these first, test serialization exhaustively.
- **TaskStore Trait gates all backends:** The trait must be designed before any backend is implemented. Get the API right because changing it later breaks all backends.
- **Task Creation Flow is the hardest integration:** The middleware intercepts tools/call, creates tasks, spawns background execution, and returns CreateTaskResult. This touches the server's request routing, which is the most sensitive part of the existing SDK.
- **Task Variables require TaskStore but not Task Creation Flow:** Variables can be tested in isolation against the store before the full middleware is wired up.
- **Workflow Integration depends on both TaskContext and existing SequentialWorkflow:** This is the last feature to build because it requires both the new task system and understanding of the existing workflow module.
- **DynamoDB Backend and CloudFormation are independent of the core task flow:** They can be built in parallel with other features once the TaskStore trait is stable.

## MVP Definition

### Launch With (v0.1.0 -- pmcp-tasks)

Minimum viable product to validate the task system works end-to-end with real MCP clients.

- [ ] **Core protocol types** -- Foundation for everything. Blocks all other work.
- [ ] **Task status state machine** -- Validated transitions, property-tested.
- [ ] **TaskStore trait** -- The extension point. Must be stable before backends.
- [ ] **In-memory storage backend** -- Enables dev/testing without cloud dependencies.
- [ ] **Capability negotiation** -- Via `experimental.tasks` field. Without this, no client knows tasks are supported.
- [ ] **Tool-level task support** -- Per-tool forbidden/optional/required.
- [ ] **Task creation flow (TaskMiddleware)** -- The core integration. Intercept tools/call, spawn background, return handle.
- [ ] **tasks/get, tasks/result, tasks/cancel, tasks/list** -- The four task management endpoints.
- [ ] **Related-task metadata** -- Message correlation across task lifecycle.
- [ ] **TTL and resource management** -- Prevent task accumulation.
- [ ] **Error handling** -- Spec-compliant error codes.
- [ ] **Owner binding** -- Security baseline. Resolve from auth context or session.
- [ ] **Security limits** -- Max tasks per owner, max TTL.
- [ ] **Task variables (PMCP extension)** -- The differentiator. Ship it in v0.1 because it is the core value proposition.
- [ ] **TaskContext ergonomic API** -- Without this, the DX is unusable for tool handlers.
- [ ] **Basic example** (`60_tasks_basic.rs`) -- Proves the system works end-to-end.

### Add After Validation (v0.2.0)

Features to add once the core task system is working and tested with real clients.

- [ ] **DynamoDB backend** -- Trigger: first production deployment on AWS Lambda.
- [ ] **CloudFormation template** -- Trigger: `cargo-pmcp` deployment integration needed.
- [ ] **SequentialWorkflow integration** -- Trigger: users request durable workflows.
- [ ] **Task status notifications** -- Trigger: SSE/Streamable HTTP transport users need proactive updates.
- [ ] **Model immediate response** -- Trigger: LLM host applications need to continue processing during tasks.
- [ ] **Progress token continuity** -- Trigger: long-running tasks need progress reporting.
- [ ] **Variable size limits** -- Trigger: DynamoDB backend encounters 400KB limit issues.
- [ ] **Additional examples** (workflow, code mode, DynamoDB) -- Trigger: v0.1 is stable.

### Future Consideration (v0.3+ / post-spec-stabilization)

Features to defer until the MCP Tasks spec stabilizes and production usage patterns emerge.

- [ ] **Move types to core pmcp crate** -- Wait for spec to drop "experimental" status.
- [ ] **Redis backend** -- Wait for non-AWS deployment demand.
- [ ] **Task analytics / monitoring hooks** -- Wait for OpenTelemetry integration patterns to emerge.
- [ ] **Streaming SSE integration** -- Wait for transport-layer patterns to mature.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Core protocol types | HIGH | LOW | P1 |
| Task status state machine | HIGH | LOW | P1 |
| TaskStore trait | HIGH | MEDIUM | P1 |
| In-memory backend | HIGH | LOW | P1 |
| Capability negotiation | HIGH | MEDIUM | P1 |
| Tool-level task support | HIGH | LOW | P1 |
| Task creation flow (middleware) | HIGH | HIGH | P1 |
| tasks/get, tasks/result, tasks/cancel, tasks/list | HIGH | MEDIUM | P1 |
| Related-task metadata | HIGH | LOW | P1 |
| TTL & resource management | HIGH | LOW | P1 |
| Error handling | HIGH | LOW | P1 |
| Owner binding | HIGH | MEDIUM | P1 |
| Security limits | MEDIUM | LOW | P1 |
| Task variables (PMCP extension) | HIGH | MEDIUM | P1 |
| TaskContext API | HIGH | MEDIUM | P1 |
| DynamoDB backend | HIGH | HIGH | P2 |
| CloudFormation template | MEDIUM | LOW | P2 |
| Workflow integration | HIGH | HIGH | P2 |
| Task notifications | MEDIUM | MEDIUM | P2 |
| Model immediate response | MEDIUM | LOW | P2 |
| Progress token continuity | MEDIUM | LOW | P2 |
| Variable size limits | MEDIUM | LOW | P2 |

**Priority key:**
- P1: Must have for v0.1.0 launch (validates the task system works)
- P2: Should have, add in v0.2.0 (production readiness)
- P3: Nice to have, future consideration (post-stabilization)

## Competitor Feature Analysis

| Feature | MCP Spec (base) | A2A Protocol | Temporal/Restate | PMCP Approach |
|---------|-----------------|--------------|------------------|---------------|
| **Task states** | 5 states (working, input_required, completed, failed, cancelled) | 7+ states (submitted, working, input_required, completed, failed, canceled, rejected, auth_required, unknown) | Arbitrary workflow states via event history | Match MCP spec exactly: 5 states. Simpler is better for a protocol SDK. |
| **Shared state / variables** | Not in spec (tasks are opaque state machines) | Artifacts (output-only, not bidirectional) | Workflow context + durable state | Task variables via `_meta` -- bidirectional shared scratchpad. PMCP's key differentiator. |
| **Storage abstraction** | Not specified (SDK concern) | Not specified | Pluggable (SQL, Cassandra, etc.) | TaskStore trait + in-memory + DynamoDB. Same pattern as Temporal. |
| **Polling** | Primary mechanism. `pollInterval` hint. | Supported alongside streaming/webhooks | Not needed (push-based) | Polling-first. Matches spec and Lambda deployment model. |
| **Streaming updates** | Optional notifications | SSE streaming + push webhooks | Real-time via task queues | Optional notifications in v0.2. Polling is source of truth. |
| **Cancellation** | Explicit tasks/cancel endpoint | sendMessage with cancel intent | Workflow cancellation tokens | Spec-compliant tasks/cancel with state validation. |
| **Access control** | Auth context binding (MUST when available) | Agent card authentication | Namespace-based | Owner binding from OAuth/client ID/session. Enforce on every operation. |
| **Pagination** | Cursor-based for tasks/list | Cursor-based | Workflow list filters | Cursor-based, matching spec. |
| **Progress** | progressToken continuity | Streaming events | Heartbeat + progress | Reuse existing MCP progress notification system. |
| **Result retrieval** | Blocking tasks/result endpoint | Artifact streaming | Query workflow result | Spec-compliant blocking + polling fallback pattern. |
| **Middleware/interceptor** | Not specified (SDK concern) | Not specified | Activity interceptors | TaskMiddleware intercepts tools/call with `task` field. Clean separation. |
| **Workflow integration** | Not in spec | Task lifecycle is the workflow | Core feature | SequentialWorkflow backed by tasks. Unique PMCP value. |

### Key Takeaways from Competitor Analysis

1. **MCP Tasks are intentionally simpler than A2A tasks.** A2A is a workflow protocol; MCP tasks are a durable state machine primitive. Do not over-engineer toward A2A's complexity.

2. **Temporal/Restate validate the storage abstraction pattern.** Pluggable backends with a trait is the industry standard. PMCP's TaskStore trait follows this proven pattern.

3. **Task variables are PMCP's unique value.** Neither MCP spec, A2A, nor durable execution frameworks have a bidirectional shared scratchpad between client and server. A2A has artifacts (server->client output), Temporal has workflow context (server-only). PMCP's task variables bridge both sides.

4. **Polling-first is correct for Lambda.** A2A supports webhooks for disconnected scenarios. MCP spec says polling is authoritative. For Lambda (stateless, short-lived), polling-only is the right default. Notifications are a bonus for long-lived servers.

5. **Do not build a workflow engine.** The SDK should enable task-backed workflows, not BE a workflow engine. SequentialWorkflow + TaskContext is the sweet spot -- structured enough to be useful, simple enough to maintain.

## Sources

### Authoritative (HIGH confidence)
- [MCP Tasks Specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks) -- Complete task spec with all MUST/SHOULD/MAY requirements
- [MCP Specification Overview](https://modelcontextprotocol.io/specification/2025-11-25) -- Protocol context
- [MCP Anniversary Blog](http://blog.modelcontextprotocol.io/posts/2025-11-25-first-mcp-anniversary/) -- Spec release context

### Verified (MEDIUM confidence)
- [WorkOS: MCP Async Tasks Guide](https://workos.com/blog/mcp-async-tasks-ai-agent-workflows) -- Implementation patterns and SDK responsibilities
- [WorkOS: MCP 2025-11-25 Spec Update](https://workos.com/blog/mcp-2025-11-25-spec-update) -- Feature analysis
- [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/) -- Task model comparison
- [A2A Streaming & Async](https://a2a-protocol.org/latest/topics/streaming-and-async/) -- A2A async patterns
- [A2A Key Concepts](https://a2a-protocol.org/latest/topics/key-concepts/) -- A2A task lifecycle
- [Temporal: Building Resilient Systems with AWS](https://aws.amazon.com/blogs/apn/building-resilient-distributed-systems-with-temporal-and-aws/) -- Durable execution patterns
- [Restate Documentation](https://docs.restate.dev/use-cases/async-tasks/) -- Async task patterns, Rust SDK
- [Restate Rust SDK](https://docs.rs/restate-sdk/latest/restate_sdk/) -- Rust durable execution API reference

### Additional Context (LOW confidence -- patterns only)
- [MCP vs A2A Comparison](https://www.blott.com/blog/post/mcp-vs-a2a-which-protocol-is-better-for-ai-agents) -- Protocol positioning
- [Azure Durable Functions](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-overview) -- Durable task patterns
- [Hatchet: How to Think About Durable Execution](https://hatchet.run/blog/durable-execution) -- State management patterns

---
*Feature research for: MCP Tasks implementation in PMCP SDK*
*Researched: 2026-02-21*
