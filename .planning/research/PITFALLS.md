# Pitfalls Research

**Domain:** Async durable task/state machine system integrated into existing Rust protocol SDK (MCP Tasks for PMCP)
**Researched:** 2026-02-21
**Confidence:** HIGH (combination of codebase analysis, official docs, and well-documented Rust async ecosystem issues)

## Critical Pitfalls

### Pitfall 1: Non-Atomic `complete()` Creates a Window for Inconsistent Task State

**What goes wrong:**
The design document's `TaskContext::complete()` method performs two sequential store operations: `update_status(Completed)` followed by `set_result(result)`. If the process crashes, the network fails, or the Lambda invocation times out between these two calls, the task is permanently stuck in `Completed` status with no result. Clients polling `tasks/result` will get `NotReady` forever on a task that claims to be `Completed`.

**Why it happens:**
Developers naturally decompose "complete with result" into "set status" + "set result" because those are two conceptual operations and the `TaskStore` trait separates them. The two-step approach feels clean from an API perspective but violates the atomicity requirement of state machine transitions with associated data.

**How to avoid:**
Add a `complete_with_result(task_id, owner_id, result) -> Result<TaskRecord>` method to the `TaskStore` trait that atomically sets status to `Completed` AND stores the result in a single operation. For in-memory: hold the write lock across both mutations. For DynamoDB: use a single `UpdateItem` that sets both `#status = :completed` AND `#result = :result_json` with a `ConditionExpression` validating the source status. Keep `update_status` and `set_result` as separate methods for other use cases, but make `complete()` on `TaskContext` use the atomic variant.

**Warning signs:**
- Integration tests that never test crash recovery between status update and result storage
- `tasks/result` returning `NotReady` for tasks showing `Completed` in `tasks/get`
- DynamoDB items with `status=completed` but no `result` attribute

**Phase to address:**
Phase 1 (Core Types + TaskStore trait design). Must be part of the trait definition before any backend implements it.

---

### Pitfall 2: Holding `parking_lot::RwLock` Across `.await` Points Causes Deadlocks

**What goes wrong:**
The existing codebase uses `parking_lot::RwLock` extensively (per MEMORY.md). The in-memory `TaskStore` design uses `parking_lot::RwLock<HashMap<String, TaskRecord>>`. If any `TaskStore` method implementation holds a `parking_lot` lock guard and then `.await`s (e.g., for logging, metrics, or calling another async method), the synchronous lock blocks the Tokio worker thread. Under load, this starves the thread pool and causes cascading deadlocks because other tasks waiting for the same lock can never make progress -- their executor threads are blocked.

**Why it happens:**
`parking_lot::RwLock` guards implement `Send`, so Rust does not emit a compiler error when they are held across `.await` points. The code compiles and works under low concurrency, but deadlocks under load. This is especially insidious because the in-memory backend will primarily be used in tests and development, where concurrency is low, and the bug only manifests in production-like stress scenarios.

**How to avoid:**
For the `InMemoryTaskStore`, use `parking_lot::RwLock` but scope all lock acquisitions to synchronous blocks. Each async method should: (1) acquire the lock, (2) read/mutate the HashMap synchronously, (3) release the lock, (4) then perform any async work. The pattern is:

```rust
async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
    // Lock is acquired and released synchronously -- no .await while held
    let record = {
        let tasks = self.tasks.read();
        tasks.get(task_id).cloned()
    };
    // Now safe to do async work if needed
    record.ok_or(TaskError::NotFound)
}
```

Alternatively, use `tokio::sync::RwLock` if any method needs to hold the lock across async operations, but this is slower for the common case. Given that the in-memory store operations are purely synchronous (HashMap lookups), `parking_lot` is the right choice -- just enforce the scoping discipline.

**Warning signs:**
- Lock guards stored in local variables that span across `.await` points
- `clippy::await_holding_lock` lint being suppressed
- Stress tests hanging under concurrent task operations

**Phase to address:**
Phase 2 (In-Memory Backend implementation). Add `#![deny(clippy::await_holding_lock)]` to the `pmcp-tasks` crate root.

---

### Pitfall 3: `serde(flatten)` on `TaskWithVariables` Silently Eats Unknown Fields and Breaks Forward Compatibility

**What goes wrong:**
The design uses `#[serde(flatten)]` on `TaskWithVariables` to merge standard `Task` fields with the `variables` extension. When the MCP spec adds new fields to the Task type (which is expected -- the spec is experimental), those unknown fields get silently swallowed into the `variables` HashMap instead of being preserved or causing a clear error. A new spec field like `priority: "high"` would appear as a task variable instead of a protocol field, corrupting both the variable namespace and the protocol semantics.

