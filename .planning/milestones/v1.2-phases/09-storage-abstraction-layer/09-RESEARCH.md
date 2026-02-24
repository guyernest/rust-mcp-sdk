# Phase 9: Storage Abstraction Layer - Research

**Researched:** 2026-02-23
**Domain:** Rust trait-based storage abstraction, KV backend design, canonical JSON serialization
**Confidence:** HIGH

## Summary

Phase 9 introduces a two-layer storage architecture into the existing `pmcp-tasks` crate: a low-level `StorageBackend` trait defining KV operations, and a `GenericTaskStore<B: StorageBackend>` that implements all domain logic (state machine, owner isolation, variable merge, TTL, size limits) by delegating raw storage to the backend. The existing `TaskStore` trait (11 async methods, each containing duplicated domain logic in `InMemoryTaskStore`) is redesigned from scratch.

The current `InMemoryTaskStore` implementation (608 lines) embeds all domain logic directly: owner checking, TTL validation, state machine transitions, variable merge with size limits, and atomic completion. This logic must be extracted into `GenericTaskStore` so that adding DynamoDB (Phase 11) and Redis (Phase 12) backends requires only implementing ~6 KV methods, not re-implementing domain rules.

**Primary recommendation:** Define `StorageBackend` as a 6-method async trait working with `TaskRecord` (not raw bytes), backed by canonical JSON serialization in `GenericTaskStore` for storage. Use monotonic `u64` version numbers for CAS. Redesign the public API as concrete methods on `GenericTaskStore` rather than a new `TaskStore` trait, matching the `TaskRouter` pattern already established in the codebase.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Cross-Cutting: Security Posture** - TaskStorage is the first persistent attack surface in the PMCP SDK. MCP servers were previously stateless -- this is a new threat vector. All decisions must account for: data leakage across users, storage explosion/corruption, malicious input, and injection via serialized data. Owner binding must be enforced structurally (not just filter-based). NotFound on mismatch -- never reveal that a task exists for another owner. The existing v1.0 security model (owner isolation, configurable limits, anonymous access control) must be preserved and reinforced at the backend level. Size limits serve as design guidance: tasks store lightweight coordination state (status, variables, metadata), not bulk data.
- **Trait Method Design** - StorageBackend exposes ~6 methods: get, put, put-if-version (CAS), delete, list-by-prefix, cleanup-expired. No dedicated count-by-prefix -- list + .len() is sufficient. Trait methods work with TaskRecord (domain-aware), not raw bytes. Key structure (composite string vs separate fields): Claude's discretion.
- **Serialization Strategy** - Canonical serialization format: JSON -- human-readable, debuggable via DynamoDB console / Redis CLI, better for security auditing. Storage shape: single JSON blob per record -- backend stores/retrieves bytes, all field access in GenericTaskStore after deserialization. Variable values: size limit enforcement PLUS schema validation (no nested depth bombs, no extremely long strings within the size limit). No content sanitization. Universal size limit enforced in GenericTaskStore -- not backend-specific. Configurable via StoreConfig.
- **Error Model** - Domain-aware errors: TaskNotFound, ConcurrentModification, StorageFull -- not generic NotFound/IoError. ConcurrentModification is a specific variant with expected_version and actual_version fields. Backend errors include underlying cause via std::error::Error::source(). No auto-retry on transient failures -- surface errors immediately.
- **TaskStore Redesign** - Clean break from the current 11-method trait -- redesign from scratch based on the new architecture. Only keep what makes sense for GenericTaskStore's public interface. Since TaskStore is unpublished, no backward compatibility concern.

