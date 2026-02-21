# Architecture Patterns

**Domain:** Durable task system for async Rust protocol SDK (MCP Tasks for PMCP)
**Researched:** 2026-02-21
**Confidence:** HIGH (based on existing codebase analysis, official MCP Tasks spec, and established Rust patterns)

## Recommended Architecture

A **layered crate** (`pmcp-tasks`) within the existing workspace, following the dependency inversion principle: upper layers depend on trait abstractions defined in lower layers. The crate integrates with the host SDK through two narrow interfaces: a `ToolMiddleware` implementation for intercepting task-augmented `tools/call` requests, and a `TaskHandler` for routing `tasks/*` JSON-RPC methods.

```
                          External (pmcp crate)
                    +---------------------------------+
                    |  ServerCore request router      |
                    |  ServerCoreBuilder              |
                    |  ToolMiddlewareChain            |
                    +-----------+-----+---------------+
                                |     |
              +-----------------+     +------------------+
              |                                          |
              v                                          v
    +--------------------+                  +------------------------+
    | TaskMiddleware     |                  |  TaskHandler           |
    | (ToolMiddleware    |                  |  (tasks/get, result,   |
    |  impl, priority 5) |                  |   list, cancel)        |
    +--------+-----------+                  +-----------+------------+
             |                                          |
             +-------------------+----------------------+
                                 |
                                 v
                    +------------------------+
                    |  Layer 3: TaskContext   |  <-- handler-facing API
                    |  (get/set variables,   |
                    |   status transitions)  |
                    +-----------+------------+
                                |
                                v
                    +------------------------+
                    |  Layer 2: TaskStore    |  <-- storage trait
                    |  (async trait: create, |
                    |   get, update, list,   |
                    |   cancel, cleanup)     |
                    +-----------+------------+
                       |                  |
                       v                  v
             +----------------+  +------------------+
             | InMemoryTask   |  | DynamoDbTask     |
             | Store          |  | Store            |
             | (always)       |  | (feature-gated)  |
             +----------------+  +------------------+
                                          ^
                                          |
                    +------------------------+
                    |  Layer 1: Protocol     |  <-- types only
                    |  Types                 |
                    |  (Task, TaskStatus,    |
                    |   CreateTaskResult,    |
                    |   capabilities, etc.)  |
                    +------------------------+
```

### Component Boundaries

| Component | Responsibility | Communicates With | Crate/Module |
|-----------|---------------|-------------------|--------------|
| **Protocol Types** (Layer 1) | Serde-serializable types matching MCP 2025-11-25 spec. Task, TaskStatus, CreateTaskResult, TaskParams, capabilities, error codes. Zero business logic. | All other layers (data structures only) | `pmcp-tasks/src/types.rs`, `capabilities.rs`, `error.rs` |
| **TaskStore trait** (Layer 2) | Async trait defining CRUD + state machine operations on tasks. Owns the contract for atomic status transitions, owner enforcement, TTL, and variable merge semantics. | Implemented by backends; consumed by TaskContext and TaskHandler | `pmcp-tasks/src/store.rs` |
| **TaskContext** (Layer 3) | Ergonomic handle passed to tool handlers. Wraps `Arc<dyn TaskStore>` with a pinned `task_id` and `owner_id`. Exposes `get_variable`, `set_variable`, `require_input`, `fail`, `complete`. | TaskStore (reads/writes), tool handlers (passed to them) | `pmcp-tasks/src/context.rs` |
| **InMemoryTaskStore** (Layer 4a) | `HashMap<String, TaskRecord>` behind `parking_lot::RwLock`. Validates state transitions inline. Lazy TTL cleanup on read. | TaskStore trait (implements it) | `pmcp-tasks/src/backends/memory.rs` |
| **DynamoDbTaskStore** (Layer 4b) | DynamoDB client with conditional writes for atomic transitions. Uses native DynamoDB TTL + read-time expiry check. GSI for owner-scoped listing. | TaskStore trait (implements it), AWS SDK | `pmcp-tasks/src/backends/dynamodb.rs` |
| **TaskHandler** | Routes `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel` JSON-RPC methods. Resolves owner ID from `RequestHandlerExtra`. Delegates to TaskStore. | TaskStore, RequestHandlerExtra (owner resolution), ServerCore (registered as handler) | `pmcp-tasks/src/handler.rs` |
| **TaskMiddleware** | `ToolMiddleware` implementation that intercepts `tools/call` requests containing `params.task`. Creates task in store, returns `CreateTaskResult` immediately, spawns background execution. | TaskStore, ToolMiddlewareChain (registered into), RequestHandlerExtra (owner + metadata injection) | `pmcp-tasks/src/middleware.rs` |
| **TaskSecurityConfig** | Configuration struct: max tasks per owner, max TTL, default TTL, allow_anonymous. Validated at construction time. | TaskStore backends (enforced there), ServerCoreBuilder (configured there) | `pmcp-tasks/src/security.rs` |

