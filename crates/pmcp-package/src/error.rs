//! Crate-wide error surface for `pmcp-package`.
//!
//! One flat, structured `PackageError` enum covers every failure mode the
//! crate's modules produce: digest/verify, allowlist enforcement
//!, reference parsing, slot-conflict detection, OCI layout I/O,
//! and (de)serialization. Variants carry concrete structured fields (not bare
//! `String`-wrapped messages), following the `PathError`
//! (`built-in/agents-api/crates/mcp-builtin-server-core/src/path_validator.rs`)
//! and `PolicyManagementError`
//! (`built-in/shared/mcp-server-common/src/code_mode/policy_management.rs`)
//! precedents.
//!
//! There is deliberately NO `SlotDeviation` variant: behavior-relevant
//! deviation detection returns `Option<Deviation>` — a value, not
//! an error — so an error variant for it would be dead code. `SlotConflict`
//! is the one slot-related error variant; it is returned by
//! `slot::aggregate()` when the same behavior-relevant slot (kind+name)
//! carries different tested values across components (a silent
//! discard would mask a behavioral change).
//!
//! `serde_json::Error` has a single type covering both serialize and
//! deserialize failures, so there is exactly one `#[from]` variant
//! (`Serialize`) for it — a second `#[from] serde_json::Error` variant would
//! be a duplicate `impl From` and fail to compile.
//!
//! No `reqwest::Error` variant exists — this crate makes no HTTP calls.

/// Result type alias for this crate.
pub type Result<T> = std::result::Result<T, PackageError>;

/// All ways a `pmcp-package` operation can fail.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    /// JSON (de)serialization failure. Covers both directions — serde_json
    /// has a single `Error` type for both, so there is exactly one variant.
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    /// Filesystem I/O failure (OCI layout read/write, blob read/write).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A recomputed digest did not match the digest declared in a manifest
    /// or descriptor (tamper detection).
    #[error("digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    /// A digest string is not well-formed (e.g. not `sha256:<64-hex>`).
    #[error("malformed digest: {reason}")]
    MalformedDigest { reason: String },

    /// A deploy descriptor requested a CloudFormation resource type not on
    /// the allowlist.
    #[error("allowlist violation: {resource}")]
    AllowlistViolation { resource: String },

    /// A `ComponentRef` string (semver range or pinned+digest form) failed
    /// to parse.
    #[error("invalid reference: {reason}")]
    InvalidReference { reason: String },

    /// The same behavior-relevant slot (kind+name) carried different tested
    /// values across components during aggregation.
    #[error("slot conflict on '{slot}': tested={tested}, proposed={proposed}")]
    SlotConflict {
        slot: String,
        tested: String,
        proposed: String,
    },

    /// An OCI Image Layout on disk was malformed or incomplete (missing
    /// `oci-layout`, missing `index.json`, missing referenced blob, etc.).
    #[error("OCI layout error: {reason}")]
    Layout { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_mismatch_display_contains_structured_fields() {
        let err = PackageError::DigestMismatch {
            expected: "sha256:aaaa".to_string(),
            actual: "sha256:bbbb".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("sha256:aaaa"), "message was: {msg}");
        assert!(msg.contains("sha256:bbbb"), "message was: {msg}");
    }

    #[test]
    fn slot_conflict_display_contains_structured_fields() {
        let err = PackageError::SlotConflict {
            slot: "llm-provider".to_string(),
            tested: "anthropic".to_string(),
            proposed: "openai".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("llm-provider"), "message was: {msg}");
        assert!(msg.contains("anthropic"), "message was: {msg}");
        assert!(msg.contains("openai"), "message was: {msg}");
    }

    #[test]
    fn serialize_variant_wraps_serde_json_error_via_from() {
        let json_err = serde_json::from_str::<serde_json::Value>("{not valid json")
            .expect_err("must fail to parse");
        let err: PackageError = json_err.into();
        assert!(matches!(err, PackageError::Serialize(_)));
    }
}
