# Phase 10: InMemory Backend Refactor - Research

**Researched:** 2026-02-23
**Domain:** Rust storage abstraction / refactoring with behavioral parity
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Constructor changes to accept a backend argument: `InMemoryTaskStore::new(backend)` following GenericTaskStore's pattern (breaking change accepted)
- Builder methods (`with_config`, `with_security`, `with_poll_interval`) must remain available on the resulting type
- Accept stricter validation from GenericTaskStore (variable depth bomb protection, string length limits) -- these are improvements, not regressions
- Accept CAS (put_if_version) semantics for mutations -- ConcurrentModification errors surfaced explicitly instead of silent DashMap lock serialization
- Accept JSON serialization overhead on every operation -- correctness and backend uniformity matter more than raw performance for in-memory dev/test use
- Cleanup_expired: InMemoryBackend deserializes each record to check TTL (consistent with TestBackend pattern in generic.rs, simple approach)
- "Tests pass unchanged" means: test assertions and coverage stay the same, but test setup code (e.g., forcing expiry by mutating internals) can be adapted to the new internal structure
- Replace the TestBackend in generic.rs with the real InMemoryBackend after the refactor -- single source of truth, no duplicated test backend
- Per-backend unit tests (TEST-01): full contract coverage of all 6 StorageBackend methods (get, put, put_if_version, delete, list_by_prefix, cleanup_expired) with happy paths and error cases

### Claude's Discretion
- Type alias vs thin wrapper decision
- InMemoryBackend visibility (public vs pub(crate))
- Poll interval default (5000ms vs 500ms)
- Test file organization strategy
- Any internal implementation details for InMemoryBackend (data structures, locking strategy within DashMap)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| IMEM-01 | InMemoryBackend implements StorageBackend using DashMap for concurrent KV storage | TestBackend in generic.rs is the exact prototype; InMemoryBackend is a promoted, public version with identical data structure `DashMap<String, (Vec<u8>, u64)>` |
| IMEM-02 | InMemoryTaskStore becomes GenericTaskStore\<InMemoryBackend\> with backward-compatible constructors | Thin wrapper recommended over type alias to preserve `new()` (no-arg), `Default`, and doctests; delegates to `GenericTaskStore::new(InMemoryBackend::new())` internally |
| IMEM-03 | All existing 200+ tests pass unchanged after the refactor | 536 tests identified across 10 test files; key migration concerns documented in Pitfalls section (keying model, poll interval, concurrency, test internals) |
| TEST-01 | Per-backend unit tests for InMemoryBackend validating StorageBackend contract | 6 methods x (happy + error paths) = ~12-18 tests; placed in `store/memory.rs` or a new `store/memory/tests.rs` module |
</phase_requirements>

## Summary

Phase 10 replaces the current `InMemoryTaskStore` (which directly implements all 11 `TaskStore` methods with inline domain logic across ~600 lines in `store/memory.rs`) with a thin wrapper around `GenericTaskStore<InMemoryBackend>`. The `GenericTaskStore` (built in Phase 9, ~600 lines in `store/generic.rs`) already contains all domain logic -- state machine validation, owner isolation, variable merge, TTL, CAS-based mutations, canonical JSON serialization. The `InMemoryBackend` struct needs only to implement the 6-method `StorageBackend` trait as a dumb KV store using DashMap.

The TestBackend in `generic.rs` (lines 626-730) is essentially the implementation of InMemoryBackend already. It uses `DashMap<String, (Vec<u8>, u64)>` with identical semantics. The refactoring task is therefore primarily about: (1) promoting TestBackend to a production-quality InMemoryBackend, (2) making InMemoryTaskStore delegate to GenericTaskStore, (3) adapting test setup code that accesses internals, and (4) replacing TestBackend in generic.rs tests with the real InMemoryBackend.