### Data Flow

#### Flow 1: Task-Augmented tools/call (create + background execute)

```
Client                          ServerCore                    TaskMiddleware              TaskStore         ToolHandler
  |                                |                              |                        |                 |
  |-- tools/call {task: {ttl}} --> |                              |                        |                 |
  |                                |-- process_request ---------> |                        |                 |
  |                                |                              |-- resolve_owner_id()   |                 |
  |                                |                              |-- store.create() ----> |                 |
  |                                |                              |<-- TaskRecord --------- |                 |
  |                                |                              |                        |                 |
  |                                |                              |   [return CreateTaskResult immediately]   |
  |<---- CreateTaskResult ---------|<-----------------------------|                        |                 |
  |                                |                              |                        |                 |
  |                                |                              |-- tokio::spawn ------->|                 |
  |                                |                              |   [background task]    |                 |
  |                                |                              |                        |-- call_tool --> |
  |                                |                              |                        |                 |
  |                                |                              |                        |   [tool uses    |
  |                                |                              |                        |    TaskContext   |
  |                                |                              |                        |    to r/w vars]  |
  |                                |                              |                        |                 |
  |                                |                              |                        |<-- result ------|
  |                                |                              |                        |                 |
  |                                |                              |-- store.set_result --> |                 |
  |                                |                              |-- store.update_status  |                 |
  |                                |                              |   (completed/failed)   |                 |
```

**Key design decision:** The middleware returns `CreateTaskResult` synchronously and spawns a `tokio::spawn` for the actual tool execution. This decouples the response time from tool execution time. The spawned task holds an `Arc<dyn TaskStore>` and writes the result when complete.

#### Flow 2: Client polls with tasks/get

```
Client                          ServerCore                    TaskHandler               TaskStore
  |                                |                              |                        |
  |-- tasks/get {taskId} -------> |                              |                        |
  |                                |-- route to TaskHandler ----> |                        |
  |                                |                              |-- resolve_owner_id()   |
  |                                |                              |-- store.get() -------> |
  |                                |                              |<-- TaskRecord --------- |
  |                                |                              |-- convert to Task      |
  |<---- Task (status, etc.) -----|<-----------------------------|                        |
```

#### Flow 3: Client retrieves result with tasks/result

```
Client                          ServerCore                    TaskHandler               TaskStore
  |                                |                              |                        |
  |-- tasks/result {taskId} ----> |                              |                        |
  |                                |-- route to TaskHandler ----> |                        |
  |                                |                              |-- resolve_owner_id()   |
  |                                |                              |-- store.get_result() -> |
  |                                |                              |<-- Value (or NotReady) |
  |                                |                              |                        |
  |                                |                              |   [if NotReady: block  |
  |                                |                              |    until terminal, per  |
  |                                |                              |    spec requirement]    |
  |                                |                              |                        |
  |<---- CallToolResult + meta ---|<-----------------------------|                        |
```

**Spec note:** `tasks/result` MUST block until terminal status. For Lambda (stateless), this means polling-only behavior with immediate return and `NotReady` error. For long-running servers, the handler can await a `tokio::sync::watch` channel that the background task signals on completion.

#### Flow 4: Task variable accumulation across workflow steps

```
Workflow Step 1                    TaskContext                   TaskStore
  |                                   |                            |
  |-- ctx.set_variable("region", ...) |                            |
  |                                   |-- store.set_variables() -> |
  |                                   |                            |

Workflow Step 2                    TaskContext                   TaskStore
  |                                   |                            |
  |-- ctx.get_variable("region") ---> |                            |
  |                                   |-- store.get() -----------> |
  |                                   |<-- record.variables ------- |
  |<-- Some("us-east-1") ------------|                            |
  |                                   |                            |
  |-- ctx.set_variable("vpc_id", ..)  |                            |
  |                                   |-- store.set_variables() -> |
```

### Integration Points with Existing SDK

#### 1. ServerCoreBuilder: Adding task support

The builder gains new methods, but existing API surface is unchanged:

```rust
// New builder method (additive, non-breaking)
ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("slow_analysis", AnalysisTool)
    .task_store(Arc::new(InMemoryTaskStore::new()))      // NEW
    .task_security(TaskSecurityConfig::default())          // NEW
    .build()
```

