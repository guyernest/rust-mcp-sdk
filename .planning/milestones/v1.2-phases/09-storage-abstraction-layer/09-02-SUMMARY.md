---
phase: 09-storage-abstraction-layer
plan: 02
subsystem: database
tags: [generic-task-store, cas, blanket-impl, type-erasure, domain-logic, serde]

# Dependency graph
requires:
  - phase: 09-01
    provides: "StorageBackend trait, StorageError, VersionedRecord, key helpers, TaskRecord serde"
provides:
  - "GenericTaskStore<B: StorageBackend> with all 11 domain operations"
  - "Blanket TaskStore impl for GenericTaskStore (type erasure via Arc<dyn TaskStore>)"
  - "CAS-based mutations via put_if_version on all write operations"
  - "Canonical JSON serialization at the storage boundary"
  - "Variable validation (depth bombs, long strings) enforced before serialization"
  - "Owner isolation via composite key scoping + defense-in-depth check"
affects: [10-in-memory-backend, 11-dynamodb-backend, 12-redis-backend]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "GenericTaskStore delegates all storage to B: StorageBackend via composite keys"
    - "Blanket impl<B: StorageBackend + 'static> TaskStore for GenericTaskStore<B>"
    - "Clone-check-commit pattern for variable merge with size validation"
    - "All mutations use put_if_version (CAS), create uses plain put"
    - "Owner mismatch returns NotFound (never reveals task existence)"

key-files:
  created:
    - "crates/pmcp-tasks/src/store/generic.rs"
  modified:
    - "crates/pmcp-tasks/src/store/mod.rs"
    - "crates/pmcp-tasks/src/lib.rs"

key-decisions:
  - "GenericTaskStore default poll interval is 500ms (vs InMemoryTaskStore's 5000ms)"
  - "TestBackend uses DashMap for CAS testing -- minimal, not the Phase 10 InMemoryBackend"
  - "Blanket impl requires B: StorageBackend + 'static for Arc<dyn TaskStore> compatibility"
  - "Variable schema validation (depth + string length) applied to incoming variables before merge"

patterns-established:
  - "3-layer architecture: TaskStore trait -> GenericTaskStore -> StorageBackend"
  - "Domain logic centralized in GenericTaskStore, backends remain dumb KV stores"
  - "Type erasure via blanket impl enables Arc<dyn TaskStore> for TaskContext and TaskRouterImpl"

requirements-completed: [ABST-02, ABST-04]

# Metrics
duration: 6min
completed: 2026-02-24
---

# Phase 9 Plan 02: GenericTaskStore and TaskStore Blanket Impl Summary

**GenericTaskStore with 11 CAS-backed domain operations, blanket TaskStore impl for type erasure, and 30 unit tests including CAS conflict and type-erasure verification**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-24T00:54:29Z
- **Completed:** 2026-02-24T01:00:53Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- GenericTaskStore<B: StorageBackend> implementing all 11 domain operations (create, get, update_status, set_variables, set_result, get_result, complete_with_result, list, cancel, cleanup_expired, config)
- All mutation operations use put_if_version (CAS) for optimistic concurrency control
- Owner isolation via composite key scoping plus defense-in-depth owner_id verification
- Canonical JSON serialization/deserialization at the storage boundary
- Variable validation: depth bomb detection, long string rejection, total size enforcement
- Clone-check-commit pattern for variable merge with null-deletion semantics
- TTL hard reject (no silent clamping) per locked decision
- Blanket TaskStore impl for GenericTaskStore<B: StorageBackend + 'static>
- GenericTaskStore re-exported from crate root
- TaskStore trait docs updated to describe type-erasure role
- Module docs updated to describe 3-layer architecture
- 30 new tests (29 domain logic + 1 type-erasure), all 536 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement GenericTaskStore with all domain logic** - `6b33d8d` (feat)
2. **Task 2: Blanket TaskStore impl and re-exports** - `0816bcf` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/generic.rs` - GenericTaskStore<B> with all domain operations, TestBackend, CasConflictBackend, 30 unit tests
- `crates/pmcp-tasks/src/store/mod.rs` - Blanket impl, updated module and trait docs, 3-layer architecture description
- `crates/pmcp-tasks/src/lib.rs` - GenericTaskStore re-export from crate root

## Decisions Made
- GenericTaskStore default poll interval is 500ms (different from InMemoryTaskStore's 5000ms) -- appropriate for CAS-backed stores
- TestBackend is a minimal DashMap-based backend for testing domain logic -- not the full InMemoryBackend planned for Phase 10
- Blanket impl uses `B: StorageBackend + 'static` bound which is required for `Arc<dyn TaskStore>` compatibility
- Variable schema validation (depth + string length) applied to incoming variables map before merge, not after -- this prevents validation on existing stored data that was valid at insert time

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- GenericTaskStore is ready for InMemoryBackend (Phase 10) to plug in as a StorageBackend
- Blanket impl proves any StorageBackend implementation automatically gets full TaskStore functionality
- TestBackend pattern can be reused for integration testing in future phases
- All existing code (TaskContext, TaskRouterImpl, InMemoryTaskStore) continues to work unchanged

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 09-storage-abstraction-layer*
*Completed: 2026-02-24*
