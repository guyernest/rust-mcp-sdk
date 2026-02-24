# Phase 12: Redis Backend - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement `RedisBackend` that implements the `StorageBackend` trait (from Phase 9-10) behind a `redis` feature flag, using the `redis` crate with async/tokio support. Developers can persist tasks in Redis for long-running server deployments. This proves the StorageBackend trait generalizes beyond DynamoDB to a fundamentally different storage system.

</domain>

<decisions>
## Implementation Decisions

### Storage model & key schema
- Redis-idiomatic key naming: `pmcp:tasks:{owner_id}:{task_id}` -- colon-separated, follows Redis conventions
- Redis hashes for task storage with field-level mapping (version, data, expires_at as separate hash fields)
- Sorted set index for owner-scoped task listing (RDIS-05)

### TTL & expiry semantics
- cleanup_expired is a no-op returning Ok(0) -- rely on Redis EXPIRE for automatic deletion, mirroring DynamoDB approach
- Application-level filtering on get/list: backend checks stored expires_at against current time to filter expired-but-not-yet-deleted items (consistent semantics per success criteria)

### Integration testing strategy
- Tests run against local Redis (localhost, developer starts Redis manually)
- Gated behind `redis-tests` feature flag: `cargo test --features redis-tests` -- consistent with DynamoDB's `dynamodb-tests` pattern
- Default connection URL overridable via env var

### Lua script design
- All write operations (put, put_if_version, delete) use Lua scripts for atomic operations (hash + sorted set index + TTL in single round-trip)
- Use EVAL (send script text each time), not EVALSHA -- simpler, no registration step, Redis caches scripts internally
- CAS check in Lua for put_if_version: atomically read version, compare, write if match

### Claude's Discretion
- Sorted set index design: per-owner sorted set vs global sorted set
- Hash field layout: which fields are stored as separate hash fields vs JSON blob
- Redis setup: fail-fast connection error behavior (consistent with DynamoDB "table must exist" pattern)
- Test isolation approach: unique key prefix vs FLUSHDB (DynamoDB used UUID prefix)
- Default Redis test URL (localhost:6379 vs 6379/15)
- Lua script embedding: string constants vs separate files
- Lua error propagation: custom return values vs redis.error_reply
- TTL format: EXPIREAT (absolute epoch) vs EXPIRE (relative seconds)
- Index cleanup strategy for orphaned sorted set entries when hashes expire

</decisions>

<specifics>
## Specific Ideas

- The DynamoDbBackend (Phase 11) serves as the reference for how a non-InMemory backend implements StorageBackend -- follow the same structural patterns where applicable
- Phase 9 established: JSON serialization in GenericTaskStore, TaskRecord-based trait, domain-aware errors, no content sanitization
- The `redis` crate with `tokio-comp` feature is the standard async Redis client for Rust
- Per the Phase 9 security posture: owner binding enforced structurally via key prefix, NotFound on owner mismatch

</specifics>

<deferred>
## Deferred Ideas

- Redis Cluster support -- out of scope per REQUIREMENTS.md, single-node sufficient for proving the trait
- ConnectionManager auto-reconnect (ADVN-03) -- listed as future requirement
- `cargo pmcp tasks init --backend redis` CLI command -- future phase alongside DynamoDB equivalent

</deferred>

---

*Phase: 12-redis-backend*
*Context gathered: 2026-02-24*