Internally, `task_store` does three things:
1. Creates a `TaskMiddleware` and adds it to the `ToolMiddlewareChain` at priority 5 (before auth at 10)
2. Creates a `TaskHandler` and registers routes for `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`
3. Adds task capabilities to `ServerCapabilities.experimental` (or `.tasks` post-stabilization)

#### 2. ClientRequest enum: Routing new methods

The `ClientRequest` enum in `src/types/protocol.rs` needs four new variants:

```rust
pub enum ClientRequest {
    // ... existing variants ...
    #[serde(rename = "tasks/get")]
    TaskGet(TaskGetParams),
    #[serde(rename = "tasks/result")]
    TaskResult(TaskResultParams),
    #[serde(rename = "tasks/list")]
    TaskList(TaskListParams),
    #[serde(rename = "tasks/cancel")]
    TaskCancel(TaskCancelParams),
}
```

**This is the one place where the core pmcp crate MUST change.** The `ClientRequest` enum must know how to deserialize these methods. The actual handling logic stays in `pmcp-tasks`.

**Alternative (to avoid core changes):** Use `serde_json::Value` catch-all with `#[serde(other)]` in the enum, and route unknown methods through a generic handler extension point. This keeps the enum stable but adds complexity. The design doc favors direct enum variants because the task methods are part of the official MCP spec, not a custom extension.

#### 3. ToolMiddleware: Where TaskMiddleware plugs in

The existing `ToolMiddleware` trait already has the right shape:

```rust
#[async_trait]
pub trait ToolMiddleware: Send + Sync {
    async fn on_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()>;
    // ...
}
```

`TaskMiddleware` implements this. In `on_request`, it checks `args["task"]`. If present, it:
1. Extracts `TaskParams` from `args.task`
2. Creates a task in the store
3. Injects `TaskContext` into `extra.metadata` (serialized as a metadata key)
4. Returns `Err` with a special "short-circuit" signal that the middleware executor interprets as "return this response instead of calling the tool"

**Problem:** The current `ToolMiddleware::on_request` returns `Result<()>`. There is no mechanism for a middleware to short-circuit with a custom response value. This needs a design decision:

**Option A (recommended):** Add a `MiddlewareResult` enum:
```rust
pub enum MiddlewareAction {
    Continue,                      // proceed to next middleware / tool
    ShortCircuit(Value),           // return this value as the response
}
```

**Option B:** Have the middleware spawn the background task, set the result in `extra.metadata`, and have the core check for it after middleware completes.

**Option C:** Don't use the middleware chain at all. Instead, check for `task` in `ServerCore::handle_call_tool` before dispatching to the tool handler, and delegate to `TaskHandler` if present.

Recommendation: **Option C** for initial implementation. It is simpler, avoids middleware trait changes, and keeps the task interception logic co-located. The design doc's `TaskMiddleware` concept is the logical boundary; the implementation mechanism can be a direct check in `handle_call_tool` rather than a formal `ToolMiddleware` impl. Revisit middleware integration once the base works.

#### 4. RequestHandlerExtra: Owner resolution

Owner ID resolution uses fields already present on `RequestHandlerExtra`:

```rust
pub fn resolve_owner_id(extra: &RequestHandlerExtra) -> String {
    // 1. OAuth subject (highest trust)
    if let Some(auth_ctx) = &extra.auth_context {
        return auth_ctx.subject.clone();
    }
    // 2. Client ID from auth info
    if let Some(auth_info) = &extra.auth_info {
        if let Some(client_id) = &auth_info.client_id {
            return client_id.clone();
        }
    }
    // 3. Session ID (fallback)
    extra.session_id.clone().unwrap_or_else(|| "anonymous".to_string())
}
```

No changes to `RequestHandlerExtra` required. The auth context fields (`auth_context`, `auth_info`, `session_id`) already carry what we need.

## Patterns to Follow

### Pattern 1: Layered Dependency Inversion

**What:** Each layer depends only on the layer below it via traits, never on concrete implementations. Layer 1 (types) has zero dependencies on business logic. Layer 2 (store trait) defines the contract. Layer 3 (context) consumes the trait. Layer 4 (backends) implements the trait.

**When:** Always, for all components in `pmcp-tasks`.

**Why:** This is the standard Rust crate layering pattern (used by hashbrown, tauri-core, embedded-storage). It enables:
- Testing with mock stores
- Swapping backends without changing handler code
- Compiling without optional backends (feature flags work cleanly)