**Why it happens:**
`serde(flatten)` with a HashMap catch-all is a known footgun in Rust serialization. It is convenient for extensibility but creates ambiguity: any field not explicitly defined in the struct is captured by the HashMap. When the spec adds fields, the Rust struct lags behind, and the new fields silently become "variables." This is documented as a known issue in serde (serde-rs/serde#1346).

**How to avoid:**
Do NOT use `#[serde(flatten)]` on `TaskWithVariables`. Instead, make `TaskWithVariables` contain a `Task` as a named field and serialize/deserialize explicitly. Variables should live in a dedicated `_meta.pmcp:variables` namespace in the wire format, not at the top level. This matches the design doc's intent ("Variables are surfaced to the client through `_meta`") but the Rust type design contradicts it. The wire format should be:

```json
{
  "taskId": "...",
  "status": "working",
  "_meta": {
    "pmcp:variables": { "region": "us-east-1" }
  }
}
```

Not:
```json
{
  "taskId": "...",
  "status": "working",
  "variables": { "region": "us-east-1" }
}
```

This keeps protocol fields and PMCP extensions cleanly separated and forward-compatible.

**Warning signs:**
- Deserialization tests that only test the current spec fields
- No test for "unknown fields in Task JSON are preserved, not captured as variables"
- Variables appearing that nobody set (they are actually new spec fields)

**Phase to address:**
Phase 1 (Core Protocol Types). Must be correct before any serialization tests are written.

---

### Pitfall 4: Background Task Execution Orphans When Lambda/Server Shuts Down

**What goes wrong:**
The `TaskMiddleware` design spawns background execution of tool handlers via `tokio::spawn` after returning `CreateTaskResult` to the client. In Lambda, the runtime freezes after the response is sent, killing the spawned task. In long-running servers, if the server shuts down gracefully, `tokio::spawn`ed tasks are detached (not cancelled) when the `JoinHandle` is dropped. The task's status remains `Working` forever with no process executing it -- a zombie task.

**Why it happens:**
Developers assume `tokio::spawn` tasks run to completion, but (a) Lambda freezes the process after the HTTP response, (b) dropping a `JoinHandle` detaches the task silently (no cancellation), and (c) there is no built-in mechanism to track or await spawned background tasks during shutdown. The MCP design makes this worse because the middleware MUST return `CreateTaskResult` immediately, creating a forced split between "respond" and "execute."

**How to avoid:**
For Lambda: Do NOT spawn background tasks. Instead, use a two-invocation pattern: (1) first invocation creates the task and enqueues the work (e.g., via SQS or a DynamoDB work item), (2) second invocation (triggered by SQS/DynamoDB Stream) performs the actual work. This matches Lambda's execution model. For long-running servers: maintain a `TaskSet` (from `tokio::task::JoinSet`) that tracks all spawned task executions, and drain it during graceful shutdown. Register abort handles so cancellation propagates. Add a `task_runner` component that owns the `JoinSet` and provides `shutdown()`:

```rust
pub struct TaskRunner {
    tasks: JoinSet<()>,
    shutdown_signal: CancellationToken,
}

impl TaskRunner {
    pub async fn shutdown(&mut self) {
        self.shutdown_signal.cancel();
        while let Some(result) = self.tasks.join_next().await {
            if let Err(e) = result {
                tracing::warn!("Task failed during shutdown: {}", e);
            }
        }
    }
}
```

**Warning signs:**
- Tasks stuck in `Working` status after server restart
- Lambda invocations timing out without the background work completing
- No `JoinSet` or equivalent tracking mechanism for spawned tasks
- Tests that do not simulate process interruption during task execution

**Phase to address:**
Phase 3 (TaskMiddleware + Server Integration). Design the execution model BEFORE implementing the middleware. Lambda vs long-running server should be a configuration choice, not an afterthought.

---

### Pitfall 5: DynamoDB Conditional Write Retries Silently Violate State Machine Invariants

**What goes wrong:**
DynamoDB conditional writes use `ConditionExpression` to enforce valid state transitions (e.g., `#status IN (:working)` when transitioning to `completed`). When a `ConditionalCheckFailedException` occurs, the AWS SDK's default retry policy may retry the request. If the condition failed because another writer already moved the task to a different state, retrying is incorrect -- the state has legitimately changed. Worse, the `ReturnValuesOnConditionCheckFailure` parameter (which returns the current item on failure) is not used by default, so the error gives no information about the actual current state.

**Why it happens:**
The AWS SDK for Rust has built-in retry logic that treats `ConditionalCheckFailedException` as a retryable error by default. Developers who implement conditional writes without customizing retry behavior get silent retries that mask the true failure reason. Additionally, many implementations catch `ConditionalCheckFailedException` and return a generic "invalid transition" error without inspecting what the current state actually is, making debugging impossible.

**How to avoid:**
(1) Disable automatic retries for conditional write operations or configure the retry policy to NOT retry `ConditionalCheckFailedException`. (2) Always use `ReturnValuesOnConditionCheckFailure::ALL_OLD` to get the actual item state when the condition fails. (3) Map the DynamoDB error to a specific `TaskError::InvalidTransition { current_status, attempted_status }` that tells the caller exactly why the transition failed:

```rust
.return_values_on_condition_check_failure(ReturnValuesOnConditionCheckFailure::AllOld)
```

Then in the error handler:
```rust
Err(SdkError::ServiceError(err)) if err.err().is_conditional_check_failed_exception() => {
    let current = err.err().item().and_then(|item| extract_status(item));
    Err(TaskError::InvalidTransition {
        task_id: task_id.to_string(),
        current_status: current,
        attempted_status: new_status,
    })
}
```

**Warning signs:**
- Generic "conditional check failed" errors in logs with no state context
- Status transitions succeeding when they should not (due to retries)
- DynamoDB `ConditionalCheckFailedException` counts higher than expected in CloudWatch

**Phase to address:**
Phase 4 (DynamoDB Backend). Must be addressed during DynamoDB `TaskStore` implementation, not retrofitted.

---

### Pitfall 6: Spec Drift Couples Experimental Crate to Unstable Protocol Surface

**What goes wrong:**
The MCP Tasks spec is experimental (2025-11-25). The protocol has already had multiple breaking changes (batching removed in June 2025, auth overhauled in March 2025). The `pmcp-tasks` crate types are a 1:1 mapping of the spec schema. When the spec changes (field renames, new required fields, removed concepts), every type, every serialization test, every integration test, and every example breaks simultaneously. If the crate has been published to crates.io, downstream users face a cascade of breaking changes.

**Why it happens:**
Mapping protocol types 1:1 feels correct (spec compliance), but it couples the crate's API surface directly to an unstable external specification. Each spec revision becomes a semver-breaking release of the crate. The design doc acknowledges this risk ("Migration & Compatibility" section) but the mitigation is vague ("migration guide").

**How to avoid:**
(1) Keep `pmcp-tasks` at `0.x` semver to signal instability -- never publish `1.0` while the spec is experimental. (2) Add an internal abstraction layer: protocol types (`wire::Task`) are separate from domain types (`Task`). Wire types handle serialization/deserialization and change with the spec. Domain types are what the SDK user interacts with and change less frequently. Conversion between them is explicit. (3) Version-gate the wire types: `mod wire_2025_11_25` so multiple spec versions can coexist during migration. (4) Pin the spec version in the crate metadata and CI: test against a specific schema revision, not "latest."

**Warning signs:**
- Crate version `1.x` while the spec is still experimental
- Wire types and domain types are the same structs
- No spec version pinned in tests or documentation
- Users importing `pmcp_tasks::Task` and using it for both serialization and business logic

**Phase to address:**
Phase 1 (Core Protocol Types). The type layering decision must be made before any types are published.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using `HashMap<String, String>` for `RequestHandlerExtra::metadata` to pass `TaskContext` | No new types needed, fits existing API | Type safety lost, metadata key collisions with other middleware, no compile-time guarantee that TaskContext is present | Never -- add `Option<TaskContext>` as a proper field on `RequestHandlerExtra` or use a typed extension map |
| Skipping variable size validation in in-memory store | Faster implementation, "it's just for dev" | In-memory and DynamoDB backends behave differently. Code tested against in-memory passes with 1MB variables, then fails in production against DynamoDB's 400KB limit | Only acceptable if in-memory store enforces the same configurable limit as DynamoDB |
| Single `TaskStore` trait with all methods required | Simple trait, one implementation per backend | Backends that do not support all operations (e.g., a read-only cache layer, or a store that does not support `cleanup_expired`) must provide dummy implementations | Never -- split into `TaskStore` (core CRUD) and optional `TaskStoreMaintenance` (cleanup) traits |
| Polling-only without exponential backoff guidance | Simpler initial implementation | Clients hammer the server with polls at `pollInterval` even when the task is clearly long-running, wasting DynamoDB read capacity | Acceptable for MVP, but document recommended polling strategies and provide a `poll_interval` that increases over time |

## Integration Gotchas

Common mistakes when connecting to external services and the existing SDK.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| DynamoDB TTL | Relying solely on DynamoDB TTL for expiration enforcement. DynamoDB TTL deletion is eventual (up to 48 hours delay). Tasks appear in `tasks/list` and `tasks/get` long after their TTL expires. | Always check expiration on read: `if now > created_at + ttl { return NotFound }`. Use DynamoDB TTL as background cleanup only, not as the primary enforcement mechanism. |
| Cargo workspace feature unification | Enabling the `dynamodb` feature in `pmcp-tasks` causes `aws-sdk-dynamodb` and `aws-config` to be compiled for ALL workspace members during `cargo test`, even those that do not need it, bloating compile times. | Use `resolver = "2"` in workspace `Cargo.toml` (already standard). Test the `dynamodb` feature with `--package pmcp-tasks --features dynamodb` explicitly, not with `--workspace`. Add a CI job specifically for DynamoDB tests. |
| Existing `ToolMiddleware` trait | The current `ToolMiddleware::on_request` returns `Result<()>`. To short-circuit with a `CreateTaskResult`, the middleware would need to return the result value, but the trait signature does not support this. Forcing it through `Result<Err>` conflates "task created successfully" with "middleware error." | Extend the middleware trait with a `MiddlewareAction` enum: `Continue`, `ShortCircuit(Value)`, or `Error(Error)`. Or add a dedicated `TaskMiddleware` that runs before regular middleware and can intercept the request entirely. |
| `RequestHandlerExtra` metadata for passing `TaskContext` | Using `extra.metadata` (which is `HashMap<String, String>`) to pass a `TaskContext` requires serializing/deserializing the context, losing type safety and adding overhead. | Add `pub task_context: Option<TaskContext>` directly to `RequestHandlerExtra`. This is a field addition (non-breaking for struct construction via builder pattern) and provides compile-time type safety. |
| Owner ID resolution fallback chain | The fallback to `"anonymous"` when no auth context exists creates a shared namespace where all unauthenticated users see each other's tasks. In multi-tenant deployments, this is a data leak. | Default `allow_anonymous` to `false` in `TaskSecurityConfig`. When anonymous is not allowed and no owner can be resolved, return an authentication error rather than using a shared anonymous identity. |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| `InMemoryTaskStore` with unbounded HashMap | Memory usage grows without limit; OOM kills the process | Enforce `max_tasks_per_owner` AND a global `max_total_tasks` limit. Run `cleanup_expired` on a timer (e.g., every 60 seconds), not just lazily on access. | ~10K tasks in memory without cleanup |
| `tasks/list` full table scan on DynamoDB | GSI query returns all tasks for an owner; response grows linearly with task count. Eventually exceeds DynamoDB's 1MB query response limit. | Enforce `limit` parameter with a reasonable default (e.g., 50). Always paginate. Add GSI sort key on `CREATED#timestamp` to enable time-range queries. | ~1K tasks per owner |
| Per-variable `set_variable` calls from handler code | Each `TaskContext::set_variable()` call hits the store (DynamoDB write). A handler setting 10 variables makes 10 DynamoDB writes. | Provide `set_variables(HashMap)` for batch updates. Document that `set_variable` is for convenience/single updates; use `set_variables` for multi-variable updates. Internally buffer within a single handler call. | ~5 variables per tool call at scale |
| DynamoDB `GetItem` on every `TaskContext::get_variable()` | Each variable read is a full item fetch from DynamoDB. Reading 5 variables in a handler makes 5 identical `GetItem` calls. | Cache the `TaskRecord` within `TaskContext` for the duration of a single handler invocation. Invalidate on write. This is safe because a single handler invocation is the only writer during its execution. | Immediately noticeable in Lambda cold starts |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Task ID as sole access control (no owner enforcement) | Anyone who guesses or observes a task ID can read its variables, result, and status. UUIDv4 has 122 bits of entropy but is not a security boundary -- task IDs may appear in logs, URLs, or client-side storage. | Always enforce owner_id matching on every store operation. Task ID is for identification, owner_id is for authorization. Both are required for access. |
| Storing sensitive data in task variables without encryption | Task variables are stored as plaintext JSON in DynamoDB. Variables like API keys, PII, or credentials are visible to anyone with DynamoDB table access (ops, support, compromised IAM roles). | Document that task variables are NOT a secure store. Recommend storing sensitive data in Secrets Manager/Parameter Store and putting only references in variables. Consider adding an optional `EncryptedVariableStore` wrapper that uses KMS envelope encryption. |
| Session ID fallback for owner_id in multi-tenant environments | Session IDs are ephemeral and transport-specific. If the transport reconnects with a new session ID, the user loses access to their tasks. If session IDs are reused (e.g., sticky sessions), tasks leak across users. | Require OAuth `sub` or client ID for multi-tenant deployments. Session ID fallback should only be allowed in single-tenant mode (`TaskSecurityConfig::allow_session_fallback = false` by default). |
| No rate limiting on `tasks/create` | A single client can create thousands of tasks, exhausting DynamoDB write capacity and storage. `max_tasks_per_owner` is enforced per owner, but checking the count requires a query on every create -- which itself consumes read capacity. | Implement token-bucket rate limiting at the middleware layer (before the store). Cache the task count per owner in memory with a TTL. Combine with DynamoDB's provisioned capacity alarms. |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **TaskStore trait:** Often missing atomic `complete_with_result` -- verify that status + result can be set in a single operation
- [ ] **In-memory backend:** Often missing TTL enforcement timer -- verify that `cleanup_expired` is called periodically, not just lazily
- [ ] **DynamoDB backend:** Often missing read-time expiration check -- verify that `get()` filters expired items even before DynamoDB TTL deletes them
- [ ] **Owner isolation:** Often missing owner check on `set_result` and `get_result` -- verify ALL seven store methods enforce owner_id
- [ ] **State machine tests:** Often missing concurrent transition tests -- verify two threads racing to complete the same task (only one should succeed)
- [ ] **Variable merge semantics:** Often missing `Value::Null` deletion behavior -- verify that setting a variable to `null` removes it
- [ ] **Error types:** Often missing `InvalidTransition` with current+attempted states -- verify the error carries enough context for debugging
- [ ] **Middleware integration:** Often missing `ToolMiddleware` return type extension -- verify the middleware can short-circuit with `CreateTaskResult` without using error path
- [ ] **Lambda compatibility:** Often missing test for "task execution completes before Lambda freezes" -- verify the execution model works for serverless
- [ ] **Capability negotiation:** Often missing test for "client does not support tasks" -- verify graceful degradation when client lacks task capabilities

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Non-atomic complete (zombie completed-no-result tasks) | MEDIUM | Write a one-time migration script that scans for `status=completed` tasks with no `result` attribute, and either transitions them to `failed` with a "recovery: result lost" message, or re-runs the original tool call if the request is stored. |
| parking_lot deadlock in production | LOW | Restart the process. Then add `#![deny(clippy::await_holding_lock)]` and fix all violations. No data loss because tasks are in DynamoDB. |
| serde(flatten) field capture (variables contain spec fields) | HIGH | Requires migrating all stored tasks: extract spec fields from `variables`, add them as proper fields, and re-serialize. Requires a schema version bump and migration tooling. |
| Orphaned background tasks (zombies in Working status) | MEDIUM | Add a "stale task reaper" that transitions tasks stuck in `Working` for longer than `max_execution_time` to `Failed` with a "timeout: execution exceeded limit" message. Run as a scheduled Lambda or periodic timer. |
| DynamoDB retry masking state transitions | LOW | Disable retries for conditional writes. Review CloudWatch metrics for `ConditionalCheckFailedException` counts. No data corruption occurs (the condition prevents invalid writes), but the wrong error is returned to clients. |
| Spec drift breaking published crate | HIGH | If types were published as `1.x`, must follow semver and release `2.0` for breaking changes. If `0.x`, can release `0.next` with migration notes. Prevention (staying on `0.x`) is far cheaper than recovery. |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Non-atomic complete | Phase 1: Core Types + TaskStore trait | Trait has `complete_with_result` method; integration test simulates crash between status and result writes |
| parking_lot deadlock | Phase 2: In-Memory Backend | `#![deny(clippy::await_holding_lock)]` in crate root; stress test with 100 concurrent tasks |
| serde(flatten) field capture | Phase 1: Core Protocol Types | Deserialization test: JSON with unknown fields does NOT capture them as variables; variables live in `_meta` |
| Orphaned background tasks | Phase 3: TaskMiddleware + Integration | Lambda execution model test; `JoinSet` tracking for long-running servers; shutdown drains all tasks |
| DynamoDB conditional write retries | Phase 4: DynamoDB Backend | `ConditionalCheckFailedException` is NOT retried; error includes current state via `ReturnValuesOnConditionCheckFailure` |
| Spec drift | Phase 1: Core Protocol Types | Wire types separate from domain types; crate version is `0.x`; spec version pinned in CI |
| Owner isolation gaps | Phase 2: Security Tests | Property test: random owner pairs never cross boundaries on any of the 7 store methods |
| Variable size limits inconsistent | Phase 2: In-Memory Backend | In-memory store enforces same configurable limit as DynamoDB; test that 500KB variable is rejected by both |
| DynamoDB TTL stale reads | Phase 4: DynamoDB Backend | Test that `get()` returns `NotFound` for an expired task even before DynamoDB TTL deletes the item |
| RequestHandlerExtra metadata type safety | Phase 3: Server Integration | `TaskContext` is a typed field on `RequestHandlerExtra`, not smuggled through `HashMap<String, String>` |

## Sources

- [Common mistakes with Async Rust](https://www.elias.sh/posts/common_mistakes_with_async_rust) -- blocking in async, cancellation, lock pitfalls
- [Rust Concurrency: Common Async Pitfalls Explained](https://leapcell.medium.com/rust-concurrency-common-async-pitfalls-explained-8f80d90b9a43) -- runtime lock-in, recursive futures
- [Cancellation and Async State Machines](https://theincredibleholk.org/blog/2023/11/08/cancellation-async-state-machines/) -- drop semantics during state transitions
- [Tokio Shared State tutorial](https://tokio.rs/tokio/tutorial/shared-state) -- parking_lot vs tokio::sync guidance
- [Tokio RwLock Deadlock Mystery](https://www.techedubyte.com/tokio-rwlock-deadlock-blockon-hangs/) -- block_on causing silent hangs
- [parking_lot RwLock docs](https://amanieu.github.io/parking_lot/parking_lot/struct.RwLock.html) -- recursive read lock deadlock warning
- [Understanding DynamoDB Condition Expressions](https://www.alexdebrie.com/posts/dynamodb-condition-expressions/) -- conditional writes for state machines
- [Handle conditional write errors in high concurrency](https://aws.amazon.com/blogs/database/handle-conditional-write-errors-in-high-concurrency-scenarios-with-amazon-dynamodb/) -- ReturnValuesOnConditionCheckFailure pattern
- [Using Improved Conditional Writes in DynamoDB](https://aws.amazon.com/blogs/developer/using-improved-conditional-writes-in-dynamodb/) -- 2024 improvements
- [Large object storage strategies for DynamoDB](https://aws.amazon.com/blogs/database/large-object-storage-strategies-for-amazon-dynamodb/) -- 400KB limit workarounds
- [DynamoDB TTL docs](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/howitworks-ttl.html) -- 48-hour deletion delay
- [Analysis of DynamoDB's TTL delay](https://medium.com/@michabahr/analysis-of-dynamodbs-ttl-delay-de878e2c6d47) -- real-world TTL behavior
- [Cargo Workspace Feature Unification Pitfall](https://nickb.dev/blog/cargo-workspace-and-the-feature-unification-pitfall/) -- feature flag cross-contamination
- [serde flatten issues (serde-rs/serde#1346)](https://github.com/serde-rs/serde/issues/1346) -- flatten + map interaction
- [Rust tokio task cancellation patterns](https://cybernetist.com/2024/04/19/rust-tokio-task-cancellation-patterns/) -- JoinHandle drop behavior
- [tokio-rs/tokio#1830: JoinHandle cancel on drop](https://github.com/tokio-rs/tokio/issues/1830) -- why dropped handles don't cancel
- [MCP 2025-11-25 spec changelog](https://modelcontextprotocol.io/specification/2025-11-25/changelog) -- breaking changes history
- [MCP 2025-11-25 Tasks announcement](https://workos.com/blog/mcp-2025-11-25-spec-update) -- experimental status
- [Google Cloud Tasks common pitfalls](https://cloud.google.com/tasks/docs/common-pitfalls) -- duplicate execution, ordering guarantees
- [async_trait crate docs](https://docs.rs/async-trait) -- Send bounds and dyn compatibility
- [Dyn async traits, part 10](https://smallcultfollowing.com/babysteps/blog/2025/03/24/box-box-box/) -- 2025 language evolution for async traits

---
*Pitfalls research for: MCP Tasks implementation in PMCP SDK (Rust async durable task system)*
*Researched: 2026-02-21*
