# Phase 9: Storage Abstraction Layer - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the StorageBackend KV trait that all backends implement, and GenericTaskStore that holds all domain logic (state machine, owner isolation, variable merge, TTL enforcement). Redesign the TaskStore trait from scratch. No backends are implemented in this phase — only the abstraction and the contract.

</domain>

<decisions>
## Implementation Decisions

### Cross-Cutting: Security Posture
- TaskStorage is the first persistent attack surface in the PMCP SDK. MCP servers were previously stateless — this is a new threat vector.
- All decisions must account for: data leakage across users, storage explosion/corruption, malicious input, and injection via serialized data.
- Owner binding must be enforced structurally (not just filter-based). NotFound on mismatch — never reveal that a task exists for another owner.
- The existing v1.0 security model (owner isolation, configurable limits, anonymous access control) must be preserved and reinforced at the backend level.
- Size limits serve as design guidance: tasks store lightweight coordination state (status, variables, metadata), not bulk data. The limit communicates intent to developers.

### Trait Method Design
- StorageBackend exposes ~6 methods: get, put, put-if-version (CAS), delete, list-by-prefix, cleanup-expired
- No dedicated count-by-prefix — list + .len() is sufficient. Backends can optimize internally if needed.
- CAS version mechanism: Claude's discretion — pick monotonic integer or content hash based on what works cleanest across DynamoDB and Redis
- Trait methods work with TaskRecord (domain-aware), not raw bytes. Purpose-built for task storage.
- Key structure (composite string vs separate fields): Claude's discretion — pick based on how DynamoDB and Redis handle keys differently

### Serialization Strategy
- Canonical serialization format: JSON — human-readable, debuggable via DynamoDB console / Redis CLI, better for security auditing
- Storage shape: single JSON blob per record — backend stores/retrieves bytes, all field access in GenericTaskStore after deserialization
- Variable values: size limit enforcement PLUS schema validation (no nested depth bombs, no extremely long strings within the size limit). No content sanitization.
- Universal size limit enforced in GenericTaskStore — not backend-specific. Configurable via StoreConfig. This is a design signal, not just a technical constraint.

### Error Model
- Domain-aware errors: TaskNotFound, ConcurrentModification, StorageFull — not generic NotFound/IoError
- ConcurrentModification is a specific variant with expected_version and actual_version fields — callers can explicitly handle contention
- Backend errors include underlying cause via std::error::Error::source() — developers can inspect for debugging
- No auto-retry on transient failures — surface errors immediately. Callers decide retry policy. No hidden latency.

### TaskStore Redesign
- Clean break from the current 11-method trait — redesign from scratch based on the new architecture
- Only keep what makes sense for GenericTaskStore's public interface
- Since TaskStore is unpublished, no backward compatibility concern

### Claude's Discretion
- CAS version mechanism (monotonic integer vs content hash)
- Key structure (composite string vs separate fields)
- Construction pattern (builder vs constructor+config) — align with existing SDK patterns
- Public API shape (TaskStore trait vs concrete GenericTaskStore methods) — align with TaskRouter pattern
- StoreConfig ownership (GenericTaskStore-only vs both layers) — pragmatic choice with security in mind

</decisions>

<specifics>
## Specific Ideas

- "The P in PMCP is pragmatic — keep the SDK usable and guide developers on best practices"
- Size limits are design signals — they communicate what should and shouldn't be stored on tasks
- Follow existing SDK patterns: structs for inputs, validation of received values, builder pattern for construction
- The security model from v1.0 (19 security tests, owner isolation, configurable limits) is the baseline — this phase must preserve and reinforce it

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 09-storage-abstraction-layer*
*Context gathered: 2026-02-23*