### Claude's Discretion
- CAS version mechanism (monotonic integer vs content hash)
- Key structure (composite string vs separate fields)
- Construction pattern (builder vs constructor+config) -- align with existing SDK patterns
- Public API shape (TaskStore trait vs concrete GenericTaskStore methods) -- align with TaskRouter pattern
- StoreConfig ownership (GenericTaskStore-only vs both layers) -- pragmatic choice with security in mind

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ABST-01 | StorageBackend trait defines KV operations (get, put, delete, list-by-prefix, cleanup-expired) | Architecture Pattern 1 defines the full trait with 6 methods including put_if_version (CAS). Composite key pattern enables all backends. |
| ABST-02 | GenericTaskStore implements all TaskStore domain logic (state machine, owner isolation, variable merge, TTL) by delegating to any StorageBackend | Architecture Pattern 2 shows GenericTaskStore extracting all domain logic from InMemoryTaskStore into backend-agnostic code. Every domain operation (owner check, state transition, variable merge, size validation, TTL) lives in GenericTaskStore. |
| ABST-03 | Canonical serialization layer in GenericTaskStore ensures consistent JSON round-trips regardless of backend | Architecture Pattern 3 covers canonical JSON serialization: TaskRecord gains Serialize/Deserialize, serde_json with preserve_order ensures deterministic output, GenericTaskStore serializes/deserializes at the boundary. |
| ABST-04 | TaskStore trait can be simplified/redesigned to leverage the KV backend pattern | Architecture Pattern 4 recommends removing the TaskStore trait entirely in favor of concrete GenericTaskStore methods, matching the existing TaskRouter pattern. The old trait becomes unnecessary when all domain logic lives in GenericTaskStore. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 | Serialization framework | Already in Cargo.toml, universal in Rust |
| serde_json | 1.0 (preserve_order) | Canonical JSON serialization | Already in Cargo.toml with `preserve_order` feature |
| async-trait | 0.1 | Async trait support for StorageBackend | Already used throughout the crate |
| dashmap | 6.1 | Concurrent hash map for InMemoryBackend | Already used by InMemoryTaskStore |
| chrono | 0.4 | DateTime for TTL/expiry computations | Already used in TaskRecord |
| uuid | 1.17 | Task ID generation | Already used in TaskRecord::new |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.0 | Derive Error trait with source chaining | Already in deps, needed for new StorageError with source() |
| tracing | 0.1 | Structured logging for security events | Already used for owner mismatch warnings |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_json preserve_order | serde_json default | preserve_order already enabled, gives deterministic key ordering for debugging |
| Monotonic u64 version | Content hash (SHA-256) | Monotonic u64 is simpler, cheaper to compare, and maps directly to DynamoDB version numbers. Content hash adds crypto dependency and is slower. |
| Composite string keys | Separate key fields | Composite strings (`{owner_id}:{task_id}`) are universally supported across DynamoDB (partition key) and Redis (key string). Separate fields would require backend-specific key construction. |

**No new dependencies required.** Everything needed is already in `Cargo.toml`.

## Architecture Patterns

### Recommended Module Structure
```
crates/pmcp-tasks/src/
├── store/
│   ├── mod.rs              # StoreConfig, ListTasksOptions, TaskPage (keep)
│   ├── backend.rs          # NEW: StorageBackend trait + StorageError
│   ├── generic.rs          # NEW: GenericTaskStore<B: StorageBackend>
│   └── memory.rs           # Refactored: InMemoryBackend implements StorageBackend
├── domain/
│   ├── record.rs           # TaskRecord gains Serialize/Deserialize + version field
│   └── variables.rs        # TaskWithVariables (unchanged)
├── error.rs                # Add ConcurrentModification, StorageFull variants
└── ...                     # types/, security/, context/, router/ (minimal changes)
```

### Pattern 1: StorageBackend Trait (KV Operations)

**What:** A low-level async trait defining 6 KV operations that any storage engine can implement.

**Design decisions applied:**
- Methods work with `TaskRecord` (domain-aware), not raw bytes -- the backend receives/returns the full domain type
- However, `GenericTaskStore` serializes `TaskRecord` to canonical JSON before calling `put`/`put_if_version`, and deserializes after `get` -- the backend stores opaque bytes alongside a composite key
- CAS uses monotonic `u64` versions (see Discretion section)
- Composite string keys encode owner + task_id (see Discretion section)

