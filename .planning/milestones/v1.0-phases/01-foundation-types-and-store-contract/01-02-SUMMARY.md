---
phase: 01-foundation-types-and-store-contract
plan: 02
subsystem: store
tags: [async-trait, domain-types, task-store, pagination, ttl, variables, uuid, chrono]

# Dependency graph
requires:
  - "01-01: Wire types (Task, TaskStatus, TaskError)"
provides:
  - "TaskRecord domain type with owner_id, variables, result, request_method, expires_at"
  - "TaskWithVariables domain type with _meta variable injection"
  - "TaskStore async trait with 11 methods for full task lifecycle management"
  - "StoreConfig with configurable variable size limits and TTL bounds"
  - "ListTasksOptions and TaskPage for cursor-based pagination"
affects: [01-03-serialization-tests, 02-in-memory-backend, 03-server-integration, 04-dynamodb-backend]

# Tech tracking
tech-stack:
  added: []
  patterns: [domain-vs-wire-types, variables-in-meta-top-level, cursor-based-pagination, atomic-complete-with-result]

key-files:
  created:
    - crates/pmcp-tasks/src/domain/mod.rs
    - crates/pmcp-tasks/src/domain/record.rs
    - crates/pmcp-tasks/src/domain/variables.rs
    - crates/pmcp-tasks/src/store.rs
  modified:
    - crates/pmcp-tasks/src/lib.rs

key-decisions:
  - "Variables injected at top level of _meta (not nested under PMCP key) per locked design decision"
  - "TaskRecord fields all public for store implementor access"
  - "TaskStore config() is sync (not async) since it returns a reference to configuration"
  - "StoreConfig defaults: 1MB variable limit, 1h default TTL, 24h max TTL"

patterns-established:
  - "Domain types (TaskRecord) wrap wire types (Task) with internal fields"
  - "Variable injection into _meta happens at serialization boundary, not storage"
  - "Store trait methods return Result<_, TaskError> with documented error variants"
  - "complete_with_result requires atomicity guarantee from all implementations"

requirements-completed: [STOR-01, STOR-02, STOR-03, STOR-04, HNDL-01]

# Metrics
duration: 4min
completed: 2026-02-21
---

# Phase 1 Plan 02: Domain Types and Store Contract Summary

**TaskRecord/TaskWithVariables domain types with TaskStore async trait defining 11-method storage contract including atomic complete_with_result**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-21T23:08:29Z
- **Completed:** 2026-02-21T23:12:32Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created TaskRecord domain type with all required fields (task, owner_id, variables, result, request_method, expires_at) and UUIDv4 task ID generation
- Created TaskWithVariables domain type that injects variables into _meta at top level per locked design decision
- Defined TaskStore async trait with all 11 methods, comprehensive doc comments explaining behavior, error conditions, and atomicity guarantees
- Added StoreConfig, ListTasksOptions, and TaskPage supporting types for configurable limits and cursor-based pagination
- 30 new unit tests + 10 new doctests (cumulative: 76 unit tests + 33 doctests passing)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create TaskRecord and TaskWithVariables domain types** - `928547b` (feat)
2. **Task 2: Create TaskStore trait and supporting pagination types** - `fc16dad` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/src/domain/mod.rs` - Domain module declarations and re-exports
- `crates/pmcp-tasks/src/domain/record.rs` - TaskRecord struct with constructor, is_expired, to_wire_task, to_wire_task_with_variables
- `crates/pmcp-tasks/src/domain/variables.rs` - TaskWithVariables with from_record and to_wire_task (variable injection)
- `crates/pmcp-tasks/src/store.rs` - TaskStore async trait, StoreConfig, ListTasksOptions, TaskPage
- `crates/pmcp-tasks/src/lib.rs` - Updated module declarations and re-exports for domain and store

## Decisions Made
- Variables injected at top level of _meta per locked design decision (not nested under a PMCP-specific key)
- TaskRecord fields are all public so store implementors have full access (consistent with CONTEXT.md)
- StoreConfig defaults chosen as 1MB variable limit, 1h default TTL, 24h max TTL (reasonable production defaults)
- TaskStore::config() is sync (not async) since it returns a reference; all other methods are async

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Domain types and store trait ready for Plan 03 (serialization tests)
- TaskStore trait ready for Phase 2 (in-memory backend implementation)
- TaskRecord and TaskWithVariables ready for Phase 3 (server integration, handler context)

## Self-Check: PASSED

All 5 created/modified files verified on disk. Both task commits (928547b, fc16dad) verified in git log. 76 unit tests + 33 doctests passing. Zero clippy warnings, zero doc warnings.

---
*Phase: 01-foundation-types-and-store-contract*
*Completed: 2026-02-21*