**Primary recommendation:** Use a thin wrapper struct (not a type alias) for InMemoryTaskStore to preserve the zero-argument `new()` constructor, `Default` impl, and all existing doctests. InMemoryBackend should be public (`pub`) to allow downstream users to instantiate GenericTaskStore with custom configurations.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dashmap | 6.1 | Concurrent DashMap for InMemoryBackend KV storage | Already a dependency; same library as current InMemoryTaskStore |
| async-trait | 0.1 | StorageBackend trait async methods | Already used for all async traits in the project |
| serde_json | 1.0 | TaskRecord serialization in cleanup_expired | Already used throughout |
| chrono | 0.4 | Expiry checking in cleanup_expired | Already used throughout |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1 | Async test runtime | Already in dev-dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Thin wrapper | Type alias (`type InMemoryTaskStore = GenericTaskStore<InMemoryBackend>`) | Type alias cannot have inherent `impl` blocks for `new()`, `Default`, or doctests; would break all existing call sites that use `InMemoryTaskStore::new()` |

**No new dependencies required.** All libraries are already in Cargo.toml.

## Architecture Patterns

### Recommended File Structure
```
crates/pmcp-tasks/src/store/
├── mod.rs          # TaskStore trait, StoreConfig, blanket impl (unchanged)
├── backend.rs      # StorageBackend trait, key helpers (unchanged)
├── generic.rs      # GenericTaskStore<B> (unchanged except TestBackend removal)
└── memory.rs       # InMemoryBackend + InMemoryTaskStore wrapper (rewritten)
```

### Pattern 1: InMemoryBackend as a Promoted TestBackend
**What:** The TestBackend in generic.rs (lines 626-730) becomes the production InMemoryBackend in memory.rs. The data structure and all 6 method implementations are identical -- it is literally a copy-and-promote operation.

**Data structure:**
```rust
#[derive(Debug)]
pub struct InMemoryBackend {
    data: DashMap<String, (Vec<u8>, u64)>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}
```

**Key detail:** Every StorageBackend method implementation is byte-for-byte identical to TestBackend. No logic changes needed.

### Pattern 2: InMemoryTaskStore as Thin Wrapper
**What:** InMemoryTaskStore becomes a newtype wrapper around `GenericTaskStore<InMemoryBackend>` with convenience constructors that match the current API.

```rust
#[derive(Debug)]
pub struct InMemoryTaskStore {
    inner: GenericTaskStore<InMemoryBackend>,
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            inner: GenericTaskStore::new(InMemoryBackend::new()),
        }
    }

    pub fn with_config(mut self, config: StoreConfig) -> Self {
        self.inner = self.inner.with_config(config);
        self
    }

    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self {
        self.inner = self.inner.with_security(security);
        self
    }

    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.inner = self.inner.with_poll_interval(ms);
        self
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}
```

**TaskStore delegation:** Since `GenericTaskStore<InMemoryBackend>` already has a blanket `TaskStore` impl, InMemoryTaskStore needs its own `#[async_trait] impl TaskStore` that delegates to `self.inner`. This is ~50 lines of boilerplate forwarding.

### Pattern 3: TestBackend Replacement in generic.rs
**What:** After InMemoryBackend exists, the TestBackend in generic.rs tests is replaced by `InMemoryBackend`. The `CasConflictBackend` test wrapper remains because it tests GenericTaskStore's CAS error handling logic, not a specific backend.

```rust
// Before:
fn test_store() -> GenericTaskStore<TestBackend> {
    GenericTaskStore::new(TestBackend::new())
        .with_security(...)
}

// After:
use crate::store::memory::InMemoryBackend;
fn test_store() -> GenericTaskStore<InMemoryBackend> {
    GenericTaskStore::new(InMemoryBackend::new())
        .with_security(...)
}
```

**CasConflictBackend:** Stays as-is but wraps `Arc<InMemoryBackend>` instead of `Arc<TestBackend>`.