```rust
use async_trait::async_trait;

/// Error type for raw storage operations.
///
/// Backend errors carry the underlying cause via source() for debugging.
/// GenericTaskStore maps these to domain-aware TaskError variants.
#[derive(Debug)]
pub enum StorageError {
    /// The key was not found in storage.
    NotFound { key: String },

    /// A put_if_version call failed because the stored version does not
    /// match the expected version.
    VersionConflict {
        key: String,
        expected: u64,
        actual: u64,
    },

    /// The backend has reached a capacity limit.
    CapacityExceeded { message: String },

    /// An I/O or backend-specific error occurred.
    Backend {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// A stored record with its version number for CAS operations.
pub struct VersionedRecord {
    /// The serialized task record bytes (canonical JSON).
    pub data: Vec<u8>,

    /// Monotonic version number. Starts at 1, increments on each put.
    /// Used by put_if_version for optimistic concurrency.
    pub version: u64,
}

/// Key-value storage backend for task persistence.
///
/// Implementations provide the raw storage primitives. Domain logic
/// (state machine, owner isolation, variable merge, TTL enforcement,
/// serialization) lives in GenericTaskStore, not here.
///
/// Keys are composite strings in the format `{owner_id}:{task_id}`.
/// Prefix queries use `{owner_id}:` to scope listings to an owner.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Retrieve a record by key.
    ///
    /// Returns the record bytes and current version, or NotFound.
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError>;

    /// Store a record unconditionally (create or overwrite).
    ///
    /// The backend assigns version 1 for new keys or increments the
    /// existing version. Returns the assigned version.
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError>;

    /// Store a record only if the current version matches expected_version.
    ///
    /// This is the CAS (compare-and-swap) primitive. If the stored version
    /// does not match, returns VersionConflict with both versions.
    /// Returns the new version on success.
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError>;

    /// Delete a record by key.
    ///
    /// Returns Ok(true) if the key existed and was deleted, Ok(false)
    /// if the key did not exist (idempotent delete).
    async fn delete(&self, key: &str) -> Result<bool, StorageError>;

    /// List all records whose key starts with the given prefix.
    ///
    /// Returns (key, data, version) tuples. Used for owner-scoped
    /// listing with prefix `{owner_id}:`.
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError>;

    /// Remove records that have expired based on backend-specific criteria.
    ///
    /// For in-memory: scan all records, check TTL, remove expired ones.
    /// For DynamoDB: no-op (native TTL handles cleanup).
    /// Returns the count of records removed.
    async fn cleanup_expired(&self) -> Result<usize, StorageError>;
}
```

**Why this works for downstream backends:**
- DynamoDB: `get` = `GetItem`, `put` = `PutItem`, `put_if_version` = `PutItem` with `ConditionExpression: #version = :expected`, `delete` = `DeleteItem`, `list_by_prefix` = `Query` on GSI with `begins_with(SK, prefix)`, `cleanup_expired` = no-op (DynamoDB TTL)
- Redis: `get` = `HGETALL`, `put` = `HSET`, `put_if_version` = Lua script for atomic check-and-set, `delete` = `DEL`, `list_by_prefix` = sorted set + `SCAN`, `cleanup_expired` = `SCAN` + check TTL field

### Pattern 2: GenericTaskStore<B: StorageBackend>

**What:** A generic struct that implements all domain logic (currently scattered in InMemoryTaskStore) by delegating raw storage to a `StorageBackend`.

**Key insight:** The current `InMemoryTaskStore` (608 lines) contains ~400 lines of domain logic that must be identical across all backends: owner checking, state machine validation, variable merge with null-deletion, size limit enforcement, TTL computation, atomic completion. `GenericTaskStore` extracts this once.

```rust
/// Backend-agnostic task store implementing all domain logic.
///
/// Domain operations (state transitions, owner isolation, variable merge,
/// TTL enforcement, size limits) are implemented here once. The storage
/// backend only handles raw KV operations.
pub struct GenericTaskStore<B: StorageBackend> {
    backend: B,
    config: StoreConfig,
    security: TaskSecurityConfig,
    default_poll_interval: u64,
}

impl<B: StorageBackend> GenericTaskStore<B> {
    /// Creates a new GenericTaskStore with the given backend and default config.
    pub fn new(backend: B) -> Self { /* ... */ }

    /// Builder: sets the store configuration.
    pub fn with_config(mut self, config: StoreConfig) -> Self { /* ... */ }

    /// Builder: sets the security configuration.
    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self { /* ... */ }

    /// Builder: sets the default poll interval.
    pub fn with_poll_interval(mut self, ms: u64) -> Self { /* ... */ }

    // --- Domain operations (public API) ---

    pub async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError> {
        // 1. check_anonymous_access(owner_id)
        // 2. count owner tasks via list_by_prefix("{owner_id}:")
        // 3. validate TTL against max
        // 4. create TaskRecord::new(...)
        // 5. serialize to canonical JSON
        // 6. backend.put(key, bytes)
        // 7. return record
    }

    pub async fn get(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError> {
        // 1. construct key = "{owner_id}:{task_id}"
        // 2. backend.get(key) -> VersionedRecord
        // 3. deserialize from canonical JSON
        // 4. verify record.owner_id == owner_id (structural check)
        // 5. return record (even if expired -- per locked decision)
    }

    pub async fn update_status(...) -> Result<TaskRecord, TaskError> {
        // 1. backend.get(key) -> VersionedRecord
        // 2. deserialize
        // 3. owner check, expiry check, state machine validation
        // 4. apply transition
        // 5. serialize
        // 6. backend.put_if_version(key, bytes, version) -> CAS
        // 7. map VersionConflict -> TaskError::ConcurrentModification
    }

    // ... set_variables, set_result, get_result, complete_with_result,
    //     list, cancel, cleanup_expired
    // All follow the same pattern: get -> deserialize -> validate ->
    // mutate -> serialize -> put_if_version
}
```