**Example:**
```rust
// Layer 2: trait (no concrete backend knowledge)
#[async_trait]
pub trait TaskStore: Send + Sync {
    async fn create(&self, owner_id: &str, ...) -> Result<TaskRecord, TaskError>;
}

// Layer 3: consumes trait (no concrete backend knowledge)
pub struct TaskContext {
    store: Arc<dyn TaskStore>,  // dynamic dispatch, backend-agnostic
    task_id: String,
    owner_id: String,
}

// Layer 4: implements trait
pub struct InMemoryTaskStore { ... }
impl TaskStore for InMemoryTaskStore { ... }
```

### Pattern 2: State Machine Enforcement at the Store Level

**What:** The `TaskStore` trait contract requires atomic status transitions with validation. Backends enforce the state machine invariant, not callers.

**When:** Every status update.

**Why:** Prevents invalid transitions regardless of how many code paths update status. DynamoDB uses `ConditionExpression` for atomicity; in-memory uses lock-protected checks.

**Example:**
```rust
// Store validates internally - caller just says "go to completed"
async fn update_status(&self, task_id: &str, owner_id: &str,
    new_status: TaskStatus, message: Option<String>
) -> Result<TaskRecord, TaskError> {
    let record = self.get_internal(task_id)?;
    if !record.task.status.can_transition_to(&new_status) {
        return Err(TaskError::InvalidTransition {
            from: record.task.status,
            to: new_status,
        });
    }
    // ... perform update atomically
}
```

### Pattern 3: Owner Binding as a Store-Level Concern

**What:** Owner enforcement is baked into the store trait. Every operation takes `owner_id`. The store is responsible for rejecting cross-owner access.

**When:** Every store operation.

**Why:** Security invariant cannot be bypassed by caller error. DynamoDB uses the sort key `OWNER#{owner_id}` so a wrong-owner `GetItem` simply returns no item (treated as NotFound). In-memory store checks explicitly.

### Pattern 4: Feature-Gated Backends

**What:** Optional backends (DynamoDB) are behind Cargo feature flags. The core crate compiles with zero optional dependencies by default.

**When:** Any backend with heavy dependencies (AWS SDK, Redis, etc.).

**Example:**
```toml
[features]
default = []
dynamodb = ["aws-sdk-dynamodb", "aws-config"]
```

```rust
#[cfg(feature = "dynamodb")]
pub mod dynamodb;
```

### Pattern 5: Builder Integration via Extension Trait

**What:** Rather than modifying `ServerCoreBuilder` directly, `pmcp-tasks` provides an extension trait that adds task-specific builder methods.

**When:** Integrating the separate crate with the main SDK's builder.

**Why:** Keeps `pmcp-tasks` changes self-contained. The main crate only needs to conditionally use the extension.

**Example:**
```rust
// In pmcp-tasks crate
pub trait TaskBuilderExt {
    fn task_store(self, store: Arc<dyn TaskStore>) -> Self;
    fn task_security(self, config: TaskSecurityConfig) -> Self;
}

// In pmcp crate (behind feature flag)
#[cfg(feature = "tasks")]
impl TaskBuilderExt for ServerCoreBuilder {
    fn task_store(self, store: Arc<dyn TaskStore>) -> Self { ... }
    fn task_security(self, config: TaskSecurityConfig) -> Self { ... }
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Leaking Store Implementation Details into Handlers

**What:** Tool handlers directly importing `DynamoDbTaskStore` or checking backend-specific behavior.

**Why bad:** Breaks portability. Tool handlers work with `TaskContext` only. They must never know or care which backend is in use.

**Instead:** Tool handlers receive `Option<TaskContext>` and interact exclusively through its methods. Backend selection happens at server construction time.

### Anti-Pattern 2: Checking Task Status in Tool Handlers

**What:** Tool handlers polling their own task's status or trying to manage the task lifecycle directly.

**Why bad:** The middleware/handler manages lifecycle. Tool handlers just do their work and optionally call `ctx.set_variable()`, `ctx.require_input()`, or `ctx.fail()`. The middleware wraps the tool result in the proper task lifecycle.

**Instead:** Tool handlers return their result normally. The middleware/handler catches the result and stores it, then transitions the task to `completed` or `failed`.

### Anti-Pattern 3: Blocking the Middleware Chain for Background Tasks

**What:** Having `TaskMiddleware::on_request` await the entire tool execution before returning.

**Why bad:** Defeats the purpose of tasks. The spec requires immediate return of `CreateTaskResult`.

**Instead:** Create the task record, return immediately with `CreateTaskResult`, and `tokio::spawn` the actual tool execution.

### Anti-Pattern 4: Coupling Types to Storage

**What:** Putting DynamoDB attribute annotations or storage-specific serialization on the protocol types.

**Why bad:** Layer 1 types are pure spec representations. They serialize to JSON for the wire protocol. Storage serialization is a separate concern.

**Instead:** Use `TaskRecord` (a store-level struct) that contains a `Task` plus storage-specific fields (`owner_id`, `result`, `variables`). Backends convert between `TaskRecord` and their storage format.

### Anti-Pattern 5: Variable Size Enforcement in the Context

**What:** Checking variable sizes in `TaskContext` rather than in the store.

**Why bad:** Variable size limits are a security/resource concern that must be enforced consistently regardless of access path. If someone calls `store.set_variables()` directly (e.g., in tests or internal code), the limit must still be enforced.

**Instead:** Enforce variable size limits in the `TaskStore` trait (as a documented requirement) and in each backend implementation.

## Build Order (Dependency Chain)

The layers form a strict dependency chain that determines build order. Each layer can be implemented and tested independently once its dependencies exist.

```
Phase 1: Types + Error (zero business deps)
    |
    v