### Anti-Patterns to Avoid
- **Type alias for InMemoryTaskStore:** A `type InMemoryTaskStore = GenericTaskStore<InMemoryBackend>` cannot have an inherent `impl` block, so `InMemoryTaskStore::new()` (no arguments) would be impossible. Every test file, example, and consumer uses `InMemoryTaskStore::new()`.
- **Exposing GenericTaskStore internals:** Tests should not reach into `GenericTaskStore.backend` to mutate internal state. Instead, use the StorageBackend methods (put_if_version) to force-write modified records for expiry testing.
- **Duplicating domain logic in InMemoryBackend:** The backend MUST remain a dumb KV store. All domain logic is in GenericTaskStore.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CAS atomicity | Custom lock-based CAS | DashMap entry API (`get_mut` + version check) | TestBackend already uses this correctly; same pattern |
| JSON serialization | Custom serialization | `serde_json::to_vec` / `from_slice` | GenericTaskStore handles all serialization |
| Owner isolation | Owner checks in backend | GenericTaskStore owner isolation | Backend keys already include owner; GenericTaskStore checks owner_id post-deserialization |
| State machine | Status validation in backend | GenericTaskStore state machine | Domain logic belongs exclusively in GenericTaskStore |

**Key insight:** InMemoryBackend is ~100 lines of code because it does NOTHING intelligent. All intelligence is in GenericTaskStore. Resist any temptation to add domain awareness to the backend.

## Common Pitfalls

### Pitfall 1: Keying Model Mismatch
**What goes wrong:** The current InMemoryTaskStore keys tasks by `task_id` only (flat DashMap\<String, TaskRecord\>). GenericTaskStore uses composite keys `{owner_id}:{task_id}`. Tests that directly access `store.tasks.get(&task_id)` will break because the key format changes.
**Why it happens:** The old store used a flat namespace with runtime owner filtering. The new store uses owner-scoped keys for backend-agnostic prefix listing.
**How to avoid:** This is an intentional architectural change. Tests that force-write expired records currently do `store.tasks.get_mut(&task_id)`. These must be adapted to use the `InMemoryBackend` methods (put_if_version with the composite key) instead. The CONTEXT.md explicitly allows adapting test setup code.
**Warning signs:** Any test accessing `store.tasks` directly (found in memory.rs unit tests at lines 801, 859, 1010, 1296, and 1302).
**Affected tests:**
- `get_returns_expired_task` (line 801)
- `update_status_rejects_expired_task` (line 859)
- `set_variables_rejects_expired` (line 1010)
- `cleanup_expired_removes_expired_tasks` (line 1296)
- `cleanup_expired_keeps_non_expired` checks `store.tasks.len()` (line 1302)

### Pitfall 2: Poll Interval Default Mismatch
**What goes wrong:** Current InMemoryTaskStore defaults to 5000ms poll interval. GenericTaskStore defaults to 500ms. Tests that assert `poll_interval == Some(5000)` will fail if the wrapper uses GenericTaskStore's default.
**Why it happens:** Different reasonable defaults chosen at different times.
**How to avoid:** The thin wrapper's `new()` constructor MUST explicitly pass `with_poll_interval(5000)` to GenericTaskStore to maintain behavioral parity. Alternatively, change to 500ms and update the ~3 test assertions. CONTEXT.md leaves this to Claude's discretion. Recommendation: keep 5000ms for zero test churn.
**Warning signs:** Tests asserting `poll_interval == Some(5000)`:
- `memory.rs::new_creates_empty_store` (line 637)
- `memory.rs::default_delegates_to_new` (line 644)
- `memory.rs::create_returns_working_task` (line 679)
- `store_tests.rs::test_create_sets_poll_interval` (line 63)

### Pitfall 3: Concurrent Variable Write Semantics Change
**What goes wrong:** Current InMemoryTaskStore uses DashMap entry locks for atomic variable merge. GenericTaskStore uses CAS (read-modify-write with version check). Under concurrent writes, CAS can produce `ConcurrentModification` errors that the old store never raised.
**Why it happens:** DashMap's `get_mut` holds a lock for the duration, making concurrent writes serial. CAS is optimistic and detects conflicts rather than preventing them.
**How to avoid:** The `test_concurrent_updates_no_data_loss` test in `store_tests.rs` (line 755) expects ALL 5 concurrent variable writes to succeed. With CAS, some may fail with ConcurrentModification. Per CONTEXT.md, this is an accepted behavioral change. However, this test may need to be adapted to retry on ConcurrentModification or to accept that some writes fail.
**Warning signs:** The `concurrency_tests` module in `store_tests.rs` (lines 714-838).

