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
}
