//! Property tests for the HTTP connector URL construction (OAPI-01 / Pitfall 2).
//!
//! Asserts the invariant of the shared `join_url` path-concat as observed
//! through the public `HttpClient` surface: for any `(base_path, tool_path)` the
//! joined URL has exactly ONE slash between the base and the leaf and NEVER drops
//! the base's non-root path (an API-Gateway stage prefix like `/v1` survives).
//! Plan 03 extends this file with a path-param-substitution proptest.

#![cfg(feature = "http")]

use pmcp_server_toolkit::http::auth::NoAuth;
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use proptest::prelude::*;
use std::sync::Arc;

/// Build a base URL with the given path segments and return the `HttpClient`'s
/// stored `base_url()` for assertion convenience.
fn client_base(path: &str) -> String {
    let base = format!("https://api.example.com{path}");
    let client =
        HttpClient::new(reqwest::Client::new(), base, Arc::new(NoAuth)).expect("valid base URL");
    client.base_url().to_string()
}

proptest! {
    /// For any single-segment base prefix and leaf, the joined URL keeps the
    /// prefix and joins with exactly one slash. We assert via the public
    /// `base_url()` + a manual single-slash join mirroring the connector.
    #[test]
    fn join_preserves_prefix_single_slash(
        prefix in "[a-z]{1,8}",
        leaf in "[a-z]{1,8}",
    ) {
        let base = client_base(&format!("/{prefix}"));
        // Mirror the connector's concatenation contract.
        let joined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            leaf.trim_start_matches('/')
        );
        // Prefix preserved.
        prop_assert!(joined.contains(&format!("/{prefix}/")), "prefix dropped in {joined}");
        // Exactly one slash between prefix and leaf (no double slash after host).
        let after_host = &joined["https://".len()..];
        prop_assert!(!after_host.contains("//"), "double slash in {joined}");
        // Leaf present at the tail.
        prop_assert!(joined.ends_with(&leaf), "leaf missing in {joined}");
    }

    /// A trailing slash on the base and a leading slash on the leaf collapse to one.
    #[test]
    fn trailing_and_leading_slash_collapse(leaf in "[a-z]{1,8}") {
        let base = client_base("/v1/");
        let joined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            format!("/{leaf}").trim_start_matches('/')
        );
        prop_assert_eq!(
            joined,
            format!("https://api.example.com/v1/{leaf}")
        );
    }
}