### Pitfall 4: Version Field in Test Assertions
**What goes wrong:** The current InMemoryTaskStore never sets `record.version` (it stays 0 because the store does not use versioning). GenericTaskStore sets `record.version` to the backend's version number (starting at 1). Tests that explicitly check `version == 0` will fail.
**Why it happens:** Versioning was not part of the original InMemoryTaskStore.
**How to avoid:** Check all test assertions on `record.version`. The generic.rs test `create_returns_working_task` already asserts `version > 0`.
**Warning signs:** Any assertion like `assert_eq!(record.version, 0)` in memory.rs tests. Review found: the `new_creates_empty_store` test checks `store.tasks.len()` and `default_poll_interval`, NOT version. The `domain/record.rs` test `new_record_version_is_zero` tests `TaskRecord::new()` directly (not store-created records), so it is unaffected.

### Pitfall 5: `store.tasks.len()` No Longer Available
**What goes wrong:** Memory.rs unit tests check `store.tasks.len()` directly. After refactoring, the `tasks` DashMap field no longer exists; it is inside `InMemoryBackend` inside `GenericTaskStore` inside the wrapper.
**Why it happens:** Internal structure changes.
**How to avoid:** Tests that need to count stored items should use `store.list()` or expose a `len()` method on InMemoryBackend. Alternatively, the thin wrapper can expose the backend for test access via `#[cfg(test)]` accessor.
**Warning signs:** `store.tasks.len()` at lines 635, 642, 1302, 1314.

### Pitfall 6: Missing `impl TaskStore for InMemoryTaskStore` After Wrapper
**What goes wrong:** The blanket `impl TaskStore for GenericTaskStore<B>` does not automatically apply to the InMemoryTaskStore wrapper. A separate delegation impl is needed.
**Why it happens:** InMemoryTaskStore is a newtype, not GenericTaskStore itself.
**How to avoid:** Add an explicit `#[async_trait] impl TaskStore for InMemoryTaskStore` that delegates all 11 methods to `self.inner`. Alternatively, implement `Deref<Target = GenericTaskStore<InMemoryBackend>>` but this leaks the inner type and is discouraged for newtypes.
**Warning signs:** Compilation errors on `Arc<InMemoryTaskStore>` used as `Arc<dyn TaskStore>`.

### Pitfall 7: Lifecycle Integration Tests Use `Arc<InMemoryTaskStore>` Concretely
**What goes wrong:** Tests like `lifecycle_integration.rs` and `workflow_integration.rs` hold `Arc<InMemoryTaskStore>` and pass it to `TaskRouterImpl::new()`. These must continue to work.
**Why it happens:** The test helper functions return concrete `Arc<InMemoryTaskStore>` types.
**How to avoid:** Ensure `InMemoryTaskStore` implements `TaskStore` (via delegation). `TaskRouterImpl::new()` takes `Arc<dyn TaskStore>`, so `Arc<InMemoryTaskStore>` will coerce to `Arc<dyn TaskStore>` as long as `TaskStore` is implemented.
**Warning signs:** All test files that import `pmcp_tasks::store::memory::InMemoryTaskStore` or `pmcp_tasks::InMemoryTaskStore`.

## Code Examples