Phase 2: TaskStore trait + TaskRecord (depends on types)
    |
    v
Phase 3: InMemoryTaskStore (depends on store trait)
    |     (this gives us a testable backend immediately)
    |
    +-- Phase 4a: TaskContext (depends on store trait)
    |     (can test against InMemoryTaskStore)
    |
    +-- Phase 4b: TaskSecurityConfig (depends on types)
    |
    v
Phase 5: TaskHandler + TaskMiddleware (depends on context, store, security)
    |     (integration with ServerCore routing)
    |
    v
Phase 6: DynamoDB backend (depends on store trait, feature-gated)
    |     (independent of handler/middleware)
    |
    v
Phase 7: Workflow integration (depends on handler, context)
    |     (connects SequentialWorkflow to TaskContext)
    |
    v
Phase 8: Examples + docs (depends on everything)
```

**Critical dependency:** Phase 5 (handler/middleware) requires changes to the core `pmcp` crate (`ClientRequest` enum + `ServerCore` routing + `ServerCoreBuilder`). This is the highest-risk integration point and should be designed carefully before implementation begins.

**Parallelizable:** Phases 4a and 4b can run in parallel. Phase 6 (DynamoDB) can run in parallel with Phase 7 (workflow integration) since both only depend on the store trait.

## Scalability Considerations

| Concern | Single-process (dev) | Lambda/Serverless | Multi-instance (ECS/K8s) |
|---------|---------------------|-------------------|--------------------------|
| Task storage | InMemoryTaskStore, tasks lost on restart | DynamoDbTaskStore, tasks persist across invocations | DynamoDbTaskStore, shared state across instances |
| Concurrency | `parking_lot::RwLock` (no contention in dev) | Single-threaded Lambda, no lock contention | DynamoDB conditional writes prevent races |
| TTL cleanup | Lazy on read + periodic timer | DynamoDB native TTL (async ~48h) + read-time check | DynamoDB native TTL + read-time check |
| Owner isolation | Session ID (single user) | OAuth sub from API Gateway | OAuth sub from load balancer |
| Result blocking (`tasks/result`) | `tokio::sync::watch` channel | Immediate return (polling-only) | DynamoDB polling loop or SNS notification |
| Task listing pagination | In-memory cursor (offset) | DynamoDB GSI `ExclusiveStartKey` | DynamoDB GSI `ExclusiveStartKey` |

## Sources

- [MCP Tasks Specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks) - Official spec, HIGH confidence
- [MCP Spec Anniversary Blog Post](http://blog.modelcontextprotocol.io/posts/2025-11-25-first-mcp-anniversary/) - Context on experimental status
- [WorkOS MCP 2025-11-25 Analysis](https://workos.com/blog/mcp-2025-11-25-spec-update) - Ecosystem context
- [Agnost: Long Running Tasks in MCP](https://agnost.ai/blog/long-running-tasks-mcp/) - Implementation patterns
- Existing PMCP SDK codebase analysis (traits.rs, tool_middleware.rs, cancellation.rs, workflow/, core.rs, builder.rs) - HIGH confidence
- PMCP Tasks Design Document (`docs/design/tasks-feature-design.md`) - HIGH confidence
- [Layered DDD in Rust](https://leapcell.io/blog/crafting-maintainable-rust-web-apps-with-layered-ddd) - Architecture pattern reference
- [Tauri Core Crates Architecture](https://deepwiki.com/tauri-apps/tauri/9.3-core-crates-overview) - Multi-crate layering reference
