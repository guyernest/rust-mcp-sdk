//! Protocol version constants and negotiation logic.

/// Latest protocol version supported by this SDK.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";

/// Default protocol version used for negotiation fallback.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

/// All protocol versions supported by this SDK.
///
/// Includes the 2024-11-05 base version for backward compatibility with
/// clients that haven't upgraded yet (Claude Code, Cursor, etc.).
/// The 2025 versions add features but the base JSON-RPC request/response
/// format is the same — accepting 2024-11-05 is safe.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    DEFAULT_PROTOCOL_VERSION,
    "2024-11-05",
];

/// The MCP 2026-07-28 (v2) protocol version, opt-in only.
///
/// This constant is **deliberately not** a member of
/// [`SUPPORTED_PROTOCOL_VERSIONS`] and is **never** returned by
/// [`negotiate_protocol_version`]. The v2 era is reached only through the
/// per-server opt-in accept-list (Phase 112 Plan 04), never through legacy
/// version negotiation. Keeping [`LATEST_PROTOCOL_VERSION`] pinned to
/// `2025-11-25` is the single most important backward-compat guard in the
/// v2.5 milestone: `negotiate_protocol_version` returns `LATEST` for any
/// unknown version, so flipping `LATEST` would silently upgrade legacy
/// clients to v2 semantics.
pub const PROTOCOL_VERSION_2026_07_28: &str = "2026-07-28";

/// Protocol era: the coarse behavioral generation a negotiated version belongs to.
///
/// The whole v2.5 milestone era-gates off this classifier. `V1` covers every
/// `2024`/`2025` protocol version (the compatibility layer); `V2` is the
/// `2026-07-28` stateless/Tasks/MCP-Apps generation. Unknown or unrecognized
/// versions conservatively classify as [`Era::V1`] so a malformed or
/// forward-dated version string can never accidentally reach v2 behavior.
///
/// # Why this derives `Hash`
///
/// The emit-time `outputSchema` validator cache in
/// `crate::server::output_validation` is keyed on `(Era, schema text)`, not on
/// the schema text alone: under Phase 115 D-01 the SAME schema document
/// compiles to two DIFFERENT validators depending on the era (v1 auto-detects
/// the declared `$schema` dialect; v2 pins Draft 2020-12). Keying on text alone
/// would be first-writer-wins for the process lifetime — whichever era compiled
/// a given schema first would serve its validator to the other. `Hash` on this
/// fieldless enum is what makes the tuple key possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Era {
    /// The 2024/2025 protocol generation (compatibility layer, current default).
    V1,
    /// The 2026-07-28 protocol generation (opt-in, stateless-first).
    V2,
}

/// Classify a negotiated protocol version string into its behavioral [`Era`].
///
/// Returns [`Era::V2`] **only** for [`PROTOCOL_VERSION_2026_07_28`]
/// (`"2026-07-28"`). Every other string — including all supported v1 versions
/// and any unknown/malformed input — classifies as [`Era::V1`]. This
/// conservative unknown-to-`V1` fallback guarantees that only an exact,
/// deliberate `2026-07-28` negotiation reaches v2 behavior.
///
/// ```
/// use pmcp::types::protocol::{protocol_era, Era, PROTOCOL_VERSION_2026_07_28};
///
/// assert_eq!(protocol_era(PROTOCOL_VERSION_2026_07_28), Era::V2);
/// assert_eq!(protocol_era("2025-11-25"), Era::V1);
/// assert_eq!(protocol_era("who-knows"), Era::V1);
/// ```
pub fn protocol_era(version: &str) -> Era {
    if version == PROTOCOL_VERSION_2026_07_28 {
        Era::V2
    } else {
        Era::V1
    }
}

