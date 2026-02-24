---
phase: 13-feature-flag-verification
plan: 01
subsystem: testing
tags: [feature-flags, rustdoc, cargo-doc, clippy, ci, makefile, pmcp-tasks]

# Dependency graph
requires:
  - phase: 12-redis-backend
    provides: Redis backend behind redis feature flag
  - phase: 11-dynamodb-backend
    provides: DynamoDB backend behind dynamodb feature flag
provides:
  - Zero broken intra-doc links across all 4 feature flag combinations
  - make test-feature-flags target for local feature-flag verification
  - CI feature-flags job for automated regression prevention
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: [feature-flag-safe doc-links using plain backtick text for gated types, full crate paths for in-crate cross-module doc-links]

key-files:
  created: []
  modified:
    - crates/pmcp-tasks/src/store/mod.rs
    - crates/pmcp-tasks/src/types/workflow.rs
    - crates/pmcp-tasks/src/router.rs
    - Makefile
    - .github/workflows/ci.yml

key-decisions:
  - "Plain backtick references for feature-gated types (DynamoDbBackend, RedisBackend) instead of cfg_attr conditional doc links"
  - "Full crate paths (crate::store::generic::GenericTaskStore) for in-crate cross-module doc-links"
  - "Full crate paths for WorkflowProgress/WORKFLOW_PROGRESS_KEY in router.rs (types are in scope via crate::types::workflow)"

patterns-established:
  - "Feature-gated doc-links: Use plain backtick text for types behind feature flags to avoid broken links"
  - "Feature-flag verification: 5 checks per combination (check, clippy, test --no-run, test --doc, cargo doc -D warnings)"

requirements-completed: [TEST-04]

# Metrics
duration: 4min
completed: 2026-02-24
---

# Phase 13 Plan 01: Feature Flag Verification Summary

**Zero broken doc-links across all 4 pmcp-tasks feature combos with automated make/CI verification target**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-24T06:14:02Z
- **Completed:** 2026-02-24T06:18:30Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Fixed all 7 broken intra-doc links in pmcp-tasks across 3 source files
- All 4 feature flag combinations (none, dynamodb, redis, both) now generate clean documentation with -D warnings
- Added `make test-feature-flags` target running 5 checks per combination (20 total verifications)
- Added CI `feature-flags` job for automated regression prevention on every push/PR

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix all broken intra-doc links in pmcp-tasks** - `6a0dad1` (fix)
2. **Task 2: Add feature-flag verification to Makefile and CI** - `8025940` (chore)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/mod.rs` - Fixed DynamoDbBackend/RedisBackend feature-gated links and GenericTaskStore paths (2 locations)
- `crates/pmcp-tasks/src/types/workflow.rs` - Converted cross-crate SequentialWorkflow and WorkflowStep references to plain backtick text
- `crates/pmcp-tasks/src/router.rs` - Added full crate paths for WorkflowProgress and WORKFLOW_PROGRESS_KEY doc-links
- `Makefile` - Added test-feature-flags target with 4-combination verification matrix and help entry
- `.github/workflows/ci.yml` - Added feature-flags CI job with cargo cache and make test-feature-flags

## Decisions Made
- Used plain backtick references for feature-gated types (DynamoDbBackend, RedisBackend) rather than complex `#[cfg_attr(feature=..., doc=...)]` conditional doc-links. Simpler and works across all feature states.
- Used full crate paths for WorkflowProgress and WORKFLOW_PROGRESS_KEY in router.rs since these types are in scope via crate paths but not bare identifiers at the method scope.
- Used `-D warnings` for clippy in the feature-flag target (not the full pedantic/nursery allow list) since pmcp-tasks is a clean crate that doesn't trigger those lints.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 13 is the final phase. All feature flag combinations are verified and CI will prevent regression.
- The project now has automated verification that each backend compiles independently.

## Self-Check: PASSED

All 5 modified files verified on disk. Both task commits (6a0dad1, 8025940) verified in git log.

---
*Phase: 13-feature-flag-verification*
*Completed: 2026-02-24*