**Domain logic extracted from InMemoryTaskStore:**
1. Anonymous access check (`check_anonymous_access`)
2. Owner task count limit (`count_owner_tasks` via `list_by_prefix`)
3. TTL validation and default application
4. State machine transition validation (`validate_transition`)
5. Variable merge with null-deletion semantics
6. Variable size limit enforcement (clone-check-commit)
7. Expiry check on mutation operations
8. Atomic status + result completion
9. Owner isolation (mismatch -> NotFound)
10. Cursor-based pagination with sorting

### Pattern 3: Canonical JSON Serialization

**What:** TaskRecord gains `Serialize`/`Deserialize` derives. GenericTaskStore serializes to/from JSON at the storage boundary using `serde_json` with `preserve_order`.

**Current state:** `TaskRecord` does NOT derive `Serialize`/`Deserialize`. It needs to.

**Changes required to TaskRecord:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub task: Task,
    pub owner_id: String,
    pub variables: HashMap<String, Value>,
    pub result: Option<Value>,
    pub request_method: String,
    /// Computed expiry time. Serialized as ISO 8601 for cross-backend consistency.
    pub expires_at: Option<String>,  // Change from DateTime<Utc> to String
    /// Monotonic version for CAS operations. Not serialized to wire.
    #[serde(skip)]
    pub version: u64,
}
```

**Key decisions:**
- `expires_at` changes from `DateTime<Utc>` to `String` (ISO 8601) for canonical serialization. The `DateTime<Utc>` can be computed on deserialization.
- Alternatively, keep `DateTime<Utc>` with chrono's serde support, which serializes to ISO 8601 by default.
- `version` is `#[serde(skip)]` because it is a storage-layer concern, not part of the serialized record. The backend manages versions separately.
- `serde_json::to_vec` (not `to_string`) for efficient byte serialization.
- The `preserve_order` feature on `serde_json` (already enabled) ensures map keys are serialized in insertion order, giving deterministic output.

**Serialization in GenericTaskStore:**

```rust
impl<B: StorageBackend> GenericTaskStore<B> {
    /// Serialize a TaskRecord to canonical JSON bytes.
    fn serialize_record(record: &TaskRecord) -> Result<Vec<u8>, TaskError> {
        serde_json::to_vec(record)
            .map_err(|e| TaskError::StoreError(format!("serialization failed: {e}")))
    }

    /// Deserialize a TaskRecord from JSON bytes.
    fn deserialize_record(data: &[u8]) -> Result<TaskRecord, TaskError> {
        serde_json::from_slice(data)
            .map_err(|e| TaskError::StoreError(format!("deserialization failed: {e}")))
    }
}
```

**Variable schema validation (locked decision: size limit PLUS schema validation):**

```rust
/// Validates variable values for safety.
/// Enforced in GenericTaskStore before serialization.
fn validate_variables(variables: &HashMap<String, Value>) -> Result<(), TaskError> {
    for (key, value) in variables {
        // Reject nested depth bombs (depth > 10 levels)
        validate_json_depth(value, 0, 10)?;
        // Reject extremely long individual strings (> 64KB per string value)
        validate_string_lengths(value, 65_536)?;
    }
    Ok(())
}
```

### Pattern 4: TaskStore Trait Redesign

**What:** The existing 11-method `TaskStore` trait is removed. `GenericTaskStore<B>` exposes concrete methods directly. The `TaskRouterImpl` changes from `Arc<dyn TaskStore>` to `Arc<GenericTaskStore<B>>` (or a type-erased wrapper).

**Rationale (from CONTEXT.md):** "Clean break from the current 11-method trait -- redesign from scratch." Since `TaskStore` is unpublished, there is no backward compatibility concern.

**Current pattern in codebase:** `TaskRouterImpl` holds `Arc<dyn TaskStore>` and delegates. The `TaskRouter` trait (in main `pmcp` crate) uses `Value` to avoid circular dependencies.

**Recommended approach:** Keep a simplified `TaskStore` trait as a thin interface for type erasure (`Arc<dyn TaskStore>`), but it delegates to `GenericTaskStore` methods rather than reimplementing domain logic. Alternatively, make `GenericTaskStore` the concrete type used everywhere.