### Example 1: InMemoryBackend Implementation (derived from TestBackend)
```rust
// Source: crates/pmcp-tasks/src/store/generic.rs lines 626-730 (TestBackend)
use dashmap::DashMap;
use async_trait::async_trait;
use crate::store::backend::{StorageBackend, StorageError, VersionedRecord};
use crate::domain::TaskRecord;

#[derive(Debug)]
pub struct InMemoryBackend {
    data: DashMap<String, (Vec<u8>, u64)>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self { data: DashMap::new() }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl StorageBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        let entry = self.data.get(key).ok_or_else(|| StorageError::NotFound {
            key: key.to_string(),
        })?;
        let (data, version) = entry.value();
        Ok(VersionedRecord { data: data.clone(), version: *version })
    }

    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        let new_version = self.data.get(key).map_or(1, |entry| entry.value().1 + 1);
        self.data.insert(key.to_string(), (data.to_vec(), new_version));
        Ok(new_version)
    }

    async fn put_if_version(
        &self, key: &str, data: &[u8], expected_version: u64,
    ) -> Result<u64, StorageError> {
        let mut entry = self.data.get_mut(key).ok_or_else(|| StorageError::NotFound {
            key: key.to_string(),
        })?;
        let (_, current_version) = *entry.value();
        if current_version != expected_version {
            return Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: current_version,
            });
        }
        let new_version = current_version + 1;
        *entry.value_mut() = (data.to_vec(), new_version);
        Ok(new_version)
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        Ok(self.data.remove(key).is_some())
    }

    async fn list_by_prefix(
        &self, prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        let results = self.data.iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| {
                let (data, version) = entry.value();
                (entry.key().clone(), VersionedRecord { data: data.clone(), version: *version })
            })
            .collect();
        Ok(results)
    }

    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        let keys_to_remove: Vec<String> = self.data.iter()
            .filter_map(|entry| {
                let (data, _) = entry.value();
                let record: TaskRecord = serde_json::from_slice(data).ok()?;
                if record.is_expired() { Some(entry.key().clone()) } else { None }
            })
            .collect();
        for key in &keys_to_remove { self.data.remove(key); }
        Ok(keys_to_remove.len())
    }
}
```

### Example 2: InMemoryTaskStore Thin Wrapper with TaskStore Delegation
```rust
pub struct InMemoryTaskStore {
    inner: GenericTaskStore<InMemoryBackend>,
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            inner: GenericTaskStore::new(InMemoryBackend::new())
                .with_poll_interval(5000), // Preserve legacy default
        }
    }
    // with_config, with_security, with_poll_interval delegate to inner
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn create(&self, owner_id: &str, request_method: &str, ttl: Option<u64>)
        -> Result<TaskRecord, TaskError> {
        self.inner.create(owner_id, request_method, ttl).await
    }
    // ... all 11 methods delegate to self.inner
}
```

### Example 3: Test Adaptation for Forced Expiry (Before/After)
```rust
// BEFORE (accesses store.tasks directly):
let mut entry = store.tasks.get_mut(&created.task.task_id).unwrap();
entry.value_mut().expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));
drop(entry);

// AFTER (uses backend methods through composite key):
// Option A: Create task with 1ms TTL and sleep
let created = store.create("owner", "tools/call", Some(1)).await.unwrap();
tokio::time::sleep(std::time::Duration::from_millis(10)).await;

// Option B: Expose backend for test via #[cfg(test)] accessor
// then use put_if_version to write a modified record with past expiry
```

