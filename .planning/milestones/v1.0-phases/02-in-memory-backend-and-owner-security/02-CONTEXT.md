# Phase 2: In-Memory Backend and Owner Security - Context

**Gathered:** 2026-02-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement the InMemoryTaskStore backend, owner isolation security, TaskSecurityConfig limits, and TaskContext ergonomic helpers. This is the first phase that introduces server-side storage of user information. The MCP server operates statelessly with OAuth, and tasks are the first feature that persists per-user data — cross-user data leak prevention is a critical security concern.

</domain>

<decisions>
## Implementation Decisions

### Owner isolation model
- Owner ID derived from OAuth token identity (authentication-based, not session-based)
- Owner ID used as part of the store key itself, making cross-user access structurally impossible rather than relying on access control checks
- On owner mismatch: return NotFound (never reveal that a task exists but belongs to someone else)
- No admin bypass — strict isolation with no exceptions
- No cross-owner listing — list() always scopes to a single owner
- Variable size limits enforced at the store level (defense in depth)

### Security limits behavior
- Max tasks per owner: hard reject with ResourceExhausted error (no auto-eviction)
- TTL enforcement: reject with error if client requests TTL above configured max (no silent clamping)
- Anonymous access: supported for local single-user servers without OAuth — use a well-known default owner ID (e.g., "local") when no auth is configured. All tasks belong to this default owner, keeping code paths consistent.

### TaskContext ergonomics
- Typed variable accessors: ctx.get_string("key"), ctx.get_i64("count") with type conversion (not just raw JSON values)
- Status transition methods and result handling: Claude's discretion on whether to accept result inline (ctx.complete(value)) or separately, based on how complete_with_result works at the store level
- Store reference ownership: Claude's discretion on whether TaskContext owns Arc<dyn TaskStore> or takes it as parameter, based on Phase 3 handler integration needs
- Scope: tool handler focused — designed for use inside tool handlers, scoped to a single task lifecycle

### Concurrency and cleanup
- Expired task cleanup: on-demand only — cleanup_expired() called explicitly, no background task inside the store
- Expiry read behavior: expired-but-not-yet-cleaned-up tasks are still readable with an expiration flag (allows client to retry with longer TTL or different approach). Once cleanup removes them, returns NotFound.
- Locking strategy: Claude's discretion (single RwLock or DashMap) based on expected concurrency patterns and existing codebase conventions

### Claude's Discretion
- Locking strategy (RwLock vs DashMap) — pick based on codebase conventions
- TaskContext store ownership pattern — pick based on Phase 3 handler integration needs
- Status transition + result API shape — pick based on store's complete_with_result semantics

</decisions>

<specifics>
## Specific Ideas

- "We are opting for stateless operation of the MCP server with OAuth which limits the risk of data leaks across users. Tasks is now adding some server side storage of user's information, and we need to be careful that we don't start leaking information between users."
- "We can use the user ID from the OAuth token as part of the store key. It can prevent from accidental leaks."
- Owner ID as part of store key is a structural guarantee — not an access check that could be bypassed
- Local MCP servers (single user, no OAuth) should work with a default owner ID for consistency

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-in-memory-backend-and-owner-security*
*Context gathered: 2026-02-21*