**Option A (Concrete -- recommended for Phase 9):**
```rust
// TaskRouterImpl becomes generic over the backend:
pub struct TaskRouterImpl<B: StorageBackend> {
    store: GenericTaskStore<B>,
    // ...
}

// Or use a type alias:
pub type InMemoryTaskRouter = TaskRouterImpl<InMemoryBackend>;
```

**Option B (Trait-based -- if dynamic dispatch needed):**
```rust
// Simplified TaskStore trait with only the methods TaskRouter needs
#[async_trait]
pub trait TaskStore: Send + Sync {
    async fn create(&self, owner_id: &str, request_method: &str, ttl: Option<u64>)
        -> Result<TaskRecord, TaskError>;
    async fn get(&self, task_id: &str, owner_id: &str)
        -> Result<TaskRecord, TaskError>;
    // ... same methods, but now they're thin delegates to GenericTaskStore
}

// Blanket impl for GenericTaskStore
#[async_trait]
impl<B: StorageBackend> TaskStore for GenericTaskStore<B> {
    // Methods delegate to self (GenericTaskStore already has them)
}
```

**Recommendation:** Use Option B. Keep `TaskStore` as a thin trait for backward compatibility with `TaskContext` (which uses `Arc<dyn TaskStore>`) and `TaskRouterImpl` (which uses `Arc<dyn TaskStore>`). But now the trait methods are trivially implemented by `GenericTaskStore` as direct pass-through, with zero domain logic in the trait. The trait exists solely for type erasure.

### Anti-Patterns to Avoid

- **Domain logic in StorageBackend implementations:** NEVER put state machine validation, owner checking, variable merging, or TTL logic in a backend. Backends are dumb KV stores.
- **Backend-specific serialization:** NEVER let backends choose their own serialization format. GenericTaskStore owns the canonical JSON format.
- **Implicit retry on ConcurrentModification:** NEVER add retry loops. Surface the error immediately (locked decision). Callers decide retry policy.
- **Silent TTL clamping:** NEVER clamp TTL values. Hard reject if above max (locked decision, already implemented in InMemoryTaskStore).
- **Leaking task existence across owners:** NEVER return OwnerMismatch to callers. Always return NotFound on owner mismatch (locked decision).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CAS mechanism | Custom atomic compare-and-swap | `put_if_version` with backend-native CAS | DynamoDB has ConditionExpression, Redis has Lua scripts -- the backend layer maps naturally |
| JSON serialization ordering | Custom canonical form | `serde_json` with `preserve_order` feature | Already enabled in Cargo.toml, gives deterministic key ordering |
| Version tracking | Custom version counter in separate storage | `version` field on `VersionedRecord` managed by backend | Each backend tracks versions naturally (DynamoDB condition, Redis hash field, in-memory u64) |
| TTL expiry detection | Custom timer/scheduler | `is_expired()` method on deserialized TaskRecord + backend cleanup | Already implemented, just needs to work through the serialization layer |

**Key insight:** The value of this phase is NOT in building new storage capabilities -- it's in EXTRACTING existing proven domain logic from `InMemoryTaskStore` into `GenericTaskStore`, and defining a clean backend contract. The in-memory backend implementation should shrink from 608 lines to ~80 lines (just `DashMap` operations).

## Common Pitfalls

### Pitfall 1: Losing Atomicity in GenericTaskStore
**What goes wrong:** The current `InMemoryTaskStore` uses `DashMap::get_mut` to hold a lock during the entire read-check-modify-write cycle. When moving to `GenericTaskStore`, the pattern becomes get -> deserialize -> validate -> mutate -> serialize -> put_if_version. If the backend doesn't support CAS, two concurrent requests can both read version N, both validate, and both write, with the second silently overwriting the first.
**Why it happens:** The abstraction boundary moves atomicity from the store to the backend's CAS primitive.
**How to avoid:** `put_if_version` is the mandatory CAS method. GenericTaskStore MUST use it for all mutations (update_status, set_variables, set_result, complete_with_result). A VersionConflict error surfaces as ConcurrentModification to the caller.
**Warning signs:** Tests that pass with InMemoryBackend but fail under concurrent load indicate missing CAS usage.

