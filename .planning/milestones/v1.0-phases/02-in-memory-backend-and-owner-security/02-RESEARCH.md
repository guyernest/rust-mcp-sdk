# Phase 2: In-Memory Backend and Owner Security - Research

**Researched:** 2026-02-21
**Domain:** In-memory concurrent data store, owner isolation security, ergonomic async context
**Confidence:** HIGH

## Summary

Phase 2 builds directly on the Phase 1 foundation (types, store trait, error types) to deliver the first concrete `TaskStore` implementation. The three core deliverables are: (1) `InMemoryTaskStore` implementing the full `TaskStore` async trait with thread-safe concurrent access, (2) `TaskSecurityConfig` enforcing owner isolation, resource limits, and anonymous access controls, and (3) `TaskContext` providing ergonomic typed variable accessors and status transition convenience methods for use inside tool handlers.

The codebase already uses both `parking_lot::RwLock` and `dashmap::DashMap` for concurrency. The primary data structure is a `HashMap<String, TaskRecord>` keyed by task ID, with owner isolation enforced structurally by checking `owner_id` on every operation. The decision to use owner ID as a structural key (returning `NotFound` on mismatch rather than `OwnerMismatch`) means the store internally uses `OwnerMismatch` but the public API surfaces `NotFound` -- this is the "never reveal that a task exists but belongs to someone else" security requirement.

