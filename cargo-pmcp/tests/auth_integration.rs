//! End-to-end tests for the `cargo pmcp auth` subcommand group + cache fallback.
//!
//! Covers:
//! - TokenCacheV1 round-trip via `cache::*` helpers
//! - URL normalization edge cases (T-74-D)
//! - logout no-args error (D-09)
//! - auth token stdout discipline (D-11)
//! - precedence: explicit `--api-key` wins over cached OAuth token (D-13, LOW-10)

// Review MED-5 — use the narrow `test_support` seam, not the full `commands` tree.
use cargo_pmcp::test_support::cache::{
    default_multi_cache_path, is_near_expiry, normalize_cache_key, TokenCacheEntry, TokenCacheV1,
    REFRESH_WINDOW_SECS,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn normalize_covers_t74d_edge_cases() {
    // IDN / mixed case / trailing slash / default port / custom port
    assert_eq!(
        normalize_cache_key("HTTPS://API.Example.Com/").unwrap(),
        "https://api.example.com"
    );
    assert_eq!(
        normalize_cache_key("https://api.example.com:443").unwrap(),
        "https://api.example.com"
    );
    assert_eq!(
        normalize_cache_key("http://api.example.com:8080/x/y/z").unwrap(),
        "http://api.example.com:8080"
    );
}

#[tokio::test]
async fn cache_roundtrip_via_write_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".pmcp").join("oauth-cache.json");
    let mut c = TokenCacheV1::empty();
    c.entries.insert(
        "https://mockito.example".to_string(),
        TokenCacheEntry {
            access_token: "integration-test-token".into(),
            refresh_token: Some("rt".into()),
            expires_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
            scopes: vec!["openid".into()],
            issuer: Some("https://issuer.example".into()),
            client_id: "cid".into(),
        },
    );
    c.write_atomic(&path).unwrap();

    let back = TokenCacheV1::read(&path).unwrap();
    assert_eq!(
        back.entries
            .get("https://mockito.example")
            .unwrap()
            .access_token,
        "integration-test-token"
    );
}

#[test]
fn is_near_expiry_window_is_60s() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let entry = TokenCacheEntry {
        access_token: "at".into(),
        refresh_token: None,
        expires_at: Some(now + 30),
        scopes: vec![],
        issuer: None,
        client_id: "c".into(),
    };
    assert!(is_near_expiry(&entry, REFRESH_WINDOW_SECS));
    let far = TokenCacheEntry {
        expires_at: Some(now + 3600),
        ..entry
    };
    assert!(!is_near_expiry(&far, REFRESH_WINDOW_SECS));
}

#[tokio::test]
async fn default_multi_cache_path_ends_in_oauth_cache_json() {
    let p = default_multi_cache_path();
    let s = p.to_string_lossy().to_string();
    assert!(s.ends_with(".pmcp/oauth-cache.json") || s.ends_with(".pmcp\\oauth-cache.json"));
}

#[cfg(unix)]
#[tokio::test]
async fn logout_no_args_errors_via_cli() {
    // Warning #9 fix — use CARGO_BIN_EXE_cargo-pmcp (pre-built binary).
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["auth", "logout"])
        .env("HOME", temp.path())
        .output()
        .expect("run cargo-pmcp binary");
    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("specify a server URL or --all"),
        "expected D-09 copy in stderr, got: {stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn auth_token_prints_only_token_to_stdout() {
    // Prime a cache in a temp HOME, then invoke `cargo pmcp auth token <url>`
    // and verify stdout is EXACTLY the token + newline (no banner/status).
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let pmcp_dir = temp.path().join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir).unwrap();
    let cache_path = pmcp_dir.join("oauth-cache.json");

    let mut c = TokenCacheV1::empty();
    c.entries.insert(
        "https://mockito.example".into(),
        TokenCacheEntry {
            access_token: "SECRET-TOKEN-VALUE-42".into(),
            refresh_token: None,
            expires_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
            scopes: vec!["openid".into()],
            issuer: Some("https://issuer.example".into()),
            client_id: "cid".into(),
        },
    );
    c.write_atomic(&cache_path).unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["auth", "token", "https://mockito.example"])
        .env("HOME", temp.path())
        .output()
        .expect("run cargo-pmcp binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim_end(),
        "SECRET-TOKEN-VALUE-42",
        "D-11: raw token only"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn api_key_flag_overrides_cached_oauth_token() {
    // Review LOW-10 — explicit precedence test for D-13 (flag > env > cache).
    // Seeds a fake OAuth entry in a temp cache, then invokes a server-connecting
    // command with `--api-key <FORCED>` against a mockito server that only
    // accepts the forced key. Succeeds when the outgoing Authorization header
    // contains "Bearer forced-key-123" (not the cached OAuth bearer).
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let pmcp_dir = temp.path().join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir).unwrap();
    let cache_path = pmcp_dir.join("oauth-cache.json");

    // Spin up a mockito server that asserts the inbound Authorization header.
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    let mock = server
        .mock("POST", mockito::Matcher::Any)
        .match_header("authorization", "Bearer forced-key-123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
        .expect_at_least(1)
        .create_async()
        .await;

    // Seed the cache with a DIFFERENT (cached OAuth) token for the same URL.
    // If the cache were consulted (wrongly) it would send a different header
    // and mockito would return the default 501, failing the test.
    let mut c = TokenCacheV1::empty();
    c.entries.insert(
        normalize_cache_key(&base_url).unwrap(),
        TokenCacheEntry {
            access_token: "CACHED-OAUTH-SHOULD-NOT-BE-USED".into(),
            refresh_token: None,
            expires_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
            scopes: vec!["openid".into()],
            issuer: Some("https://issuer.example".into()),
            client_id: "cid".into(),
        },
    );
    c.write_atomic(&cache_path).unwrap();

    // Invoke `cargo pmcp test conformance <mockito_url> --api-key forced-key-123`.
    // conformance is the minimal command that consumes `AuthFlags::resolve()`
    // and flows through resolve_auth_middleware.
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args([
            "test",
            "conformance",
            &base_url,
            "--api-key",
            "forced-key-123",
        ])
        .env("HOME", temp.path())
        .output()
        .expect("run cargo-pmcp binary");

    // The test conformance command may exit non-zero on protocol assertion
    // failures from the dummy responses — we only care that the outgoing
    // header matched our mock. Assert via the mock's hit count.
    let _ = output; // status intentionally ignored
    mock.assert_async().await;
}