### Pitfall 2: Owner Isolation Broken by Key Structure
**What goes wrong:** If the key structure doesn't encode the owner, a caller could construct a key for another owner's task and bypass isolation.
**Why it happens:** Composite keys like `{task_id}` alone don't encode ownership.
**How to avoid:** Key structure MUST be `{owner_id}:{task_id}`. GenericTaskStore constructs keys from both fields and also verifies `record.owner_id == owner_id` after deserialization (defense in depth).
**Warning signs:** A test where owner-B can get owner-A's task by knowing the task_id.

### Pitfall 3: Serialization Divergence Across Backends
**What goes wrong:** If backends serialize differently (one adds extra fields, another reorders keys), the canonical JSON promise is broken. Round-trip tests pass per-backend but records cannot be migrated.
**Why it happens:** Letting backends participate in serialization rather than treating them as opaque byte stores.
**How to avoid:** GenericTaskStore is the ONLY serialization point. Backends receive and return `&[u8]` / `Vec<u8>`. They must store bytes verbatim.
**Warning signs:** `assert_eq!(serialized_a, serialized_b)` fails when comparing output from two different backends for the same logical record.

### Pitfall 4: Breaking Existing Tests During Refactor
**What goes wrong:** The crate has 4,597 lines of integration/property/security tests. A careless refactor breaks test compilation.
**Why it happens:** Tests use `InMemoryTaskStore` directly with `TaskStore` trait methods. Changing the trait changes every test.
**How to avoid:** Phase 9 introduces the new types (`StorageBackend`, `GenericTaskStore`, error variants) but does NOT refactor `InMemoryTaskStore` yet. Phase 10 (IMEM-01 through IMEM-03) handles the refactor with explicit "all 200+ tests pass unchanged" success criteria.
**Warning signs:** Compilation failures in `tests/` after changing `store/mod.rs`.

### Pitfall 5: Overly Complex cleanup_expired Contract
**What goes wrong:** Different backends handle TTL differently. DynamoDB has native TTL (automatic, async). Redis uses EXPIRE. In-memory needs explicit scanning. Trying to make `cleanup_expired` uniform across all backends adds unnecessary complexity.
**Why it happens:** Treating cleanup as a backend-agnostic operation when it's inherently backend-specific.
**How to avoid:** Make `cleanup_expired` a best-effort method. In-memory scans and removes. DynamoDB returns 0 (native TTL handles it). Redis scans and removes. The contract is "remove what you can, return count removed." GenericTaskStore calls it but doesn't depend on it for correctness -- expiry is also checked at read time.

## Code Examples

### Example 1: StorageBackend Key Construction

```rust
/// Constructs a storage key from owner_id and task_id.
///
/// Format: `{owner_id}:{task_id}`
/// The colon separator is safe because owner_id and task_id are
/// validated (owner_id comes from OAuth/session, task_id is UUIDv4).
fn make_key(owner_id: &str, task_id: &str) -> String {
    format!("{owner_id}:{task_id}")
}

/// Extracts (owner_id, task_id) from a storage key.
fn parse_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Constructs a prefix for listing all tasks owned by an owner.
fn make_prefix(owner_id: &str) -> String {
    format!("{owner_id}:")
}
```

### Example 2: GenericTaskStore CAS Pattern

```rust
// Pattern used for all mutation operations in GenericTaskStore:
async fn update_status(
    &self,
    task_id: &str,
    owner_id: &str,
    new_status: TaskStatus,
    status_message: Option<String>,
) -> Result<TaskRecord, TaskError> {
    let key = make_key(owner_id, task_id);

    // 1. Read current state
    let versioned = self.backend.get(&key).await
        .map_err(|e| self.map_storage_error(e, task_id))?;

    // 2. Deserialize
    let mut record: TaskRecord = Self::deserialize_record(&versioned.data)?;

    // 3. Domain validation (all in GenericTaskStore, not backend)
    if record.owner_id != owner_id {
        return Err(TaskError::NotFound { task_id: task_id.to_string() });
    }
    if record.is_expired() {
        return Err(TaskError::Expired { /* ... */ });
    }
    record.task.status.validate_transition(task_id, &new_status)?;

    // 4. Apply mutation
    record.task.status = new_status;
    record.task.status_message = status_message;
    record.task.last_updated_at = Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    // 5. Serialize
    let data = Self::serialize_record(&record)?;

    // 6. CAS write
    self.backend.put_if_version(&key, &data, versioned.version).await
        .map_err(|e| match e {
            StorageError::VersionConflict { expected, actual, .. } => {
                TaskError::ConcurrentModification {
                    task_id: task_id.to_string(),
                    expected_version: expected,
                    actual_version: actual,
                }
            }
            other => self.map_storage_error(other, task_id),
        })?;

    Ok(record)
}
```