### Example 4: Per-Backend StorageBackend Contract Tests
```rust
#[cfg(test)]
mod backend_tests {
    use super::*;

    fn backend() -> InMemoryBackend { InMemoryBackend::new() }

    #[tokio::test]
    async fn get_missing_key_returns_not_found() {
        let b = backend();
        let result = b.get("nonexistent").await;
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    #[tokio::test]
    async fn put_then_get_round_trips() {
        let b = backend();
        let version = b.put("key", b"data").await.unwrap();
        assert_eq!(version, 1);
        let record = b.get("key").await.unwrap();
        assert_eq!(record.data, b"data");
        assert_eq!(record.version, 1);
    }

    #[tokio::test]
    async fn put_if_version_succeeds_on_match() {
        let b = backend();
        b.put("key", b"v1").await.unwrap();
        let v2 = b.put_if_version("key", b"v2", 1).await.unwrap();
        assert_eq!(v2, 2);
    }

    #[tokio::test]
    async fn put_if_version_fails_on_mismatch() {
        let b = backend();
        b.put("key", b"v1").await.unwrap();
        let result = b.put_if_version("key", b"v2", 99).await;
        assert!(matches!(result, Err(StorageError::VersionConflict { .. })));
    }

    // ... delete, list_by_prefix, cleanup_expired tests
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct TaskStore impl in InMemoryTaskStore (~600 LOC of duplicated domain logic) | GenericTaskStore<B> centralizes all domain logic; backends are dumb KV stores | Phase 9 (2026-02-23) | InMemoryTaskStore domain logic becomes dead code, replaced by GenericTaskStore delegation |
| DashMap<String, TaskRecord> keyed by task_id | DashMap<String, (Vec<u8>, u64)> keyed by {owner_id}:{task_id} | Phase 10 (this phase) | Enables backend-agnostic prefix listing; adds JSON serialization overhead |
| DashMap entry locks for atomicity | CAS (put_if_version) for optimistic concurrency | Phase 9/10 | Concurrent modifications detected explicitly; ConcurrentModification errors possible |

## Open Questions

1. **Concurrent variable write test adaptation**
   - What we know: `test_concurrent_updates_no_data_loss` expects all 5 concurrent writes to succeed. With CAS, some may fail.
   - What's unclear: Whether this test should be relaxed (accept that CAS conflicts cause some writes to fail), or whether writes should retry on ConcurrentModification.
   - Recommendation: Adapt the test to accept that concurrent writes may produce ConcurrentModification errors (per CONTEXT.md accepted behavioral change). The test can verify that no panics occur and that the final state is consistent, without asserting all 5 writes succeed. Alternatively, run writes sequentially with small delays.

2. **Force-expiry test strategy**
   - What we know: 4 memory.rs unit tests force expiry by mutating `store.tasks.get_mut()`. This internal access disappears.
   - What's unclear: Whether to use 1ms TTL + sleep (may be flaky in CI) or to expose a `#[cfg(test)]` accessor on the wrapper.
   - Recommendation: Use the same pattern as `store_tests.rs` integration tests (TTL of 1ms + 10ms sleep). This already works reliably in the codebase at `store_tests.rs` lines 591-606. For unit tests that need deterministic control, expose a `#[cfg(test)]` method like `backend(&self) -> &InMemoryBackend` to allow test code to write modified records via `put_if_version`.

3. **`store.tasks.len()` replacement**
   - What we know: 4 tests check `store.tasks.len()` to verify task count.
   - What's unclear: Best replacement strategy.
   - Recommendation: Replace with `store.list(ListTasksOptions { owner_id: X, cursor: None, limit: None }).await.unwrap().tasks.len()` or expose `InMemoryBackend::len()` method (simple wrapper around `self.data.len()`).

## Sources

### Primary (HIGH confidence)
- **Codebase inspection:** `crates/pmcp-tasks/src/store/generic.rs` -- GenericTaskStore implementation and TestBackend (lines 1-1386)
- **Codebase inspection:** `crates/pmcp-tasks/src/store/memory.rs` -- Current InMemoryTaskStore implementation (lines 1-1327)
- **Codebase inspection:** `crates/pmcp-tasks/src/store/backend.rs` -- StorageBackend trait definition (lines 1-521)
- **Codebase inspection:** `crates/pmcp-tasks/src/store/mod.rs` -- TaskStore trait and blanket impl (lines 1-586)
- **Codebase inspection:** `crates/pmcp-tasks/src/lib.rs` -- Public re-exports (lines 1-61)
- **Codebase inspection:** All test files in `crates/pmcp-tasks/tests/` (store_tests.rs, property_tests.rs, security_tests.rs, context_tests.rs, lifecycle_integration.rs, workflow_integration.rs)
- **Codebase inspection:** Examples `60_tasks_basic.rs` and `62_task_workflow_lifecycle.rs`
- **Phase context:** `.planning/phases/10-inmemory-backend-refactor/10-CONTEXT.md`

### Secondary (MEDIUM confidence)
- **Test count verification:** `cargo test -p pmcp-tasks -- --list` confirmed 536 tests across 10 test binaries

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all libraries already in use
- Architecture: HIGH -- TestBackend in generic.rs is a proven prototype of InMemoryBackend; thin wrapper pattern is straightforward
- Pitfalls: HIGH -- all pitfalls identified through direct codebase inspection with specific line references

**Research date:** 2026-02-23
**Valid until:** 2026-03-23 (stable internal refactoring, no external dependency changes)
