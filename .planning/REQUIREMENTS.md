# Requirements: PMCP Tasks — Pluggable Storage Backends

**Defined:** 2026-02-23
**Core Value:** Tool handlers can manage long-running operations through a durable task lifecycle with shared variable state that persists across tool calls.

## v1.2 Requirements

Requirements for pluggable storage backend milestone. Each maps to roadmap phases.

### Storage Abstraction

- [ ] **ABST-01**: StorageBackend trait defines KV operations (get, put, delete, list-by-prefix, cleanup-expired)
- [ ] **ABST-02**: GenericTaskStore implements all TaskStore domain logic (state machine, owner isolation, variable merge, TTL) by delegating to any StorageBackend
- [ ] **ABST-03**: Canonical serialization layer in GenericTaskStore ensures consistent JSON round-trips regardless of backend
- [ ] **ABST-04**: TaskStore trait can be simplified/redesigned to leverage the KV backend pattern

### InMemory Refactor

- [ ] **IMEM-01**: InMemoryBackend implements StorageBackend using DashMap for concurrent KV storage
- [ ] **IMEM-02**: InMemoryTaskStore becomes GenericTaskStore\<InMemoryBackend\> with backward-compatible constructors
- [ ] **IMEM-03**: All existing 200+ tests pass unchanged after the refactor

### DynamoDB Backend

- [ ] **DYNA-01**: DynamoDbBackend implements StorageBackend behind `dynamodb` feature flag
- [ ] **DYNA-02**: Single-table design with composite keys for task storage and owner isolation
- [ ] **DYNA-03**: ConditionExpression for atomic state transitions (concurrent mutation safety)
- [ ] **DYNA-04**: Native DynamoDB TTL for automatic expired task cleanup
- [ ] **DYNA-05**: Automatic variable size cap at ~350KB to stay within DynamoDB's 400KB item limit
- [ ] **DYNA-06**: Cloud-only integration tests against real DynamoDB table

### Redis Backend

- [ ] **RDIS-01**: RedisBackend implements StorageBackend behind `redis` feature flag
- [ ] **RDIS-02**: Hash-based storage mapping task record fields to Redis hash fields
- [ ] **RDIS-03**: Lua scripts for atomic check-and-set operations (concurrent mutation safety)
- [ ] **RDIS-04**: EXPIRE-based TTL with application-level enforcement for consistent expiry semantics
- [ ] **RDIS-05**: Sorted set indexing for owner-scoped task listing

### Testing

- [ ] **TEST-01**: Per-backend unit tests for InMemoryBackend validating StorageBackend contract
- [ ] **TEST-02**: Per-backend integration tests for DynamoDbBackend against cloud DynamoDB
- [ ] **TEST-03**: Per-backend integration tests for RedisBackend against Redis instance
- [ ] **TEST-04**: Feature flag compilation verification (each backend compiles independently)

## Future Requirements

### Pagination & Querying

- **PAGE-01**: GSI-based cursor pagination for DynamoDB owner-scoped listing
- **PAGE-02**: Backend capability detection (CAS support, native TTL, pagination)

### Shared Conformance

- **CONF-01**: Macro-generated conformance test suite that every backend must pass
- **CONF-02**: Backend-agnostic test harness with pluggable backend instantiation

### Advanced Features

- **ADVN-01**: Compare-and-swap (CAS) as a first-class StorageBackend trait method
- **ADVN-02**: Backend capability negotiation in GenericTaskStore
- **ADVN-03**: ConnectionManager auto-reconnect for Redis backend

## Out of Scope

| Feature | Reason |
|---------|--------|
| DynamoDB Local / docker-based testing | Cloud-only DynamoDB in CI — no docker dependency |
| Redis Cluster support | Single-node sufficient for proving the trait |
| S3 backend | Not identified as a priority; DynamoDB + Redis validates the abstraction |
| Task status notifications | Polling-only for now (validated by v1.0) |
| Shared conformance test suite | Deferred to future — per-backend tests for this milestone |
| GSI pagination for DynamoDB | Deferred — core CRUD + TTL + conditional writes first |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| ABST-01 | Phase 9 | Pending |
| ABST-02 | Phase 9 | Pending |
| ABST-03 | Phase 9 | Pending |
| ABST-04 | Phase 9 | Pending |
| IMEM-01 | Phase 10 | Pending |
| IMEM-02 | Phase 10 | Pending |
| IMEM-03 | Phase 10 | Pending |
| DYNA-01 | Phase 11 | Pending |
| DYNA-02 | Phase 11 | Pending |
| DYNA-03 | Phase 11 | Pending |
| DYNA-04 | Phase 11 | Pending |
| DYNA-05 | Phase 11 | Pending |
| DYNA-06 | Phase 11 | Pending |
| RDIS-01 | Phase 12 | Pending |
| RDIS-02 | Phase 12 | Pending |
| RDIS-03 | Phase 12 | Pending |
| RDIS-04 | Phase 12 | Pending |
| RDIS-05 | Phase 12 | Pending |
| TEST-01 | Phase 10 | Pending |
| TEST-02 | Phase 11 | Pending |
| TEST-03 | Phase 12 | Pending |
| TEST-04 | Phase 13 | Pending |

**Coverage:**
- v1.2 requirements: 22 total
- Mapped to phases: 22
- Unmapped: 0

---
*Requirements defined: 2026-02-23*
*Last updated: 2026-02-23 after roadmap creation*
