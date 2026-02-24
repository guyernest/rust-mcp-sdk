---
phase: 11-dynamodb-backend
plan: 01
subsystem: database
tags: [dynamodb, aws-sdk, storage-backend, feature-flag, cas, ttl]

# Dependency graph
requires:
  - phase: 09-storage-abstraction-layer
    provides: StorageBackend trait with 6 methods, VersionedRecord, StorageError, composite key format
  - phase: 10-inmemory-backend-refactor
    provides: InMemoryBackend reference implementation, GenericTaskStore, per-backend contract test pattern
provides:
  - DynamoDbBackend struct implementing StorageBackend behind dynamodb feature flag
  - dynamodb and dynamodb-tests feature flags in pmcp-tasks crate
  - Single-table DynamoDB key schema (PK=OWNER#owner_id, SK=TASK#task_id)
  - CAS via ConditionExpression on version attribute
  - TTL via expires_at epoch seconds attribute (DynamoDB native TTL)
  - Conditional re-export in lib.rs
affects: [11-02 integration tests, phase-12 redis backend pattern reference]

# Tech tracking
tech-stack:
  added: [aws-sdk-dynamodb v1 (optional), aws-config v1 (optional)]
  patterns: [feature-gated backend module, DynamoDB single-table design with composite PK/SK, CAS via ConditionExpression, epoch-seconds TTL attribute]

key-files:
  created:
    - crates/pmcp-tasks/src/store/dynamodb.rs
  modified:
    - crates/pmcp-tasks/Cargo.toml
    - crates/pmcp-tasks/src/store/mod.rs
    - crates/pmcp-tasks/src/lib.rs

key-decisions:
  - "No extra GetItem on CAS failure: report actual=expected in VersionConflict rather than an extra read for the real version"
  - "Store data as AttributeValue::S (String) for DynamoDB console readability, not Binary"
  - "extract_ttl_epoch parses expiresAt from JSON data blob rather than accepting TTL as a method parameter"
  - "list_by_prefix uses Query with manual pagination loop (not paginator helper) for explicit control"
  - "Unconditional put does GetItem + PutItem (two API calls) to maintain monotonic version chain"

patterns-established:
  - "Feature-gated backend: #[cfg(feature = \"dynamodb\")] pub mod dynamodb in store/mod.rs"
  - "DynamoDB key helpers: make_pk/make_sk/parse_pk/parse_sk/split_key/split_prefix as private functions"
  - "map_sdk_error: generic error mapping from AWS SDK errors to StorageError::Backend"
  - "TTL extraction: parse serialized JSON to extract expiresAt as epoch seconds for DynamoDB native TTL"

requirements-completed: [DYNA-01, DYNA-02, DYNA-03, DYNA-04, DYNA-05]

# Metrics
duration: 5min
completed: 2026-02-24
---

# Phase 11 Plan 01: DynamoDB Backend Implementation Summary

**DynamoDbBackend implementing all 6 StorageBackend methods with feature-gated aws-sdk-dynamodb, single-table PK/SK design, CAS via ConditionExpression, and DynamoDB native TTL**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-24T03:53:41Z
- **Completed:** 2026-02-24T03:58:44Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- DynamoDbBackend struct with 3 constructors (new, from_env, from_env_with_table) and 8 private helpers
- All 6 StorageBackend methods: get (GetItem), put (GetItem+PutItem), put_if_version (PutItem with ConditionExpression), delete (DeleteItem with ReturnValue::AllOld), list_by_prefix (Query with pagination), cleanup_expired (no-op for DynamoDB native TTL)
- Feature flag infrastructure: dynamodb and dynamodb-tests features with optional aws-sdk-dynamodb and aws-config dependencies
- Conditional module declaration and re-export in store/mod.rs and lib.rs
- 455 lines of comprehensively documented code with module-level and item-level rustdoc including no_run examples
- All 561 existing tests pass with zero regressions, zero clippy warnings, clean formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: Feature flag setup and DynamoDbBackend struct with all 6 StorageBackend methods** - `757c8f9` (feat)

**Note:** Tasks 1 and 2 were implemented as a single cohesive module since the struct, helpers, and trait implementation form one natural unit. All 6 methods were included in the Task 1 commit.

## Files Created/Modified
- `crates/pmcp-tasks/src/store/dynamodb.rs` - DynamoDbBackend struct implementing StorageBackend with all 6 methods (455 lines)
- `crates/pmcp-tasks/Cargo.toml` - Added dynamodb/dynamodb-tests feature flags with optional aws-sdk-dynamodb and aws-config deps
- `crates/pmcp-tasks/src/store/mod.rs` - Added conditional `#[cfg(feature = "dynamodb")] pub mod dynamodb;` and updated module docs
- `crates/pmcp-tasks/src/lib.rs` - Added conditional `#[cfg(feature = "dynamodb")] pub use store::dynamodb::DynamoDbBackend;`

## Decisions Made
- **No extra read on CAS failure:** When put_if_version gets ConditionalCheckFailedException, we report actual=expected in VersionConflict rather than doing an extra GetItem to discover the real version. This avoids latency and another race window. The caller already knows to retry.
- **String data storage:** Data stored as AttributeValue::S (not B) for human-readability in the DynamoDB console, per locked decision.
- **TTL extraction from JSON blob:** extract_ttl_epoch parses the serialized TaskRecord JSON to find the expiresAt field and convert it to epoch seconds. This keeps the StorageBackend interface unchanged.
- **Manual pagination in list_by_prefix:** Used explicit loop with exclusive_start_key rather than the SDK's paginator helper, for explicit control over the iteration and error handling.
- **Unconditional put uses GetItem + PutItem:** Two API calls to maintain the monotonic version chain. This is acceptable since put is only called by GenericTaskStore for create operations.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed SDK API differences from research code examples**
- **Found during:** Task 1
- **Issue:** The research code used `if let Some(items) = output.items()` but aws-sdk-dynamodb v1.101 returns `&[HashMap]` directly (not `Option`). Also needed explicit type annotations for closures due to `HashMap<String, AttributeValue>` type inference.
- **Fix:** Changed to `for item in output.items()` and added `|v: &AttributeValue|` type annotations in closures.
- **Files modified:** crates/pmcp-tasks/src/store/dynamodb.rs
- **Verification:** cargo check --features dynamodb passes
- **Committed in:** 757c8f9 (Task 1 commit)

**2. [Rule 1 - Bug] Removed redundant Display bound on map_sdk_error**
- **Found during:** Task 1 (clippy check)
- **Issue:** Clippy flagged `std::fmt::Display + std::error::Error` as redundant since Display is a supertrait of Error.
- **Fix:** Removed the `std::fmt::Display +` bound, keeping only `std::error::Error + Send + Sync + 'static`.
- **Files modified:** crates/pmcp-tasks/src/store/dynamodb.rs
- **Verification:** cargo clippy --features dynamodb -- -D warnings passes with zero warnings
- **Committed in:** 757c8f9 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for compilation and zero-warning quality gate. No scope creep.

## Issues Encountered
- aws-sdk-dynamodb resolved to v1.101.0 (not v1.107.0 as specified in research) due to cargo version resolution. This is fine -- the APIs are backward-compatible and the version constraint is `version = "1"`.

## User Setup Required
None - no external service configuration required. The dynamodb feature is optional and only activates when explicitly requested.

## Next Phase Readiness
- DynamoDbBackend compiles and implements all 6 StorageBackend methods
- Ready for plan 11-02: integration tests against real DynamoDB
- Integration tests will need: AWS credentials configured, DynamoDB table provisioned with correct schema

## Self-Check: PASSED

- [x] crates/pmcp-tasks/src/store/dynamodb.rs exists (455 lines)
- [x] crates/pmcp-tasks/Cargo.toml has dynamodb feature flags
- [x] Commit 757c8f9 exists in git log
- [x] .planning/phases/11-dynamodb-backend/11-01-SUMMARY.md exists

---
*Phase: 11-dynamodb-backend*
*Completed: 2026-02-24*
