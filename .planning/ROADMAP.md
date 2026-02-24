# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- 🚧 **v1.2 Pluggable Storage Backends** — Phases 9-13 (in progress)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

### 🚧 v1.2 Pluggable Storage Backends (In Progress)

**Milestone Goal:** Introduce a pluggable KV storage backend layer, refactor TaskStore to delegate to it, and validate with DynamoDB + Redis implementations.

- [x] **Phase 9: Storage Abstraction Layer** - StorageBackend trait and GenericTaskStore with all domain logic (completed 2026-02-24)
- [x] **Phase 10: InMemory Backend Refactor** - Reimplement InMemoryTaskStore as GenericTaskStore\<InMemoryBackend\> (completed 2026-02-24)
- [x] **Phase 11: DynamoDB Backend** - Full DynamoDbBackend behind `dynamodb` feature flag (completed 2026-02-24)
- [ ] **Phase 12: Redis Backend** - Full RedisBackend behind `redis` feature flag
- [ ] **Phase 13: Feature Flag Verification** - Cross-backend compilation and feature flag isolation

## Phase Details

### Phase 9: Storage Abstraction Layer
**Goal**: Developers have a well-defined KV storage contract and a generic task store that implements all domain logic once, backend-agnostically
**Depends on**: Phase 8 (v1.1 complete)
**Requirements**: ABST-01, ABST-02, ABST-03, ABST-04
**Success Criteria** (what must be TRUE):
  1. A `StorageBackend` trait exists with KV operations (get, put, put-if-version, delete, list-by-prefix, cleanup-expired) that any backend can implement
  2. A `GenericTaskStore<B: StorageBackend>` implements all TaskStore domain logic (state machine transitions, owner isolation, variable merge, TTL enforcement) by delegating storage to the backend
  3. Canonical JSON serialization in GenericTaskStore ensures identical round-trip behavior regardless of which backend is plugged in
  4. The existing TaskStore trait is simplified or redesigned to leverage the new KV backend pattern
**Plans**: 2 plans

Plans:
- [ ] 09-01-PLAN.md — StorageBackend trait, StorageError, VersionedRecord, TaskRecord serialization, error variants, variable validation
- [ ] 09-02-PLAN.md — GenericTaskStore with all domain logic, TaskStore trait redesign with blanket impl

### Phase 10: InMemory Backend Refactor
**Goal**: The existing InMemoryTaskStore is replaced by GenericTaskStore\<InMemoryBackend\> with zero behavioral changes -- all 200+ existing tests pass unchanged
**Depends on**: Phase 9
**Requirements**: IMEM-01, IMEM-02, IMEM-03, TEST-01
**Success Criteria** (what must be TRUE):
  1. An `InMemoryBackend` struct implements `StorageBackend` using DashMap for concurrent KV storage
  2. `InMemoryTaskStore` is now a type alias or thin wrapper around `GenericTaskStore<InMemoryBackend>` with backward-compatible constructors (`new`, `with_config`)
  3. All 200+ existing unit, property, integration, and security tests pass without modification
  4. Per-backend unit tests validate InMemoryBackend's StorageBackend contract independently
**Plans**: 2 plans

Plans:
- [ ] 10-01-PLAN.md — InMemoryBackend + InMemoryTaskStore thin wrapper + test adaptation
- [ ] 10-02-PLAN.md — Per-backend StorageBackend contract tests + TestBackend replacement in generic.rs

### Phase 11: DynamoDB Backend
**Goal**: Developers can persist tasks in DynamoDB for production AWS/Lambda deployments by enabling the `dynamodb` feature flag
**Depends on**: Phase 10
**Requirements**: DYNA-01, DYNA-02, DYNA-03, DYNA-04, DYNA-05, DYNA-06, TEST-02
**Success Criteria** (what must be TRUE):
  1. `DynamoDbBackend` implements `StorageBackend` behind a `dynamodb` feature flag, using the `aws-sdk-dynamodb` crate
  2. Tasks are stored in a single-table design with composite keys that enforce owner isolation at the storage level
  3. State transitions use `ConditionExpression` for atomic compare-and-set, preventing concurrent mutation corruption
  4. Expired tasks are automatically cleaned up via native DynamoDB TTL (epoch seconds)
  5. Variable payloads are capped at ~350KB to stay within DynamoDB's 400KB item limit, with a clear error when exceeded
**Plans**: 2 plans

Plans:
- [ ] 11-01-PLAN.md — Feature flag setup, DynamoDbBackend struct, and all 6 StorageBackend method implementations
- [ ] 11-02-PLAN.md — Integration tests against real DynamoDB, gated behind dynamodb-tests feature flag

### Phase 12: Redis Backend
**Goal**: Developers can persist tasks in Redis for long-running server deployments by enabling the `redis` feature flag, proving the StorageBackend trait generalizes beyond DynamoDB
**Depends on**: Phase 10
**Requirements**: RDIS-01, RDIS-02, RDIS-03, RDIS-04, RDIS-05, TEST-03
**Success Criteria** (what must be TRUE):
  1. `RedisBackend` implements `StorageBackend` behind a `redis` feature flag, using the `redis` crate with async/tokio support
  2. Task records are stored as Redis hashes with field-level mapping for efficient partial reads
  3. Concurrent mutations are protected by Lua scripts that atomically check version and set new values
  4. Task expiry uses EXPIRE-based TTL with application-level enforcement for consistent semantics across get/list operations
  5. Owner-scoped task listing is supported via sorted set indexing
**Plans**: 2 plans

Plans:
- [ ] 12-01-PLAN.md — Feature flag setup, RedisBackend struct with all 6 StorageBackend methods, Lua scripts, sorted set indexing
- [ ] 12-02-PLAN.md — Integration tests against real Redis, gated behind redis-tests feature flag

### Phase 13: Feature Flag Verification
**Goal**: All backends compile independently and in combination under their respective feature flags, with no cross-contamination between feature-gated code
**Depends on**: Phase 11, Phase 12
**Requirements**: TEST-04
**Success Criteria** (what must be TRUE):
  1. The crate compiles with no feature flags (default: InMemoryBackend only)
  2. The crate compiles with only `dynamodb` enabled (InMemory + DynamoDB)
  3. The crate compiles with only `redis` enabled (InMemory + Redis)
  4. The crate compiles with both `dynamodb` and `redis` enabled (all backends)
**Plans**: TBD

Plans:
- [ ] 13-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 9 -> 10 -> 11 -> 12 -> 13
Note: Phase 11 (DynamoDB) and Phase 12 (Redis) both depend on Phase 10 but not on each other. They could execute in either order; DynamoDB is sequenced first as the primary production target.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types and Store Contract | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend and Owner Security | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Handler, Middleware, and Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types and Contracts | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Partial Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Structured Handoff and Client Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration and End-to-End Validation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish and Test Coverage | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction Layer | v1.2 | Complete    | 2026-02-24 | - |
| 10. InMemory Backend Refactor | 2/2 | Complete    | 2026-02-24 | - |
| 11. DynamoDB Backend | 2/2 | Complete    | 2026-02-24 | - |
| 12. Redis Backend | 1/2 | In Progress|  | - |
| 13. Feature Flag Verification | v1.2 | 0/? | Not started | - |
