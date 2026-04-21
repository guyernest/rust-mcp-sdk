//! Multi-server OAuth token cache for `cargo pmcp auth`.
//!
//! Schema version 1: `{ schema_version: 1, entries: { "<normalized_url>": Entry } }`.
//!
//! # Concurrency
//! Writes are atomic per file via `tempfile::NamedTempFile::persist`; concurrent
//! `auth login` from two terminals is last-writer-wins (per 74-RESEARCH Pitfall 4).
//! This is an accepted tradeoff — genuine simultaneous browser logins are rare.
//!
//! # Permissions (Unix)
//! Parent dir (`~/.pmcp/`) is chmod'd to `0o700` on first write; cache file is
//! chmod'd to `0o600` before the atomic rename. (Mitigates T-74-G.)

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use url::Url;

/// Per-server token cache. Version 1 schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheV1 {
    /// Schema version. Readers reject any value != 1.
    pub schema_version: u32,
    /// Normalized-URL -> credential entry map. `BTreeMap` for deterministic JSON output.
    pub entries: BTreeMap<String, TokenCacheEntry>,
}

/// One cached server's credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheEntry {
    /// Bearer access token. Sensitive — NEVER logged, never printed except by
    /// `cargo pmcp auth token <url>` (D-11).
    pub access_token: String,
    /// Refresh token, if the IdP issued one (Pitfall 5 — some IdPs don't).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Absolute expiration time (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Granted scopes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Effective OAuth issuer (caller-provided or OIDC-discovered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// Effective client_id (DCR-issued or caller-provided).
    pub client_id: String,
}

impl TokenCacheV1 {
    /// Current on-disk schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Construct an empty (zero-entry) cache at the current schema version.
    pub fn empty() -> Self {
        Self {
            schema_version: Self::CURRENT_VERSION,
            entries: BTreeMap::new(),
        }
    }

    /// Read a cache file, returning `empty()` if the file does not exist.
    /// Errors on malformed JSON or unsupported `schema_version`.
    pub fn read(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let v: Self = serde_json::from_str(&s).with_context(|| {
                    format!("cache file corrupt — delete {} to reset", path.display())
                })?;
                if v.schema_version != Self::CURRENT_VERSION {
                    anyhow::bail!(
                        "cache schema_version {} unsupported (expected {}); upgrade cargo-pmcp",
                        v.schema_version,
                        Self::CURRENT_VERSION
                    );
                }
                Ok(v)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
            Err(e) => Err(anyhow::anyhow!(
                "failed to read cache file {}: {e}",
                path.display()
            )),
        }
    }

    /// Atomic write: tempfile-in-same-dir -> chmod -> persist (rename).
    ///
    /// Cross-platform atomic on modern Linux + Windows per tempfile docs.
    /// Concurrent writers are last-writer-wins (see module rustdoc).
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cache path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache dir {}", parent.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }

        let mut tmp = NamedTempFile::new_in(parent)?;
        let json = serde_json::to_vec_pretty(self)?;
        tmp.write_all(&json)?;
        tmp.flush()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }

        tmp.persist(path)
            .map_err(|e| anyhow::anyhow!("atomic rename failed: {e}"))?;
        Ok(())
    }
}

/// Returns `~/.pmcp/oauth-cache.json` (or `./.pmcp/oauth-cache.json` as a fallback).
/// Distinct from the legacy SDK single-server cache at `~/.pmcp/oauth-tokens.json` (D-07).
pub fn default_multi_cache_path() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".pmcp");
    p.push("oauth-cache.json");
    p
}

/// Normalize an MCP server URL to a stable cache key.
///
/// `scheme://host[:port]` — lowercase host, strip path, strip trailing slash,
/// strip default ports (80 for http, 443 for https). Mitigates T-74-D.
pub fn normalize_cache_key(mcp_server_url: &str) -> Result<String> {
    let parsed = Url::parse(mcp_server_url)
        .map_err(|e| anyhow::anyhow!("Invalid MCP server URL '{}': {e}", mcp_server_url))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL has no host: {mcp_server_url}"))?
        .to_ascii_lowercase();
    let mut base = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        let is_default = (parsed.scheme() == "https" && port == 443)
            || (parsed.scheme() == "http" && port == 80);
        if !is_default {
            base.push_str(&format!(":{}", port));
        }
    }
    Ok(base)
}

