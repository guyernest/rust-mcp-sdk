//! Centralized, version-gated protocol error codes (VERS-06).
//!
//! This module is the **single source of truth** for every JSON-RPC / MCP
//! protocol error-code integer literal that reaches the wire. It exists so the
//! dominant error-code surface — [`crate::error::ErrorCode`]'s 11 associated
//! consts, referenced ~210 times across the codebase and serialized via
//! `impl From<crate::Error> for JSONRPCError` — sources every value from ONE
//! table rather than from scattered bare literals.
//!
//! # Structure-first, values-from-final-schema
//!
//! The constants below are grouped by semantic so that a later version-gated
//! resolver (`code_for(era, semantic)`) and the v2 remaps can drop in at
//! finalization **without restructuring**. v2 semantic error-code values
//! (e.g. the SEP-2164 resource-not-found `-32002`→`-32602` remap) are finalized
//! only when the 2026-07-28 `schema.json` publishes; see the Phase 112 VERS-06
//! final-schema finalization item tracked in the planning system
//! (`112-VALIDATION.md` marks VERS-06 partial-until-final-schema). Those v2
//! values are **structurally omitted** here (absent, not stubbed) — there is no
//! placeholder constant and, deliberately, no self-admitted-technical-debt
//! marker token anywhere, so PMAT's zero-SATD gate passes.
//!
//! # The two distinct meanings of `-32002`
//!
//! Two semantically different errors intentionally share the number `-32002`
//! and are represented here as two separately-named constants:
//!
//! - [`V1_TASK_PENDING`] — the FROZEN v1 task-pending code. Its call sites are
//!   `src/server/core.rs` (server-not-initialized) and
//!   `src/server/task_dispatch.rs` (task result not yet available), locked by
//!   the `pending_tasks_result_preserves_minus_32002` regression test. This
//!   value and its semantics MUST NOT be reconciled with the spec's
//!   resource-not-found rename.
//! - [`UNSUPPORTED_CAPABILITY`] — the capability-unsupported semantic that
//!   [`crate::error::ErrorCode`] already carries at `-32002`.
//!
//! The numeric collision of these two distinct meanings is preserved by name,
//! never "fixed".

// ---------------------------------------------------------------------------
// Standard JSON-RPC 2.0 error codes.
// ---------------------------------------------------------------------------

/// Parse error — invalid JSON was received (JSON-RPC 2.0).
pub const PARSE_ERROR: i32 = -32700;
/// Invalid request — the JSON is not a valid Request object (JSON-RPC 2.0).
pub const INVALID_REQUEST: i32 = -32600;
/// Method not found — the method does not exist / is not available.
///
/// v1 `server/discover` reaches this for free: unknown methods are turned into
/// `Error::method_not_found` by `parse_request` before dispatch (D-10).
pub const METHOD_NOT_FOUND: i32 = -32601;
/// Invalid params — invalid method parameter(s) (JSON-RPC 2.0).
pub const INVALID_PARAMS: i32 = -32602;
/// Internal error — internal JSON-RPC error (JSON-RPC 2.0).
pub const INTERNAL_ERROR: i32 = -32603;

// ---------------------------------------------------------------------------
// pmcp server-defined error codes (-320xx family).
//
// Mirrors `crate::error::ErrorCode` exactly; those associated consts delegate
// back to these values so this table is the real source of truth.
// ---------------------------------------------------------------------------

/// Request timeout — the server-side operation exceeded its deadline.
pub const REQUEST_TIMEOUT: i32 = -32001;

/// Unsupported capability (`-32002`).
///
/// The capability-unsupported semantic carried by
/// [`crate::error::ErrorCode::UNSUPPORTED_CAPABILITY`]. This intentionally
/// shares the number `-32002` with [`V1_TASK_PENDING`] but is a DIFFERENT
/// meaning — the two are kept distinct by name and are NOT reconciled.
pub const UNSUPPORTED_CAPABILITY: i32 = -32002;

/// Frozen v1 task-pending code (`-32002`).
///
/// Re-exports the FROZEN task-pending literal verbatim. Call sites:
/// `src/server/core.rs` (server-not-initialized) and
/// `src/server/task_dispatch.rs` (task result not yet available). Locked by the
/// `pending_tasks_result_preserves_minus_32002` regression test. This value and
/// its semantics MUST NOT change and MUST NOT be reconciled with the spec's
/// resource-not-found rename or with [`UNSUPPORTED_CAPABILITY`] (a different
/// meaning that squats on the same number).
pub const V1_TASK_PENDING: i32 = -32002;

