---
phase: 09-storage-abstraction-layer
plan: 01
subsystem: database
tags: [async-trait, serde, cas, kv-store, storage-backend, optimistic-concurrency]

# Dependency graph
requires:
  - phase: 08-workflow-api
    provides: "existing TaskRecord, TaskError, StoreConfig, TaskStore trait"
provides:
  - "StorageBackend async trait with 6 KV methods (get, put, put_if_version, delete, list_by_prefix, cleanup_expired)"
  - "StorageError enum with 4 domain-aware variants and source() chaining"
  - "VersionedRecord struct for CAS-backed versioned storage"
  - "Key helper functions (make_key, parse_key, make_prefix)"
  - "TaskRecord with Serialize/Deserialize and version field"
  - "Variable validation functions (depth bombs, long strings)"
  - "ConcurrentModification and StorageFull error variants"
  - "StoreConfig with max_variable_depth and max_string_length fields"
affects: [09-02-generic-task-store, 10-in-memory-backend, 11-dynamodb-backend, 12-redis-backend]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "StorageBackend KV trait with CAS (put_if_version) as first-class method"
    - "Composite key format: {owner_id}:{task_id}"
    - "Monotonic u64 version numbers for optimistic concurrency"
    - "Canonical JSON serialization via serde with camelCase rename"
    - "Variable schema validation (depth + string length limits)"

key-files:
  created:
    - "crates/pmcp-tasks/src/store/backend.rs"
  modified:
    - "crates/pmcp-tasks/src/store/mod.rs"
    - "crates/pmcp-tasks/src/domain/record.rs"
    - "crates/pmcp-tasks/src/error.rs"
    - "crates/pmcp-tasks/src/lib.rs"

key-decisions:
  - "Monotonic u64 versions for CAS (maps to DynamoDB ConditionExpression and Redis Lua scripts)"
  - "Composite string keys {owner_id}:{task_id} (universal across DynamoDB/Redis/in-memory)"
  - "Keep DateTime<Utc> with chrono serde for expires_at (auto ISO 8601 round-trip)"
  - "version field uses serde(skip) -- managed by storage layer, not serialized"
  - "Variable validation defaults: max_depth=10, max_string_length=65536"

patterns-established:
  - "StorageBackend trait: backends are dumb KV stores, no domain logic"
  - "Key helpers: make_key/parse_key/make_prefix as free functions"
  - "StorageError -> TaskError mapping pattern for GenericTaskStore"

requirements-completed: [ABST-01, ABST-03]

# Metrics
duration: 8min
completed: 2026-02-24
---

# Phase 9 Plan 01: StorageBackend Trait and Foundation Types Summary

**StorageBackend KV trait with 6 async CAS-backed methods, StorageError with source() chaining, TaskRecord serde derives, and variable depth/string validation**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-24T00:43:41Z
- **Completed:** 2026-02-24T00:51:41Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- StorageBackend async trait with 6 methods defining the KV contract for all storage backends
- StorageError with 4 domain-aware variants (NotFound, VersionConflict, CapacityExceeded, Backend with source())
- TaskRecord gained Serialize/Deserialize with camelCase rename and serde(skip) version field
- Variable schema validation preventing depth bombs and excessively long strings
- ConcurrentModification and StorageFull error variants in TaskError
- StoreConfig extended with max_variable_depth and max_string_length fields
- 46 new tests (25 backend unit tests + 21 record/error tests), all 506 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Create StorageBackend trait, StorageError, and VersionedRecord** - `9b4546e` (feat)
2. **Task 2: Add Serialize/Deserialize, error variants, and variable validation** - `35656f0` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/backend.rs` - StorageBackend trait, StorageError enum, VersionedRecord struct, key helper functions
- `crates/pmcp-tasks/src/store/mod.rs` - Module declaration, re-exports, StoreConfig with new validation fields
- `crates/pmcp-tasks/src/domain/record.rs` - Serialize/Deserialize derives, version field, variable validation functions
- `crates/pmcp-tasks/src/error.rs` - ConcurrentModification and StorageFull variants with Display and error_code
- `crates/pmcp-tasks/src/lib.rs` - Crate-level re-exports for StorageBackend, StorageError, VersionedRecord

## Decisions Made
- Used monotonic u64 version numbers for CAS (simpler than content hashes, maps naturally to DynamoDB and Redis)
- Composite string keys `{owner_id}:{task_id}` -- universally supported across all target backends
- Kept `DateTime<Utc>` for `expires_at` with chrono serde (auto ISO 8601 serialization) rather than converting to String
- `version` field on TaskRecord uses `#[serde(skip)]` since it is a storage-layer concern managed by the backend
- Variable validation defaults: depth limit 10 levels, string length limit 65,536 bytes (64 KB)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- StorageBackend trait is ready for GenericTaskStore (Plan 02) to build domain logic on top of
- Key helper functions are ready for use in GenericTaskStore's key construction
- TaskRecord serialization is ready for canonical JSON storage at the GenericTaskStore boundary
- Error mapping pattern (StorageError -> TaskError) is established for GenericTaskStore to implement
- All existing tests (506) continue to pass -- no regressions

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 09-storage-abstraction-layer*
*Completed: 2026-02-24*