### Example 3: InMemoryBackend (Target Implementation for Phase 10)

```rust
/// In-memory StorageBackend using DashMap.
///
/// This is what InMemoryTaskStore shrinks to after Phase 10.
/// ~80 lines instead of ~600.
pub struct InMemoryBackend {
    data: DashMap<String, (Vec<u8>, u64)>,  // key -> (data, version)
}

#[async_trait]
impl StorageBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        let entry = self.data.get(key)
            .ok_or_else(|| StorageError::NotFound { key: key.to_string() })?;
        let (data, version) = entry.value();
        Ok(VersionedRecord { data: data.clone(), version: *version })
    }

    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        let new_version = self.data
            .entry(key.to_string())
            .and_modify(|(d, v)| { *d = data.to_vec(); *v += 1; })
            .or_insert_with(|| (data.to_vec(), 1))
            .value().1;
        Ok(new_version)
    }

    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected: u64,
    ) -> Result<u64, StorageError> {
        let mut entry = self.data.get_mut(key)
            .ok_or_else(|| StorageError::NotFound { key: key.to_string() })?;
        let (current_data, current_version) = entry.value_mut();
        if *current_version != expected {
            return Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected,
                actual: *current_version,
            });
        }
        *current_data = data.to_vec();
        *current_version += 1;
        Ok(*current_version)
    }

    // ... delete, list_by_prefix, cleanup_expired
}
```

### Example 4: Error Model Updates

```rust
// New variants to add to TaskError:
pub enum TaskError {
    // ... existing variants ...

    /// Concurrent modification detected via CAS failure.
    ConcurrentModification {
        task_id: String,
        expected_version: u64,
        actual_version: u64,
    },

    /// Storage backend is full or at capacity.
    StorageFull {
        message: String,
    },
}

// New StorageError for backend layer:
#[derive(Debug)]
pub enum StorageError {
    NotFound { key: String },
    VersionConflict { key: String, expected: u64, actual: u64 },
    CapacityExceeded { message: String },
    Backend {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend { source, .. } => source.as_deref().map(|s| s as &dyn std::error::Error),
            _ => None,
        }
    }
}
```

## Discretion Decisions (Researcher Recommendations)

### CAS Version Mechanism: Monotonic u64

**Recommendation:** Use monotonic `u64` integers starting at 1, incremented on each successful write.

**Rationale:**
- DynamoDB: Maps directly to a `version` number attribute used in `ConditionExpression: #version = :expected`. DynamoDB's `ADD` operation can atomically increment.
- Redis: Stored as a hash field. Lua scripts compare integers efficiently.
- In-memory: Simple `u64` field on the entry, incremented in `put_if_version`.
- Content hash alternative: Requires SHA-256 computation on every write, adds `sha2` dependency, harder to debug ("version 7" is more meaningful than "version a3f2c1...").
- Confidence: HIGH -- monotonic integers are the standard pattern for optimistic concurrency in DynamoDB and databases generally.

### Key Structure: Composite String

**Recommendation:** Single composite string: `{owner_id}:{task_id}`.

**Rationale:**
- DynamoDB: The composite string becomes the partition key. Owner-scoped queries use `begins_with(PK, "{owner_id}:")` on a GSI, or the key itself is the full PK with a fixed SK.
- Redis: The composite string becomes the Redis key. `SCAN` with pattern `{owner_id}:*` for listing.
- In-memory: DashMap key is the composite string. `iter().filter(starts_with(prefix))` for listing.
- The colon separator is safe: owner_id comes from OAuth/session (no colons), task_id is UUIDv4 (no colons).
- Confidence: HIGH -- composite keys are standard for multi-tenant KV stores.

### Construction Pattern: Builder (with_* methods)

**Recommendation:** Use builder pattern with `new()` + `with_config()` + `with_security()` + `with_poll_interval()`.

**Rationale:** This exactly matches the existing `InMemoryTaskStore` construction pattern (see `memory.rs` lines 100-158). Maintaining the same pattern reduces cognitive load and keeps the API familiar.

### Public API Shape: Keep TaskStore Trait (Thin)

**Recommendation:** Keep a `TaskStore` trait but make it a thin interface. `GenericTaskStore<B>` implements it via blanket impl. This preserves compatibility with `TaskContext` (uses `Arc<dyn TaskStore>`) and `TaskRouterImpl` (uses `Arc<dyn TaskStore>`).