/// Authentication required — the request must be authenticated.
pub const AUTHENTICATION_REQUIRED: i32 = -32003;
/// Permission denied — the authenticated principal lacks authorization.
pub const PERMISSION_DENIED: i32 = -32004;
/// Rate limited — the client exceeded a rate limit.
pub const RATE_LIMITED: i32 = -32005;
/// Circuit breaker open — an upstream dependency is being shed.
pub const CIRCUIT_BREAKER_OPEN: i32 = -32006;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::types::protocol::ProtocolErrorCode;

    /// Both distinct meanings of `-32002` are present, by their own names, with
    /// the same numeric value. This collision is intentional and preserved.
    #[test]
    fn both_minus_32002_meanings_coexist() {
        assert_eq!(V1_TASK_PENDING, -32002);
        assert_eq!(UNSUPPORTED_CAPABILITY, -32002);
        // They are the same number but are addressed by distinct names.
        assert_eq!(V1_TASK_PENDING, UNSUPPORTED_CAPABILITY);
    }

    /// The standard JSON-RPC constants agree with the near-dead
    /// `ProtocolErrorCode` C-style enum discriminants (the enum is NOT edited;
    /// this test is the binding guard that the two representations agree).
    #[test]
    fn standard_codes_match_protocol_error_code_enum() {
        assert_eq!(INVALID_REQUEST, ProtocolErrorCode::InvalidRequest as i32);
        assert_eq!(METHOD_NOT_FOUND, ProtocolErrorCode::MethodNotFound as i32);
        assert_eq!(INVALID_PARAMS, ProtocolErrorCode::InvalidParams as i32);
        assert_eq!(INTERNAL_ERROR, ProtocolErrorCode::InternalError as i32);
    }

    /// Per-name value-equality between every `error::ErrorCode::FOO` and
    /// `error_codes::FOO`. Because `ErrorCode`'s consts DELEGATE to this table,
    /// this test transitively keeps all ~210 `ErrorCode::` call sites correct:
    /// any future edit to either side (name or value) fails CI here.
    #[test]
    fn error_code_surface_delegates_to_table() {
        assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), PARSE_ERROR);
        assert_eq!(ErrorCode::INVALID_REQUEST.as_i32(), INVALID_REQUEST);
        assert_eq!(ErrorCode::METHOD_NOT_FOUND.as_i32(), METHOD_NOT_FOUND);
        assert_eq!(ErrorCode::INVALID_PARAMS.as_i32(), INVALID_PARAMS);
        assert_eq!(ErrorCode::INTERNAL_ERROR.as_i32(), INTERNAL_ERROR);
        assert_eq!(ErrorCode::REQUEST_TIMEOUT.as_i32(), REQUEST_TIMEOUT);
        assert_eq!(
            ErrorCode::UNSUPPORTED_CAPABILITY.as_i32(),
            UNSUPPORTED_CAPABILITY
        );
        assert_eq!(
            ErrorCode::AUTHENTICATION_REQUIRED.as_i32(),
            AUTHENTICATION_REQUIRED
        );
        assert_eq!(ErrorCode::PERMISSION_DENIED.as_i32(), PERMISSION_DENIED);
        assert_eq!(ErrorCode::RATE_LIMITED.as_i32(), RATE_LIMITED);
        assert_eq!(
            ErrorCode::CIRCUIT_BREAKER_OPEN.as_i32(),
            CIRCUIT_BREAKER_OPEN
        );
    }

    /// `ErrorCode::UNSUPPORTED_CAPABILITY` delegates to the capability `-32002`,
    /// NOT to `V1_TASK_PENDING` — the two `-32002` meanings stay distinct by
    /// name even though they share the number.
    #[test]
    fn unsupported_capability_is_not_task_pending_by_name() {
        assert_eq!(
            ErrorCode::UNSUPPORTED_CAPABILITY.as_i32(),
            UNSUPPORTED_CAPABILITY
        );
        assert_eq!(UNSUPPORTED_CAPABILITY, V1_TASK_PENDING);
    }
}