/// Negotiate the protocol version for an MCP session.
///
/// If the client's requested version is in [`SUPPORTED_PROTOCOL_VERSIONS`],
/// echo it back (highest common version). Otherwise return
/// [`LATEST_PROTOCOL_VERSION`] -- the caller should treat this as
/// "unsupported version" and may return a JSON-RPC error with the
/// supported versions list.
pub fn negotiate_protocol_version(client_version: &str) -> &str {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&client_version) {
        client_version
    } else {
        LATEST_PROTOCOL_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_version_is_2025_11_25() {
        assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25");
    }

    #[test]
    fn supports_four_versions_including_2024() {
        assert_eq!(SUPPORTED_PROTOCOL_VERSIONS.len(), 4);
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&"2025-11-25"));
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&"2025-06-18"));
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&"2025-03-26"));
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&"2024-11-05"));
    }

    #[test]
    fn rejects_unknown_2024_versions() {
        // 2024-10-07 was never a real MCP version
        assert!(!SUPPORTED_PROTOCOL_VERSIONS.contains(&"2024-10-07"));
    }

    #[test]
    fn negotiate_supported_version_echoes_back() {
        assert_eq!(negotiate_protocol_version("2025-11-25"), "2025-11-25");
        assert_eq!(negotiate_protocol_version("2025-06-18"), "2025-06-18");
        assert_eq!(negotiate_protocol_version("2025-03-26"), "2025-03-26");
        assert_eq!(negotiate_protocol_version("2024-11-05"), "2024-11-05");
    }

    #[test]
    fn negotiate_unsupported_returns_latest() {
        assert_eq!(negotiate_protocol_version("2024-10-07"), "2025-11-25");
        assert_eq!(negotiate_protocol_version("unknown"), "2025-11-25");
    }

    #[test]
    fn v2_constant_is_not_in_legacy_supported_set() {
        // 2026-07-28 (v2) is opt-in only — it must NEVER be a member of the
        // legacy-negotiation set. This guards the legacy-negotiation set (the
        // versions reachable via `negotiate_protocol_version`), NOT "every
        // version the crate can understand". v2 is reached only via the
        // opt-in accept-list (Plan 04), never legacy negotiation (Pitfall 1).
        assert_eq!(PROTOCOL_VERSION_2026_07_28, "2026-07-28");
        assert!(!SUPPORTED_PROTOCOL_VERSIONS.contains(&PROTOCOL_VERSION_2026_07_28));
    }

    #[test]
    fn negotiate_never_upgrades_legacy_client_to_v2() {
        // A legacy client asking for an unknown version must fall back to
        // LATEST (v1), never to the v2 constant.
        assert_ne!(
            negotiate_protocol_version("2026-07-28"),
            PROTOCOL_VERSION_2026_07_28
        );
        assert_eq!(
            negotiate_protocol_version("2026-07-28"),
            LATEST_PROTOCOL_VERSION
        );
    }

    #[test]
    fn protocol_era_classifies_2026_07_28_as_v2() {
        assert_eq!(protocol_era("2026-07-28"), Era::V2);
        assert_eq!(protocol_era(PROTOCOL_VERSION_2026_07_28), Era::V2);
    }

    #[test]
    fn protocol_era_classifies_known_v1_versions_as_v1() {
        assert_eq!(protocol_era("2025-11-25"), Era::V1);
        assert_eq!(protocol_era("2025-06-18"), Era::V1);
        assert_eq!(protocol_era("2025-03-26"), Era::V1);
        assert_eq!(protocol_era("2024-11-05"), Era::V1);
    }

    #[test]
    fn protocol_era_classifies_unknown_as_v1() {
        // Conservative unknown -> V1 fallback: malformed/forward-dated strings
        // must never accidentally reach v2 behavior.
        assert_eq!(protocol_era("unknown"), Era::V1);
        assert_eq!(protocol_era(""), Era::V1);
        assert_eq!(protocol_era("2027-01-01"), Era::V1);
        assert_eq!(protocol_era("2026-07-29"), Era::V1);
    }
}
