---
phase: 10-inmemory-backend-refactor
plan: 01
subsystem: database
tags: [dashmap, storage-backend, generic-task-store, in-memory, refactor]

# Dependency graph
requires:
  - phase: 09-storage-abstraction-layer
    provides: GenericTaskStore<B> with all domain logic, StorageBackend trait, blanket TaskStore impl
provides:
  - InMemoryBackend implementing StorageBackend with DashMap
  - InMemoryTaskStore as thin wrapper around GenericTaskStore<InMemoryBackend>
  - backend() accessor on GenericTaskStore for test access
  - InMemoryBackend publicly re-exported from pmcp-tasks
affects: [10-inmemory-backend-refactor, 11-dynamodb-backend, 12-redis-backend]

# Tech tracking
tech-stack:
  added: []
  patterns: [thin-wrapper-delegation, backend-accessor-for-tests]

key-files:
  created: []
  modified:
    - crates/pmcp-tasks/src/store/memory.rs
    - crates/pmcp-tasks/src/store/generic.rs
    - crates/pmcp-tasks/src/store/mod.rs
    - crates/pmcp-tasks/src/lib.rs

key-decisions:
  - "Thin wrapper over type alias for InMemoryTaskStore to preserve zero-arg new() and Default"
  - "Keep 5000ms poll interval default in InMemoryTaskStore to avoid test churn"
  - "InMemoryBackend is public (pub) for downstream GenericTaskStore usage"
  - "Added backend() accessor to GenericTaskStore (not just InMemoryTaskStore) for universal test access"
  - "Behavioral test for with_security instead of field inspection (security field now private)"

patterns-established:
  - "Thin wrapper pattern: newtype with inner GenericTaskStore<Backend> and TaskStore delegation"
  - "Backend accessor: GenericTaskStore::backend() returns &B for test introspection"
  - "Force-expiry helper: async fn using backend put_if_version to rewrite records with past timestamps"

requirements-completed: [IMEM-01, IMEM-02, IMEM-03]

# Metrics
duration: 7min
completed: 2026-02-24
---

# Phase 10 Plan 01: InMemory Backend Refactor Summary

**InMemoryTaskStore rewritten as thin wrapper around GenericTaskStore<InMemoryBackend>, eliminating ~600 lines of duplicated domain logic**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-24T01:54:39Z
- **Completed:** 2026-02-24T02:02:00Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments
- Created `InMemoryBackend` implementing all 6 `StorageBackend` methods with DashMap (promoted from TestBackend)
- Rewrote `InMemoryTaskStore` as a thin wrapper around `GenericTaskStore<InMemoryBackend>` with all builder methods preserved
- All 543 tests pass (270 unit + 273 integration/doc), zero clippy warnings, clean formatting
- Net reduction of ~93 lines (373 added, 466 removed) by eliminating duplicated domain logic

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement InMemoryBackend and rewrite InMemoryTaskStore as thin wrapper** - `fac9bab` (feat)

**Plan metadata:** [pending] (docs: complete plan)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/memory.rs` - Complete rewrite: InMemoryBackend + InMemoryTaskStore wrapper + TaskStore delegation + adapted tests
- `crates/pmcp-tasks/src/store/generic.rs` - Added `backend()` accessor method to GenericTaskStore, formatting cleanup
- `crates/pmcp-tasks/src/store/mod.rs` - Updated module docs: removed Legacy section, added Backends section mentioning InMemoryBackend
- `crates/pmcp-tasks/src/lib.rs` - Added `InMemoryBackend` to public re-exports

## Decisions Made
- **Thin wrapper over type alias:** Preserves zero-arg `new()`, `Default` impl, and existing doctests without breaking any call sites
- **5000ms poll interval preserved:** InMemoryTaskStore::new() explicitly sets `with_poll_interval(5000)` to match the legacy default, avoiding churn on 4+ test assertions
- **InMemoryBackend is public:** Allows downstream users to create `GenericTaskStore<InMemoryBackend>` with custom configurations
- **backend() on GenericTaskStore:** Added as a general-purpose accessor (not just `#[cfg(test)]`) so InMemoryTaskStore can delegate; `InMemoryTaskStore::backend()` is `#[cfg(test)]` gated
- **Behavioral test for security:** `with_security_sets_security` test now verifies behavior (creates tasks up to limit, next fails) instead of inspecting the private `security` field

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added backend() accessor to GenericTaskStore**
- **Found during:** Task 1 (InMemoryTaskStore wrapper implementation)
- **Issue:** InMemoryTaskStore::backend() needs to access GenericTaskStore's inner backend, but no accessor existed
- **Fix:** Added `pub fn backend(&self) -> &B` to GenericTaskStore<B>
- **Files modified:** `crates/pmcp-tasks/src/store/generic.rs`
- **Verification:** All tests pass, clippy clean
- **Committed in:** fac9bab (Task 1 commit)

**2. [Rule 1 - Bug] Formatting cleanup in generic.rs**
- **Found during:** Task 1 (cargo fmt --check)
- **Issue:** `cargo fmt` flagged pre-existing formatting issues in generic.rs test code (line length, chain formatting)
- **Fix:** Applied `cargo fmt -p pmcp-tasks` to resolve all formatting issues
- **Files modified:** `crates/pmcp-tasks/src/store/generic.rs`
- **Verification:** `cargo fmt --check` passes
- **Committed in:** fac9bab (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for compilation and quality gate compliance. No scope creep.

## Issues Encountered
None - plan executed smoothly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- InMemoryBackend and InMemoryTaskStore wrapper complete and tested
- Ready for Plan 02 (TestBackend replacement in generic.rs, per-backend contract tests)
- GenericTaskStore::backend() accessor available for test code in all backend implementations

## Self-Check: PASSED

- FOUND: crates/pmcp-tasks/src/store/memory.rs
- FOUND: crates/pmcp-tasks/src/store/generic.rs
- FOUND: crates/pmcp-tasks/src/store/mod.rs
- FOUND: crates/pmcp-tasks/src/lib.rs
- FOUND: SUMMARY.md
- FOUND: commit fac9bab

---
*Phase: 10-inmemory-backend-refactor*
*Completed: 2026-02-24*
