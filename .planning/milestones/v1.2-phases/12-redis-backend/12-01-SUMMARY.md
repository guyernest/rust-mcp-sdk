---
phase: 12-redis-backend
plan: 01
subsystem: database
tags: [redis, lua-scripts, storage-backend, feature-flag, async-tokio]

# Dependency graph
requires:
  - phase: 09-storage-abstraction-layer
    provides: StorageBackend trait with 6 methods (get, put, put_if_version, delete, list_by_prefix, cleanup_expired)
  - phase: 10-inmemory-backend-refactor
    provides: InMemoryBackend reference, per-backend contract test pattern
  - phase: 11-dynamodb-backend
    provides: DynamoDbBackend reference implementation, feature flag pattern, integration test structure
provides:
  - RedisBackend implementing StorageBackend with Lua atomic scripts
  - redis and redis-tests feature flags
  - Per-owner sorted set indexing for list_by_prefix
  - Application-level expiry filtering for consistent semantics
  - Integration test suite for Redis backend (behind redis-tests flag)
affects: [12-02 redis integration tests, future redis cluster support]

# Tech tracking
tech-stack:
  added: [redis 1.0 (tokio-comp, script features)]
  patterns: [lua-script-atomicity, sorted-set-indexing, orphan-cleanup, absolute-path-crate-imports]

key-files:
  created:
    - crates/pmcp-tasks/src/store/redis.rs
  modified:
    - crates/pmcp-tasks/Cargo.toml
    - crates/pmcp-tasks/src/store/mod.rs
    - crates/pmcp-tasks/src/lib.rs

key-decisions:
  - "Per-owner sorted sets (not global) for O(log N) scoped listing"
  - "EXPIREAT (absolute epoch) maps directly from TaskRecord expiresAt"
  - "Lua script string constants embedded in module (not separate files)"
  - "Lazy orphan cleanup during list_by_prefix (not background process)"
  - "::redis:: absolute path imports to avoid module name collision"
  - "put_if_version on missing key returns VersionConflict (consistent with DynamoDB)"

patterns-established:
  - "Lua script atomicity: hash + sorted set + TTL in single round-trip via redis::Script"
  - "Application-level expiry filtering: check expires_at hash field vs current time before returning"
  - "Orphan cleanup: detect missing hashes during list, batch ZREM orphaned sorted set entries"
  - "Feature flag pattern: redis/redis-tests mirroring dynamodb/dynamodb-tests"

requirements-completed: [RDIS-01, RDIS-02, RDIS-03, RDIS-04, RDIS-05]

# Metrics
duration: 4min
completed: 2026-02-24
---

# Phase 12 Plan 01: Redis Backend Summary

**RedisBackend implementing all 6 StorageBackend methods with Lua atomic scripts, sorted set indexing, and application-level expiry filtering behind the redis feature flag**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-24T05:19:19Z
- **Completed:** 2026-02-24T05:23:49Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments
- Implemented RedisBackend struct with MultiplexedConnection and configurable key prefix
- All 6 StorageBackend methods implemented with 3 Lua scripts for atomic write operations
- Per-owner sorted set indexes for efficient owner-scoped task listing
- Application-level expiry filtering in get and list_by_prefix for consistent semantics
- Feature flags (redis, redis-tests) and conditional module/re-export infrastructure
- Integration test suite (20 tests) mirroring DynamoDB backend test structure

## Task Commits

Each task was committed atomically:

1. **Task 1: Feature flag setup and RedisBackend struct with all 6 StorageBackend methods** - `e97559a` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/src/store/redis.rs` - RedisBackend struct implementing StorageBackend with Lua scripts, helpers, and integration tests (888 lines)
- `crates/pmcp-tasks/Cargo.toml` - redis dependency and redis/redis-tests feature flags
- `crates/pmcp-tasks/src/store/mod.rs` - Conditional redis module declaration and updated docs
- `crates/pmcp-tasks/src/lib.rs` - Conditional RedisBackend re-export

## Decisions Made
- **Per-owner sorted sets**: Each owner gets `{prefix}:idx:{owner_id}` instead of a global sorted set. Avoids scanning irrelevant entries during list_by_prefix.
- **EXPIREAT over EXPIRE**: The `expiresAt` field is an absolute timestamp, so EXPIREAT maps directly without computing a relative offset.
- **Lua script embedding**: String constants (`const LUA_PUT`, etc.) embedded in the module. Simple, grep-able, no file loading overhead.
- **Lazy orphan cleanup**: During list_by_prefix, detect hashes that expired (empty HGETALL) and batch ZREM the orphaned sorted set entries. Best-effort, no background process needed.
- **Absolute crate path**: Use `::redis::` instead of `redis::` to avoid collision between the crate name and the module name.
- **CAS on missing key returns VersionConflict**: Consistent with DynamoDB backend behavior (Phase 11 decision).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required. Redis integration tests require a running Redis instance but are gated behind the `redis-tests` feature flag.

## Next Phase Readiness
- RedisBackend is complete and compiles cleanly with all feature combinations
- Ready for Plan 12-02: Redis backend integration tests (running against a real Redis instance)
- All 561+ existing tests continue to pass without modification

## Self-Check: PASSED

- FOUND: crates/pmcp-tasks/src/store/redis.rs
- FOUND: commit e97559a
- FOUND: .planning/phases/12-redis-backend/12-01-SUMMARY.md

---
*Phase: 12-redis-backend*
*Completed: 2026-02-24*