**Rationale:** `TaskContext` and `TaskRouterImpl` both use `Arc<dyn TaskStore>`. Removing the trait would require making these generic over `B: StorageBackend`, which propagates generics through the public API and is less ergonomic. A thin trait provides type erasure.

### StoreConfig Ownership: GenericTaskStore Only

**Recommendation:** `StoreConfig` lives on `GenericTaskStore` only. `StorageBackend` implementations don't need it -- they don't validate TTL, variable sizes, or domain constraints.

**Rationale:** Backends are dumb KV stores. They don't need to know about variable size limits or TTL policies. GenericTaskStore enforces all limits before calling the backend.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Domain logic in each backend | Generic store + KV backend | This phase | Backends become trivial (~80 lines), domain logic is single-source |
| No CAS support | put_if_version as first-class method | This phase | Concurrent safety built into the contract from day one |
| In-memory only | Pluggable backends | This phase + 10-12 | DynamoDB, Redis, custom backends all possible |
| Raw HashMap variables | Canonical JSON with schema validation | This phase | Cross-backend consistency, protection against depth bombs |

## Open Questions

1. **`expires_at` serialization format**
   - What we know: Currently `Option<DateTime<Utc>>`, which chrono serializes to ISO 8601 with its serde feature. The locked decision says "canonical JSON."
   - What's unclear: Should we change `expires_at` to `Option<String>` (explicit ISO 8601) or keep `DateTime<Utc>` with chrono serde? Both produce ISO 8601. The `String` approach is more explicit and avoids chrono version-specific serialization format differences.
   - Recommendation: Keep `DateTime<Utc>` with chrono serde -- it's already correct and avoids adding manual parsing. The `preserve_order` feature handles key ordering. Test round-trip to confirm.

2. **Variable depth/string validation specifics**
   - What we know: Locked decision says "size limit enforcement PLUS schema validation (no nested depth bombs, no extremely long strings)." No content sanitization.
   - What's unclear: Exact depth limit (10? 20?) and string length limit (64KB? 256KB?). These should be configurable in `StoreConfig`.
   - Recommendation: Add `max_variable_depth: usize` (default 10) and `max_string_length: usize` (default 65,536) to `StoreConfig`. Enforce in GenericTaskStore before serialization.

3. **Phase 9 scope boundary with Phase 10**
   - What we know: Phase 9 defines the abstraction. Phase 10 refactors InMemoryTaskStore to use it.
   - What's unclear: Should Phase 9 include a minimal `InMemoryBackend` implementation for testing the new traits, or should it be pure trait + GenericTaskStore definitions with no backend?
   - Recommendation: Include a minimal `InMemoryBackend` for testing GenericTaskStore. Without it, the new code is untestable. But don't refactor the existing `InMemoryTaskStore` -- that's Phase 10. Both can coexist temporarily.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `crates/pmcp-tasks/src/store/mod.rs` (TaskStore trait, 358 lines)
- Existing codebase: `crates/pmcp-tasks/src/store/memory.rs` (InMemoryTaskStore, 1327 lines including tests)
- Existing codebase: `crates/pmcp-tasks/src/domain/record.rs` (TaskRecord, 357 lines)
- Existing codebase: `crates/pmcp-tasks/src/error.rs` (TaskError, 228 lines)
- Existing codebase: `crates/pmcp-tasks/src/router.rs` (TaskRouterImpl, 1287 lines including tests)
- Existing codebase: `crates/pmcp-tasks/Cargo.toml` (dependencies)
- Project decision documents: `.planning/phases/09-storage-abstraction-layer/09-CONTEXT.md`
- Project requirements: `.planning/REQUIREMENTS.md`

### Secondary (MEDIUM confidence)
- Design document: `docs/design/tasks-feature-design.md` (detailed architecture)
- DynamoDB single-table design patterns (from training data, verified against design doc's table schema)
- Redis atomic operations via Lua scripts (from training data)

### Tertiary (LOW confidence)
- None -- all findings are grounded in codebase analysis and locked decisions.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all libraries already in Cargo.toml
- Architecture: HIGH -- patterns derived directly from existing InMemoryTaskStore code and locked decisions
- Pitfalls: HIGH -- identified from analyzing the actual 608-line InMemoryTaskStore implementation and its 4,597 lines of tests
- Discretion decisions: HIGH -- each recommendation maps to concrete backend requirements (DynamoDB, Redis)

**Research date:** 2026-02-23
**Valid until:** 2026-03-23 (stable domain, patterns won't change)