/// Refresh grace window constant — transparent refresh triggers when the
/// cached access_token is within `REFRESH_WINDOW_SECS` of expiry (D-15).
pub const REFRESH_WINDOW_SECS: u64 = 60;

/// Returns `true` when `entry.expires_at` is within `grace_secs` of now.
/// Returns `false` when `expires_at` is `None` (treated as long-lived).
pub fn is_near_expiry(entry: &TokenCacheEntry, grace_secs: u64) -> bool {
    let Some(exp) = entry.expires_at else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    exp.saturating_sub(grace_secs) <= now
}

/// Force-refresh the access_token for one cache entry and persist the result.
///
/// Returns the NEW access_token. Errors with an actionable message when
/// `entry.refresh_token.is_none()` (Pitfall 5 + D-16).
pub async fn refresh_and_persist(
    cache_path: &Path,
    key: &str,
    entry: &TokenCacheEntry,
) -> Result<String> {
    let refresh_token = entry.refresh_token.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "no refresh_token cached for {key} — run `cargo pmcp auth login {key}` to re-authenticate"
        )
    })?;
    // Post-Blocker-#6 login.rs always writes `issuer = result.issuer` which is
    // always `Some` for a successful authorize_with_details flow. The None
    // branch below remains as a defensive guard for pre-2.5 cache entries
    // or direct external writes to the cache file.
    let issuer = entry.issuer.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "cached entry for {key} has no issuer — run `cargo pmcp auth login {key}` to re-authenticate"
        )
    })?;

    // Mirror src/client/oauth.rs refresh HTTP pattern.
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building refresh HTTP client")?;
    let disc: serde_json::Value = http
        .get(&discovery_url)
        .send()
        .await
        .with_context(|| format!("discovery GET {discovery_url} failed"))?
        .error_for_status()
        .with_context(|| format!("discovery GET {discovery_url} returned error"))?
        .json()
        .await
        .context("parsing discovery JSON")?;
    let token_endpoint = disc
        .get("token_endpoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("discovery missing token_endpoint"))?;

    #[derive(Deserialize)]
    struct TokenRsp {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<u64>,
    }
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", entry.client_id.as_str()),
    ];
    let rsp: TokenRsp = http
        .post(token_endpoint)
        .form(&form)
        .send()
        .await
        .context("refresh_token POST failed")?
        .error_for_status()
        .with_context(|| {
            format!("refresh failed — run `cargo pmcp auth login {key}` to re-authenticate")
        })?
        .json()
        .await
        .context("parsing token response JSON")?;

    // Persist the new token + rotated refresh_token (if IdP rotated it).
    let mut cache = TokenCacheV1::read(cache_path)?;
    let new_access_token = rsp.access_token.clone();
    let mut updated = entry.clone();
    updated.access_token = new_access_token.clone();
    if let Some(new_rt) = rsp.refresh_token {
        updated.refresh_token = Some(new_rt);
    }
    if let Some(ttl) = rsp.expires_in {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        updated.expires_at = Some(now + ttl);
    }
    cache.entries.insert(key.to_string(), updated);
    cache.write_atomic(cache_path)?;

    Ok(new_access_token)
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_host_and_strips_path_and_trailing_slash() {
        assert_eq!(
            normalize_cache_key("HTTPS://MCP.Example.Com/").unwrap(),
            "https://mcp.example.com"
        );
        assert_eq!(
            normalize_cache_key("https://mcp.example.com:443/v1/api").unwrap(),
            "https://mcp.example.com"
        );
        assert_eq!(
            normalize_cache_key("http://example.com:80").unwrap(),
            "http://example.com"
        );
        assert_eq!(
            normalize_cache_key("http://localhost:8080/mcp").unwrap(),
            "http://localhost:8080"
        );
    }

    #[test]
    fn normalize_errors_on_invalid_url() {
        assert!(normalize_cache_key("not a url").is_err());
    }

    #[test]
    fn read_missing_file_returns_empty_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("does-not-exist.json");
        let c = TokenCacheV1::read(&p).unwrap();
        assert_eq!(c.schema_version, 1);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn read_rejects_wrong_schema_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("bad.json");
        std::fs::write(&p, r#"{"schema_version":99,"entries":{}}"#).unwrap();
        let err = TokenCacheV1::read(&p).unwrap_err();
        assert!(format!("{err}").contains("unsupported"), "got {err}");
    }

    #[test]
    fn write_then_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("cache.json");
        let mut c = TokenCacheV1::empty();
        c.entries.insert(
            "https://mcp.example.com".into(),
            TokenCacheEntry {
                access_token: "at".into(),
                refresh_token: Some("rt".into()),
                expires_at: Some(9_999_999_999),
                scopes: vec!["openid".into()],
                issuer: Some("https://issuer.example".into()),
                client_id: "cid".into(),
            },
        );
        c.write_atomic(&p).unwrap();
        let back = TokenCacheV1::read(&p).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(
            back.entries
                .get("https://mcp.example.com")
                .unwrap()
                .access_token,
            "at"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600_perms_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".pmcp").join("oauth-cache.json");
        TokenCacheV1::empty().write_atomic(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "cache file mode = {:o}", mode);
    }

    #[test]
    fn is_near_expiry_true_within_60s() {
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
        assert!(is_near_expiry(&entry, 60));
    }

    #[test]
    fn is_near_expiry_false_with_no_expiry() {
        let entry = TokenCacheEntry {
            access_token: "at".into(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            issuer: None,
            client_id: "c".into(),
        };
        assert!(!is_near_expiry(&entry, 60));
    }

    #[tokio::test]
    async fn refresh_errors_when_no_refresh_token() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("cache.json");
        let entry = TokenCacheEntry {
            access_token: "at".into(),
            refresh_token: None,
            expires_at: Some(0),
            scopes: vec![],
            issuer: Some("https://issuer.example".into()),
            client_id: "c".into(),
        };
        let err = refresh_and_persist(&p, "https://x.example", &entry)
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("no refresh_token"), "got {err}");
    }

    #[test]
    fn default_multi_cache_path_ends_in_oauth_cache_json() {
        let p = default_multi_cache_path();
        let s = p.to_string_lossy();
        assert!(
            s.ends_with(".pmcp/oauth-cache.json") || s.ends_with(".pmcp\\oauth-cache.json"),
            "got: {s}"
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;

    proptest! {
        #[test]
        fn normalize_round_trip_idempotent(
            scheme in prop_oneof![Just("http"), Just("https")],
            host in "[a-zA-Z][a-zA-Z0-9.-]{1,20}\\.example",
            port_opt in prop::option::of(1025u16..60000),
            path in "/[a-z]{0,10}",
        ) {
            let port_part = port_opt.map(|p| format!(":{p}")).unwrap_or_default();
            let raw = format!("{scheme}://{host}{port_part}{path}");
            let n1 = normalize_cache_key(&raw)
                .map_err(|e| TestCaseError::fail(format!("normalize raw failed: {e}")))?;
            let n2 = normalize_cache_key(&n1)
                .map_err(|e| TestCaseError::fail(format!("normalize n1 failed: {e}")))?;
            prop_assert_eq!(n1, n2);
        }

        #[test]
        fn cache_serde_roundtrip(
            access in "[a-zA-Z0-9._-]{8,40}",
            has_refresh in any::<bool>(),
            has_expiry in any::<bool>(),
            n_scopes in 0usize..4,
        ) {
            let mut c = TokenCacheV1::empty();
            c.entries.insert(
                "https://x.example".into(),
                TokenCacheEntry {
                    access_token: access.clone(),
                    refresh_token: has_refresh.then(|| "rt".to_string()),
                    expires_at: has_expiry.then_some(1_234_567_890),
                    scopes: (0..n_scopes).map(|i| format!("s{i}")).collect(),
                    issuer: Some("https://issuer.example".into()),
                    client_id: "cid".into(),
                },
            );
            let s = serde_json::to_string(&c).unwrap();
            let back: TokenCacheV1 = serde_json::from_str(&s).unwrap();
            prop_assert_eq!(
                back.entries.get("https://x.example").unwrap().access_token.clone(),
                access
            );
        }
    }
}