**Primary recommendation:** Use `DashMap<String, TaskRecord>` for the store (consistent with `SessionManager`'s pattern for keyed concurrent access), wrap `TaskSecurityConfig` alongside `StoreConfig` in the store constructor, and implement `TaskContext` as a `Clone + Send + Sync` struct wrapping `Arc<dyn TaskStore>` with task_id and owner_id for Phase 3 handler integration.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Owner ID derived from OAuth token identity (authentication-based, not session-based)
- Owner ID used as part of the store key itself, making cross-user access structurally impossible rather than relying on access control checks
- On owner mismatch: return NotFound (never reveal that a task exists but belongs to someone else)
- No admin bypass -- strict isolation with no exceptions
- No cross-owner listing -- list() always scopes to a single owner
- Variable size limits enforced at the store level (defense in depth)
- Max tasks per owner: hard reject with ResourceExhausted error (no auto-eviction)
- TTL enforcement: reject with error if client requests TTL above configured max (no silent clamping)
- Anonymous access: supported for local single-user servers without OAuth -- use a well-known default owner ID (e.g., "local") when no auth is configured. All tasks belong to this default owner, keeping code paths consistent.
- Typed variable accessors: ctx.get_string("key"), ctx.get_i64("count") with type conversion (not just raw JSON values)
- Scope: tool handler focused -- designed for use inside tool handlers, scoped to a single task lifecycle
- Expired task cleanup: on-demand only -- cleanup_expired() called explicitly, no background task inside the store
- Expiry read behavior: expired-but-not-yet-cleaned-up tasks are still readable with an expiration flag (allows client to retry with longer TTL or different approach). Once cleanup removes them, returns NotFound.

### Claude's Discretion
- Locking strategy (RwLock vs DashMap) -- pick based on codebase conventions
- TaskContext store ownership pattern -- pick based on Phase 3 handler integration needs
- Status transition + result API shape -- pick based on store's complete_with_result semantics

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| STOR-05 | In-memory backend implements TaskStore with HashMap + synchronization | DashMap pattern from SessionManager; all TaskStore methods implemented with owner check + state machine validation |
| STOR-06 | In-memory backend validates state machine transitions atomically | DashMap entry API provides mutable access under lock; validate transition before applying |
| STOR-07 | In-memory backend supports configurable poll interval and max TTL | Builder pattern with `with_poll_interval()` and StoreConfig/TaskSecurityConfig integration |
| HNDL-02 | Task variables surfaced to client via `_meta` in task responses | Already implemented in TaskRecord::to_wire_task_with_variables() from Phase 1; store returns TaskRecord, caller uses the method |
| HNDL-03 | Variable merge semantics: new keys added, existing keys overwritten, null deletes | Implement in set_variables: iterate HashMap, insert non-null, remove on null Value |
| HNDL-04 | TaskContext provides get_variable, set_variable, set_variables, variables methods | TaskContext struct wrapping Arc\<dyn TaskStore\> + task_id + owner_id; plus typed accessors get_string, get_i64, etc. |
| HNDL-05 | TaskContext provides require_input, fail, complete convenience methods for status transitions | Thin wrappers over store.update_status and store.complete_with_result |
| HNDL-06 | TaskContext is Clone and wraps Arc\<dyn TaskStore\> for sharing across async boundaries | Clone derive + Arc\<dyn TaskStore\> field; Send + Sync automatic from Arc |
| SEC-01 | Owner ID resolved from OAuth sub claim, client ID, or session ID (priority order) | OwnerResolver trait or function; uses AuthContext.subject -> client_id -> session_id fallback; "local" default for anonymous |
| SEC-02 | Every task operation enforces owner matching (get, update, cancel, set_variables, set_result) | Every store method takes owner_id param; DashMap lookup validates owner; mismatch returns NotFound |
| SEC-03 | tasks/list scoped to requesting owner only | list() filters by owner_id; DashMap iteration with owner filter |
| SEC-04 | TaskSecurityConfig with configurable max_tasks_per_owner (default: 100) | Struct field with Default impl; enforced in create() by counting owner's tasks |
| SEC-05 | TaskSecurityConfig with configurable max_ttl_ms (default: 24 hours) | Struct field; enforced in create() -- reject if requested TTL > max |
| SEC-06 | TaskSecurityConfig with configurable default_ttl_ms (default: 1 hour) | Struct field; applied in create() when client provides no TTL |
| SEC-07 | TaskSecurityConfig with allow_anonymous toggle (default: false) | Struct field; checked in create()/get()/etc when owner_id is "local" or empty |
| SEC-08 | Task IDs use UUIDv4 (122 bits of entropy) to prevent guessing | Already implemented in TaskRecord::new() using uuid::Uuid::new_v4() |
| TEST-03 | TaskContext behavior tests (variable CRUD, status transitions, complete with result) | Integration tests using InMemoryTaskStore + TaskContext; test typed accessors, null-deletion, transitions |
| TEST-04 | In-memory store tests (CRUD, pagination, TTL, concurrent access) | Unit tests in store module + integration tests; tokio::spawn for concurrency testing |
| TEST-06 | Security tests (owner isolation, anonymous rejection, max tasks enforcement, UUID entropy) | Dedicated security test module; cross-owner access attempts, limit enforcement, entropy checks |
| TEST-07 | Property tests (status transitions, variable merge, task ID uniqueness, owner isolation) | proptest strategies for arbitrary owner IDs, variable maps, status sequences; extend Phase 1 property tests |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `dashmap` | 6.1 | Concurrent HashMap for task storage | Already in workspace Cargo.toml; used by SessionManager and middleware for keyed concurrent access |
| `parking_lot` | 0.12 | Synchronous locking primitives | Already a dependency in pmcp-tasks Cargo.toml; used across the codebase for RwLock |
| `uuid` | 1.17 (v4 feature) | Task ID generation | Already a dependency; 122 bits entropy per SEC-08 |
| `chrono` | 0.4 | Timestamp handling and TTL expiry checks | Already a dependency; used in TaskRecord for expires_at |
| `async-trait` | 0.1 | Async trait support for TaskStore | Already a dependency |
| `serde_json` | 1.0 | Variable values (JSON Value type) | Already a dependency |
| `tokio` | 1.x (sync, time features) | Async runtime, test utilities | Already a dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | 1.7 | Property-based and fuzz testing | Already a dev-dependency; extend Phase 1 patterns |
| `pretty_assertions` | 1.4 | Better test output | Already a dev-dependency |
| `tracing` | 0.1 | Logging for store operations | Already a dependency; use for security-relevant events |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| DashMap | parking_lot::RwLock\<HashMap\> | RwLock is simpler but holds lock during entire iteration for list(); DashMap allows concurrent reads. SessionManager uses DashMap for the same keyed-access pattern. |
| DashMap | tokio::sync::RwLock | Async lock, but unnecessary since store operations are CPU-bound (no I/O in memory backend). Codebase uses parking_lot for sync locks. |

**Recommendation:** Use `DashMap` -- it matches the `SessionManager` pattern (concurrent keyed access to session/task records), avoids holding a global write lock during list/cleanup operations, and is already in the workspace.

## Architecture Patterns

### Recommended Module Structure
```
crates/pmcp-tasks/src/
├── store.rs              # TaskStore trait (Phase 1, exists)
├── store/
│   └── memory.rs         # InMemoryTaskStore implementation (NEW)
├── security.rs           # TaskSecurityConfig, OwnerResolver (NEW)
├── context.rs            # TaskContext ergonomic wrapper (NEW)
├── domain/
│   ├── record.rs         # TaskRecord (Phase 1, exists)
│   └── variables.rs      # TaskWithVariables (Phase 1, exists)
├── error.rs              # TaskError (Phase 1, exists)
├── types/                # Wire types (Phase 1, exists)
└── lib.rs                # Re-exports (extend with new modules)
```

**Alternative structure:** Keep `store.rs` as-is (trait only) and add `store/memory.rs` as a submodule. This maintains separation between the trait definition and implementations. Update `lib.rs` to conditionally export the in-memory backend.

### Pattern 1: DashMap with Owner-Compound Lookup

**What:** Store tasks in a `DashMap<String, TaskRecord>` keyed by task_id. Every get/update/delete operation verifies `record.owner_id == owner_id` after lookup. On mismatch, return `NotFound` (never `OwnerMismatch` to the caller).

**When to use:** All task operations that take a task_id + owner_id pair.

**Why not compound key:** A compound key `(task_id, owner_id)` would make the "task exists but wrong owner" case indistinguishable from "task does not exist" at the DashMap level -- which is exactly the desired behavior. However, it complicates `cleanup_expired()` which needs to iterate all tasks regardless of owner. Using task_id as key and checking owner internally is simpler.

**Example:**
```rust
use dashmap::DashMap;
use crate::domain::TaskRecord;
use crate::error::TaskError;

pub struct InMemoryTaskStore {
    tasks: DashMap<String, TaskRecord>,
    config: StoreConfig,
    security: TaskSecurityConfig,
}

impl InMemoryTaskStore {
    async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        let entry = self.tasks.get(task_id)
            .ok_or_else(|| TaskError::NotFound { task_id: task_id.to_string() })?;

        // Structural owner isolation: mismatch looks like NotFound
        if entry.owner_id != owner_id {
            return Err(TaskError::NotFound { task_id: task_id.to_string() });
        }

        // Expiry check: return record with expiry flag, not error
        // (per CONTEXT.md: expired-but-not-cleaned-up tasks are readable)
        Ok(entry.clone())
    }
}
```

### Pattern 2: TaskSecurityConfig as Separate Struct from StoreConfig

**What:** `TaskSecurityConfig` holds security-specific limits (max_tasks_per_owner, allow_anonymous). `StoreConfig` holds storage-specific limits (max_variable_size_bytes, default_ttl_ms, max_ttl_ms). The InMemoryTaskStore takes both.

**When to use:** When constructing the store. The security config is checked at the store level (defense in depth) even though Phase 3 middleware will also check it.

**Rationale:** StoreConfig already exists from Phase 1 with TTL fields. TaskSecurityConfig adds owner-specific limits. Keeping them separate avoids a breaking change to StoreConfig and maintains separation of concerns.

**Example:**
```rust
pub struct TaskSecurityConfig {
    pub max_tasks_per_owner: usize,
    pub allow_anonymous: bool,
}

impl Default for TaskSecurityConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_owner: 100,
            allow_anonymous: false,
        }
    }
}

// Builder pattern
let store = InMemoryTaskStore::new()
    .with_config(StoreConfig { max_ttl_ms: Some(86_400_000), ..Default::default() })
    .with_security(TaskSecurityConfig { max_tasks_per_owner: 50, ..Default::default() })
    .with_poll_interval(5000);
```

### Pattern 3: TaskContext Wrapping Arc\<dyn TaskStore\>

**What:** `TaskContext` owns `Arc<dyn TaskStore>` plus task_id and owner_id. It is `Clone + Send + Sync` and designed for use inside tool handlers scoped to a single task.

**When to use:** In Phase 3, the TaskMiddleware will construct a `TaskContext` and attach it to `RequestHandlerExtra`. Tool handlers receive it and use the ergonomic API.

**Why Arc ownership:** TaskContext needs to be cloneable (for passing across async boundaries), and it needs the store reference to outlive the handler call. Arc is the standard pattern for this in the codebase.

**Status transition + result API:** Use `complete_with_result` from the store trait for the `complete()` method (atomic status + result), and `update_status` for `fail()` and `require_input()`.

**Example:**
```rust
#[derive(Clone)]
pub struct TaskContext {
    store: Arc<dyn TaskStore>,
    task_id: String,
    owner_id: String,
}

impl TaskContext {
    // Typed variable accessors
    pub async fn get_string(&self, key: &str) -> Result<Option<String>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_str().map(String::from)))
    }

    pub async fn get_i64(&self, key: &str) -> Result<Option<i64>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_i64()))
    }

    pub async fn get_f64(&self, key: &str) -> Result<Option<f64>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_f64()))
    }

    pub async fn get_bool(&self, key: &str) -> Result<Option<bool>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_bool()))
    }

    // Status transitions using store's atomic complete_with_result
    pub async fn complete(&self, result: Value) -> Result<TaskRecord, TaskError> {
        self.store.complete_with_result(
            &self.task_id,
            &self.owner_id,
            TaskStatus::Completed,
            None,
            result,
        ).await
    }

    pub async fn fail(&self, message: impl Into<String>) -> Result<TaskRecord, TaskError> {
        self.store.update_status(
            &self.task_id,
            &self.owner_id,
            TaskStatus::Failed,
            Some(message.into()),
        ).await
    }
}
```

### Pattern 4: Expiry Read Behavior

**What:** Per the user's locked decision, expired-but-not-yet-cleaned-up tasks are still readable. The `TaskRecord::is_expired()` method already exists. The store should return the record even when expired, and let the caller decide how to handle it. Cleanup removes them permanently.

**Implementation approach:** In `get()`, after owner check, check `is_expired()`. If expired, the task is still returned (not an error). The wire response layer (Phase 3) can add an expiration flag. The `cleanup_expired()` method iterates and removes expired records.

**Note:** This deviates from the `TaskStore` trait doc which says `get()` returns `TaskError::Expired`. We need to reconcile: the trait says "Expired" error, but the user decision says "still readable with an expiration flag." **Resolution:** Add an `is_expired` field to the returned TaskRecord or return the record with its `expires_at` intact and let the caller check. The trait's `Expired` error should only be returned by operations that modify state (update_status, set_variables, complete_with_result) -- you cannot modify an expired task.

### Pattern 5: Owner Resolution Function

**What:** A standalone function (not a trait) that extracts owner_id from available identity sources, following the priority order: OAuth sub claim > client ID > session ID > "local" default.

**When to use:** Phase 3 middleware will call this. Phase 2 defines the function and its tests, but the actual wiring happens in Phase 3.

**Example:**
```rust
/// Default owner ID used when no authentication is configured.
/// All tasks for local single-user servers belong to this owner.
pub const DEFAULT_LOCAL_OWNER: &str = "local";

/// Resolve the owner ID from available identity sources.
///
/// Priority order: OAuth subject > client_id > session_id > "local"
pub fn resolve_owner_id(
    auth_context: Option<&AuthContext>,
    session_id: Option<&str>,
) -> String {
    if let Some(auth) = auth_context {
        if auth.authenticated && !auth.subject.is_empty() {
            return auth.subject.clone();
        }
        if let Some(ref client_id) = auth.client_id {
            return client_id.clone();
        }
    }
    if let Some(sid) = session_id {
        return sid.to_string();
    }
    DEFAULT_LOCAL_OWNER.to_string()
}
```

### Anti-Patterns to Avoid

- **Exposing OwnerMismatch to callers:** Never return `TaskError::OwnerMismatch` from the public store API. Always convert to `NotFound`. The `OwnerMismatch` variant exists for internal logging/tracing but must not leak to the wire.
- **Silent TTL clamping:** The user explicitly decided: reject with error, do not silently clamp.
- **Auto-eviction on max tasks:** The user explicitly decided: hard reject with `ResourceExhausted`, do not auto-evict.
- **Background cleanup threads:** The user explicitly decided: on-demand only via `cleanup_expired()`.
- **Holding DashMap refs across await points:** DashMap guards (`Ref`, `RefMut`) are not `Send`. Clone the data before any `.await`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Concurrent HashMap | Custom lock-free map | `dashmap::DashMap` | Battle-tested, already in workspace, handles all edge cases |
| UUID generation | Custom random IDs | `uuid::Uuid::new_v4()` | Already used in Phase 1, provides 122 bits entropy |
| Timestamp handling | Manual epoch math | `chrono::Utc::now()` | Already used in Phase 1, handles timezone and overflow |
| Property testing | Manual randomized tests | `proptest` | Already used in Phase 1, provides shrinking and strategies |
| Typed JSON extraction | Manual match/cast chains | `serde_json::Value` as_str/as_i64/as_f64/as_bool methods | Built-in, handles null/type mismatch gracefully |

**Key insight:** The entire Phase 2 implementation builds on existing dependencies. No new crate dependencies are needed.

## Common Pitfalls

### Pitfall 1: DashMap Guard Lifetime Across Await Points
**What goes wrong:** DashMap's `Ref` and `RefMut` guards implement `Send` only in specific configurations. Holding a guard across an `.await` point causes compiler errors.
**Why it happens:** The `TaskStore` trait is async, and it is tempting to hold a DashMap entry reference while performing validation.
**How to avoid:** Clone the `TaskRecord` from the DashMap entry before any async operation. For mutations, use DashMap's `entry()` API or `alter()` for atomic read-modify-write.
**Warning signs:** Compile error: "future is not Send" mentioning DashMap guard types.

### Pitfall 2: Race Condition in Max Tasks Check
**What goes wrong:** Two concurrent `create()` calls for the same owner both count tasks as under the limit, then both insert, exceeding the limit.
**Why it happens:** The count-then-insert is not atomic with DashMap.
**How to avoid:** Use a per-owner lock or an atomic counter. Simple approach: use `parking_lot::RwLock` wrapping just the count, or accept that the limit is approximate (off-by-one under high concurrency is acceptable for a resource limit). Alternatively, use DashMap's `entry()` API combined with a secondary `DashMap<String, AtomicUsize>` tracking per-owner counts.
**Warning signs:** Tests showing max_tasks_per_owner exceeded by 1-2 under concurrent creation.

### Pitfall 3: Owner Mismatch Leaking Information
**What goes wrong:** Returning different error types (NotFound vs OwnerMismatch) leaks whether a task exists.
**Why it happens:** It is natural to return `OwnerMismatch` for diagnostic purposes.
**How to avoid:** Map `OwnerMismatch` to `NotFound` before returning from any public store method. Use `tracing::warn!` for internal logging of the actual mismatch. The `TaskError::OwnerMismatch` variant is for internal use and security audit logs, never for wire responses.
**Warning signs:** Tests that assert on `OwnerMismatch` from public API methods.

### Pitfall 4: TTL Rejection vs Clamping Confusion
**What goes wrong:** Implementing silent clamping instead of hard rejection for over-max TTL.
**Why it happens:** Clamping is a common pattern and the design doc originally suggested it.
**How to avoid:** The user explicitly decided: reject with an error. Check `requested_ttl > config.max_ttl_ms` and return an error. Do not clamp.
**Warning signs:** Tests that check TTL was adjusted rather than rejected.

### Pitfall 5: Expired Task Read Semantics
**What goes wrong:** Returning `TaskError::Expired` on read, which prevents clients from seeing task state.
**Why it happens:** The TaskStore trait doc says `get()` returns `Expired` error.
**How to avoid:** The user decision overrides: expired tasks are readable with their `is_expired()` flag. Only mutation operations (update_status, set_variables, complete_with_result) should reject expired tasks. The `get()` method returns the record and lets the caller check `is_expired()`. This may require adjusting the trait's documented behavior or having the in-memory impl deviate from the strict trait doc (acceptable since the trait doc is aspirational and the user decision is authoritative).
**Warning signs:** Tests that expect `get()` on an expired task to return `Err(Expired)`.

### Pitfall 6: Variable Size Check Timing
**What goes wrong:** Checking variable size before merge, which allows the merge to exceed the limit.
**Why it happens:** Checking the incoming variables alone, not the merged result.
**How to avoid:** Perform the merge in memory first, compute `serde_json::to_vec(&merged).len()`, compare against `config.max_variable_size_bytes`, then commit or reject.
**Warning signs:** Variable size limit exceeded despite check appearing to pass.

## Code Examples

### Example 1: InMemoryTaskStore Constructor with Builder Pattern
```rust
pub struct InMemoryTaskStore {
    tasks: DashMap<String, TaskRecord>,
    config: StoreConfig,
    security: TaskSecurityConfig,
    default_poll_interval: u64,
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            config: StoreConfig::default(),
            security: TaskSecurityConfig::default(),
            default_poll_interval: 5000,
        }
    }

    pub fn with_config(mut self, config: StoreConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self {
        self.security = security;
        self
    }

    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.default_poll_interval = ms;
        self
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}
```

### Example 2: Atomic State Transition with DashMap
```rust
async fn update_status(
    &self,
    task_id: &str,
    owner_id: &str,
    new_status: TaskStatus,
    status_message: Option<String>,
) -> Result<TaskRecord, TaskError> {
    let mut entry = self.tasks.get_mut(task_id)
        .ok_or_else(|| TaskError::NotFound { task_id: task_id.to_string() })?;

    let record = entry.value_mut();

    // Owner isolation: mismatch = NotFound
    if record.owner_id != owner_id {
        return Err(TaskError::NotFound { task_id: task_id.to_string() });
    }

    // Reject mutations on expired tasks
    if record.is_expired() {
        return Err(TaskError::Expired {
            task_id: task_id.to_string(),
            expired_at: record.expires_at.map(|e| e.to_rfc3339()),
        });
    }

    // Validate state machine transition
    record.task.status.validate_transition(task_id, &new_status)?;

    // Apply transition
    record.task.status = new_status;
    record.task.status_message = status_message;
    record.task.last_updated_at = chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    Ok(record.clone())
}
```

### Example 3: Variable Merge with Null-Deletion
```rust
async fn set_variables(
    &self,
    task_id: &str,
    owner_id: &str,
    variables: HashMap<String, Value>,
) -> Result<TaskRecord, TaskError> {
    let mut entry = self.tasks.get_mut(task_id)
        .ok_or_else(|| TaskError::NotFound { task_id: task_id.to_string() })?;

    let record = entry.value_mut();

    if record.owner_id != owner_id {
        return Err(TaskError::NotFound { task_id: task_id.to_string() });
    }

    if record.is_expired() {
        return Err(TaskError::Expired {
            task_id: task_id.to_string(),
            expired_at: record.expires_at.map(|e| e.to_rfc3339()),
        });
    }

    // Merge: null deletes, non-null upserts
    for (key, value) in variables {
        if value.is_null() {
            record.variables.remove(&key);
        } else {
            record.variables.insert(key, value);
        }
    }

    // Check merged size
    let serialized = serde_json::to_vec(&record.variables)
        .map_err(|e| TaskError::StoreError(e.to_string()))?;
    if serialized.len() > self.config.max_variable_size_bytes {
        // Rollback would be needed here -- or do the merge on a clone first
        return Err(TaskError::VariableSizeExceeded {
            limit_bytes: self.config.max_variable_size_bytes,
            actual_bytes: serialized.len(),
        });
    }

    record.task.last_updated_at = chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    Ok(record.clone())
}
```

**Important note on Example 3:** The size check happens after mutation, which means a rollback is needed on failure. Better approach: clone the variables, perform merge on the clone, check size, then swap if OK.

### Example 4: Owner-Scoped List with Pagination
```rust
async fn list(&self, options: ListTasksOptions) -> Result<TaskPage, TaskError> {
    let mut tasks: Vec<TaskRecord> = self.tasks.iter()
        .filter(|entry| entry.value().owner_id == options.owner_id)
        .map(|entry| entry.value().clone())
        .collect();

    // Sort by creation time, newest first
    tasks.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));

    // Cursor-based pagination (cursor = task_id of last item in previous page)
    let start_idx = if let Some(ref cursor) = options.cursor {
        tasks.iter().position(|t| t.task.task_id == *cursor)
            .map(|i| i + 1)
            .unwrap_or(0)
    } else {
        0
    };

    let limit = options.limit.unwrap_or(50);
    let page_tasks: Vec<TaskRecord> = tasks[start_idx..]
        .iter()
        .take(limit)
        .cloned()
        .collect();

    let next_cursor = if start_idx + limit < tasks.len() {
        page_tasks.last().map(|t| t.task.task_id.clone())
    } else {
        None
    };

    Ok(TaskPage {
        tasks: page_tasks,
        next_cursor,
    })
}
```

### Example 5: TaskContext Typed Variable Accessor Pattern
```rust
impl TaskContext {
    /// Get a variable as a typed value using serde deserialization.
    ///
    /// Returns `None` if the key does not exist or the value cannot
    /// be deserialized to the requested type.
    pub async fn get_typed<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok()))
    }

    /// Convenience: get a string variable.
    pub async fn get_string(&self, key: &str) -> Result<Option<String>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_str().map(String::from)))
    }

    /// Convenience: get an i64 variable.
    pub async fn get_i64(&self, key: &str) -> Result<Option<i64>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_i64()))
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `std::sync::RwLock` | `parking_lot::RwLock` or `DashMap` | Established in codebase | Better performance, no poisoning; DashMap for fine-grained concurrency |
| Manual async trait impls | `async-trait` macro | Stable for years, used throughout codebase | Standard approach until Rust stabilizes native async traits |
| HashMap + RwLock for session store | DashMap for session store | Current codebase pattern | `SessionManager` uses `DashMap<String, Session>` |

**Deprecated/outdated:**
- `std::sync::RwLock`: Not used in this codebase; `parking_lot` is the standard.
- Nightly `cargo-fuzz`: Not required; `proptest` handles both property and fuzz testing without nightly Rust (Phase 1 decision).

## Open Questions

1. **Expired task read semantics reconciliation**
   - What we know: The `TaskStore` trait doc says `get()` returns `TaskError::Expired`. The user decision says expired tasks are readable with an expiration flag.
   - What's unclear: Whether to change the trait's documented behavior or have the in-memory impl use a different semantic.
   - Recommendation: Have `get()` return the `TaskRecord` even when expired (record has `is_expired()` method). Only mutation operations return `Expired` error. Update the trait doc comments accordingly -- this is a Phase 1 trait doc refinement, not a breaking change (no callers exist yet).

2. **Max tasks count atomicity under high concurrency**
   - What we know: DashMap does not provide a way to atomically "count entries matching predicate + insert."
   - What's unclear: Whether off-by-one under extreme concurrency is acceptable.
   - Recommendation: Use an `AtomicUsize` per owner (tracked in a separate `DashMap<String, AtomicUsize>`) for the count. Increment atomically before insert, decrement on failure. This is exact and lock-free. Alternatively, accept approximate enforcement -- the limit is a safety net, not a billing boundary.

3. **Should TaskSecurityConfig live in security.rs or be part of StoreConfig?**
   - What we know: StoreConfig already has `default_ttl_ms` and `max_ttl_ms`. TaskSecurityConfig adds `max_tasks_per_owner` and `allow_anonymous`.
   - What's unclear: Whether merging them simplifies the API.
   - Recommendation: Keep them separate. StoreConfig is storage-oriented (sizes, TTLs). TaskSecurityConfig is security-oriented (access control, rate limits). The store takes both. This avoids a breaking change to StoreConfig and maintains clean separation.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/pmcp-tasks/src/store.rs` -- TaskStore trait, StoreConfig, ListTasksOptions, TaskPage
- Codebase analysis: `crates/pmcp-tasks/src/domain/record.rs` -- TaskRecord, is_expired(), to_wire_task_with_variables()
- Codebase analysis: `crates/pmcp-tasks/src/error.rs` -- TaskError variants including OwnerMismatch, ResourceExhausted
- Codebase analysis: `crates/pmcp-tasks/src/types/task.rs` -- TaskStatus state machine, validate_transition()
- Codebase analysis: `src/shared/session.rs` -- DashMap usage pattern for SessionManager
- Codebase analysis: `src/server/auth/traits.rs` -- AuthContext struct with subject, client_id, session_id fields
- Codebase analysis: `src/shared/cancellation.rs` -- RequestHandlerExtra with auth_context field
- Codebase analysis: `crates/pmcp-tasks/Cargo.toml` -- existing dependencies
- Design doc: `docs/design/tasks-feature-design.md` sections 6 (TaskContext), 7 (Storage Backends), 9 (Security)

### Secondary (MEDIUM confidence)
- DashMap API: `entry()`, `get_mut()`, `iter()` for concurrent access patterns -- verified against codebase usage in session.rs and middleware.rs

### Tertiary (LOW confidence)
- None -- all findings verified against codebase sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use in the codebase, no new dependencies needed
- Architecture: HIGH -- patterns derived from existing codebase (SessionManager DashMap pattern, AuthContext fields, TaskRecord APIs)
- Pitfalls: HIGH -- identified from concrete codebase analysis (DashMap guard Send issues, race conditions in count-then-insert, owner mismatch leaking)
- Security design: HIGH -- user decisions are explicit and locked; AuthContext provides all needed identity fields
- TaskContext design: HIGH -- design doc provides clear blueprint; store trait already defines the needed methods

**Research date:** 2026-02-21
**Valid until:** 2026-03-21 (stable domain, no external dependencies changing)
