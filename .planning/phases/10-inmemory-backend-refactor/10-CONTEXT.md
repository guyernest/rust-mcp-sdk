# Phase 10: InMemory Backend Refactor - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace the existing `InMemoryTaskStore` (which directly implements `TaskStore` with all domain logic inline in `store/memory.rs`) with `GenericTaskStore<InMemoryBackend>`. The new `InMemoryBackend` implements `StorageBackend` as a dumb KV store using DashMap. All 200+ existing tests must maintain their assertions and coverage. Per-backend unit tests validate InMemoryBackend's StorageBackend contract independently.

</domain>

<decisions>
## Implementation Decisions

### Public API shape
- Claude's discretion on type alias vs thin wrapper -- pick whichever best fits the codebase patterns and downstream usage
- Claude's discretion on whether `InMemoryBackend` is public or `pub(crate)` -- pick based on SDK architecture needs
- Constructor changes to accept a backend argument: `InMemoryTaskStore::new(backend)` following GenericTaskStore's pattern (breaking change accepted)
- Builder methods (`with_config`, `with_security`, `with_poll_interval`) must remain available on the resulting type

### Behavioral parity
- Accept stricter validation from GenericTaskStore (variable depth bomb protection, string length limits) -- these are improvements, not regressions
- Accept CAS (put_if_version) semantics for mutations -- ConcurrentModification errors surfaced explicitly instead of silent DashMap lock serialization
- Accept JSON serialization overhead on every operation -- correctness and backend uniformity matter more than raw performance for in-memory dev/test use
- Cleanup_expired: InMemoryBackend deserializes each record to check TTL (consistent with TestBackend pattern in generic.rs, simple approach)

### Poll interval default
- Claude's discretion on whether to keep 5000ms (current InMemoryTaskStore) or switch to 500ms (GenericTaskStore default)

### Test migration boundary
- "Tests pass unchanged" means: test assertions and coverage stay the same, but test setup code (e.g., forcing expiry by mutating internals) can be adapted to the new internal structure
- Claude's discretion on test file organization (keep in memory.rs, split, or reorganize)
- Replace the TestBackend in generic.rs with the real InMemoryBackend after the refactor -- single source of truth, no duplicated test backend
- Per-backend unit tests (TEST-01): full contract coverage of all 6 StorageBackend methods (get, put, put_if_version, delete, list_by_prefix, cleanup_expired) with happy paths and error cases

### Claude's Discretion
- Type alias vs thin wrapper decision
- InMemoryBackend visibility (public vs pub(crate))
- Poll interval default (5000ms vs 500ms)
- Test file organization strategy
- Any internal implementation details for InMemoryBackend (data structures, locking strategy within DashMap)

</decisions>

<specifics>
## Specific Ideas

- The TestBackend in `generic.rs` is essentially a prototype of what InMemoryBackend will be -- use it as a starting reference
- InMemoryBackend stores `DashMap<String, (Vec<u8>, u64)>` (bytes + version), consistent with the TestBackend pattern
- After replacing TestBackend with InMemoryBackend, the CasConflictBackend test wrapper in generic.rs should remain (it tests GenericTaskStore's CAS error handling, not a specific backend)

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 10-inmemory-backend-refactor*
*Context gathered: 2026-02-23*
