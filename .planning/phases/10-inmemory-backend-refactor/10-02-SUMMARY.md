---
phase: 10-inmemory-backend-refactor
plan: 02
subsystem: database
tags: [storage-backend, in-memory, contract-tests, test-dedup, dashmap]

# Dependency graph
requires:
  - phase: 10-inmemory-backend-refactor
    plan: 01
    provides: InMemoryBackend implementing StorageBackend, InMemoryTaskStore thin wrapper
provides:
  - 18 per-backend StorageBackend contract tests for InMemoryBackend
  - Single source of truth for in-memory backend (TestBackend eliminated)
  - generic.rs tests using InMemoryBackend instead of duplicated TestBackend
affects: [11-dynamodb-backend, 12-redis-backend]

# Tech tracking
tech-stack:
  added: []
  patterns: [per-backend-contract-tests, single-source-of-truth-backend]

key-files:
  created: []
  modified:
    - crates/pmcp-tasks/src/store/memory.rs
    - crates/pmcp-tasks/src/store/generic.rs

key-decisions:
  - "Per-backend contract tests in separate mod backend_tests alongside existing mod tests in memory.rs"
  - "Eliminated ~110 lines of duplicated TestBackend code from generic.rs"
  - "CasConflictBackend retained (tests GenericTaskStore CAS error handling, not backend behavior)"

patterns-established:
  - "Per-backend contract test module: mod backend_tests testing StorageBackend trait directly"
  - "Contract test coverage: all 6 StorageBackend methods with happy paths and error cases"

requirements-completed: [TEST-01, IMEM-01]

# Metrics
duration: 4min
completed: 2026-02-24
---

# Phase 10 Plan 02: Per-Backend Contract Tests and TestBackend Elimination Summary

**18 StorageBackend contract tests validating InMemoryBackend directly, plus TestBackend removal from generic.rs for single source of truth**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-24T02:04:45Z
- **Completed:** 2026-02-24T02:09:13Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added 18 per-backend StorageBackend contract tests covering all 6 methods (get, put, put_if_version, delete, list_by_prefix, cleanup_expired) with happy paths and error cases
- Removed entire TestBackend struct and StorageBackend impl from generic.rs (~110 lines eliminated)
- All generic.rs tests now use InMemoryBackend as single source of truth
- Full test suite passes: 561 tests (288 unit + integration/doc), zero clippy warnings, clean formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: Add per-backend StorageBackend contract tests for InMemoryBackend** - `3fde4b3` (test)
2. **Task 2: Replace TestBackend in generic.rs with InMemoryBackend** - `e84d72c` (feat)

**Plan metadata:** [pending] (docs: complete plan)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/memory.rs` - Added `mod backend_tests` with 18 contract tests exercising InMemoryBackend directly through StorageBackend trait
- `crates/pmcp-tasks/src/store/generic.rs` - Removed TestBackend (struct + impl), replaced all usages with InMemoryBackend, updated CasConflictBackend to wrap Arc<InMemoryBackend>, removed unused dashmap import

## Decisions Made
- **Separate test module:** Per-backend tests in `mod backend_tests` (not merged into existing `mod tests`) to keep StorageBackend contract tests cleanly separated from InMemoryTaskStore wrapper tests
- **CasConflictBackend retained:** It tests GenericTaskStore's error mapping for CAS conflicts, not a specific backend -- wrapping InMemoryBackend instead of TestBackend preserves the test's purpose
- **Contract test completeness:** 18 tests covering 2 get + 3 put + 4 put_if_version + 3 delete + 3 list_by_prefix + 3 cleanup_expired, matching the plan's ~15-18 target

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - plan executed smoothly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 10 complete: InMemoryBackend fully tested with both per-backend contract tests and domain-level tests through GenericTaskStore
- Zero duplicated test backend code in the codebase
- Contract test pattern established for future backends (DynamoDB Phase 11, Redis Phase 12)
- All 561 tests passing, zero warnings

---
*Phase: 10-inmemory-backend-refactor*
*Completed: 2026-02-24*
