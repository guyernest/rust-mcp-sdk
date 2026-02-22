# Phase 1: Foundation Types and Store Contract - Context

**Gathered:** 2026-02-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Spec-compliant protocol types, TaskStore trait, state machine, error types, and serialization tests. This phase creates the `pmcp-tasks` crate as a workspace member with all foundational types and the storage abstraction. No backend implementations, no server integration, no middleware — those are later phases.

</domain>

<decisions>
## Implementation Decisions

### Variable Visibility
- Variables always included in `_meta` on every tasks/get response — client always sees current state
- Bidirectional read/write: both client and server can read and write variables
- Ownership via prefix convention: `server.*` for server vars, `client.*` for client vars. Either side can read all, writes follow convention
- Variables placed at top level of `_meta` (not nested under a PMCP-specific key)

### Extension Strategy
- Wire types and domain types are separate. Wire types map 1:1 to MCP spec JSON. Domain types include PMCP extensions (variables, owner). Convert between them at the boundary.
- TaskRecord (store level) has public variables/owner_id fields — store implementors need full access. TaskContext is the primary ergonomic API for tool handlers.
- Capabilities via `experimental.tasks` only for now. Dedicated `ServerCapabilities.tasks` field deferred to spec stabilization.

### Error Experience
- Rich context in errors: include task_id, current_status, attempted_action for debugging
- Public enum for TaskError — idiomatic Rust, match-friendly. Variants: InvalidTransition { from, to, task_id }, NotFound { task_id }, Expired { task_id, expired_at }, NotReady { task_id, current_status }, OwnerMismatch, ResourceExhausted, VariableSizeExceeded, StoreError, etc.
- Distinct Expired error (not folded into NotFound) — caller knows why the task is gone
- Errors include suggested_action field where possible (e.g., "retry after TTL extension", "poll again in 5s")
- TTL is adjustable after creation — both server (via TaskContext) and client (via task params) can extend TTL during execution
- Design priority: Rust best practices (idiomatic enums, exhaustive matching, clear ownership semantics) throughout

### Crate Public API
- Crate name: `pmcp-tasks` — direct dependency, independent versioning
- No re-export via pmcp feature flag (direct crate dependency only)
- Module structure matches existing PMCP SDK crate organization — Claude to examine and follow the pattern
- Export pattern matches main pmcp crate — consistency with existing workspace conventions

### Claude's Discretion
- Unknown JSON field handling strategy (preserve vs ignore) for spec forward-compatibility
- Whether to prepare a `stable-tasks` feature flag for future dedicated capability field
- Exact module layout within the crate (following existing pmcp patterns)
- Compression/optimization of variable serialization

</decisions>

<specifics>
## Specific Ideas

- "Tasks are coordination primitives for long-running activities between client and server, with external dependencies like LLM speed or data system processing. The system needs adjustability (TTL, steps, fields) with clear error messages and information sharing."
- Rust best practices are a design priority — public enums, exhaustive matching, clear ownership semantics, idiomatic patterns throughout
- Variable ownership via prefix convention allows both sides to maintain their context while sharing a unified view

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-types-and-store-contract*
*Context gathered: 2026-02-21*
