//! Integration tests for `OAuthHelper`'s SEP-2352 credential-store wiring.
//!
//! The subject is the WIRING, not the store: `116-05` already proved that a
//! `(issuer, account, server)` key keeps both collision classes apart, on the
//! live path, the migration path and the trait level. What is proved here is
//! that `OAuthHelper` actually CONSTRUCTS that key — with all three components,
//! the third derived through `normalize_server_key` — and persists through
//! `save_with_issuer` rather than through the flat, issuer-less
//! `~/.pmcp/oauth-tokens.json` it used to write.
//!
//! # Why several rows SEED the store rather than driving a second flow
//!
//! `pmcp` derives the authorization server from the MCP base URL directly
//! (`get_metadata_with_extras` -> `discover_metadata_with_extras`), and RFC 8414
//! section 3.3 anchoring (116-07) then requires the fetched document to declare
//! exactly that issuer. So two MCP servers at two different origins ALWAYS
//! resolve two different issuers today, and the D-116-R1 case — two MCP servers
//! sharing ONE authorization server and ONE account — is not reachable through
//! the live flow until RFC 9728 Protected Resource Metadata lands, which is
//! DEFERRED by owner decision (2026-08-02).
//!
//! Seeding the second server's entry directly is therefore not a shortcut, it is
//! the only way to build the collision at all — and it makes the assertion
//! SHARPER, because the two entries then differ in NOTHING except the server
//! component. Under the two-part key this phase replaced, they would be one
//! entry.
//!
//! # Feature gate
//!
//! `OAuthHelper` lives behind `oauth`, which `full` does NOT contain, so
//! `make quality-gate` runs **none** of this file. See `D-116-LINT-OAUTH`.

#![cfg(feature = "oauth")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mockito::{Matcher, Mock, Server, ServerGuard};
use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
use pmcp::shared::credential_store::normalize_server_key;
use pmcp::{
    CredentialKey, CredentialStore, CredentialStoreAdmin, InMemoryCredentialStore,
    StoredCredentials,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// An ephemeral loopback port, so concurrently-running rows cannot collide on
/// the callback listener.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Absolute Unix seconds, for expiry fixtures.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// A discovery document that declares `base` as its own issuer, which RFC 8414
/// section 3.3 anchoring requires.
fn discovery_body(base: &str, with_registration: bool) -> String {
    let mut doc = json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": ["openid", "mcp:read"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_registration {
        doc["registration_endpoint"] = json!(format!("{base}/register"));
    }
    doc.to_string()
}

/// A mock authorization server that answers discovery, the token endpoint and
/// (optionally) dynamic client registration.
///
/// The guard AND the mocks are RETURNED, never dropped inline: dropping a
/// `ServerGuard` stops the server and dropping a `Mock` removes it, so a row
/// that let either go would be asserting against a dead endpoint.
async fn authorization_server(with_registration: bool) -> (ServerGuard, Vec<Mock>, String) {
    let mut server = Server::new_async().await;
    let base = server.url();
    let mut mocks = Vec::new();

    mocks.push(
        server
            .mock("GET", "/.well-known/openid-configuration")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_body(&base, with_registration))
            .expect_at_least(0)
            .create_async()
            .await,
    );

    mocks.push(
        server
            .mock("POST", "/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "access_token": "fresh-access-token",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "refresh_token": "fresh-refresh-token",
                    "scope": "openid mcp:read",
                })
                .to_string(),
            )
            .expect_at_least(0)
            .create_async()
            .await,
    );

    if with_registration {
        mocks.push(
            server
                .mock("POST", "/register")
                .match_body(Matcher::PartialJsonString(
                    json!({ "application_type": "native" }).to_string(),
                ))
                .with_status(201)
                .with_header("content-type", "application/json")
                .with_body(json!({ "client_id": "dcr-issued-id" }).to_string())
                .expect_at_least(0)
                .create_async()
                .await,
        );
    }

    (server, mocks, base)
}

/// A [`BrowserLauncher`] that COMPLETES the flow — it lifts `state` out of the
/// authorization URL and delivers a legitimate callback to the loopback
/// listener the flow already bound — and COUNTS how many times it was asked to.
///
/// The count is the observable behind "no browser flow was started": a cache hit
/// and a refused authorization-server substitution must both leave it at zero.
#[derive(Debug)]
struct CountingCallbackLauncher {
    port: u16,
    opened: Arc<AtomicUsize>,
}

impl CountingCallbackLauncher {
    fn new(port: u16) -> (Arc<Self>, Arc<AtomicUsize>) {
        let opened = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Self {
                port,
                opened: opened.clone(),
            }),
            opened,
        )
    }
}

impl BrowserLauncher for CountingCallbackLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        self.opened.fetch_add(1, Ordering::SeqCst);

        let state = Url::parse(url)
            .ok()
            .and_then(|parsed| {
                parsed
                    .query_pairs()
                    .find(|(key, _)| key == "state")
                    .map(|(_, value)| value.into_owned())
            })
            .unwrap_or_default();

        let port = self.port;
        tokio::spawn(async move {
            let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await else {
                return;
            };
            let request = format!(
                "GET /callback?code=granted-code&state={state} HTTP/1.1\r\n\
                 Host: 127.0.0.1\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(request.as_bytes()).await.is_err() {
                return;
            }
            let _ = stream.flush().await;
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response).await;
        });

        Ok(())
    }
}

/// Build a helper wired to `store`, driving its callback through a counting
/// launcher.
fn helper_for(
    base: &str,
    store: &Arc<dyn CredentialStore>,
    client_id: Option<&str>,
) -> (OAuthHelper, Arc<AtomicUsize>) {
    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.to_string()),
        client_id: client_id.map(str::to_string),
        dcr_enabled: client_id.is_none(),
        scopes: vec!["openid".to_string()],
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    (helper, opened)
}

/// Let a spawned callback-driving task finish reading before a mock server is
/// dropped, so nextest does not report the reader as leaky.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

// ---------------------------------------------------------------------------
// Group A — the three-part key
// ---------------------------------------------------------------------------

/// A completed flow lands under `(discovered issuer, account scope, normalized
/// MCP server)`, and the record carries everything a later refresh needs.
#[tokio::test]
async fn a_completed_flow_stores_credentials_under_the_three_part_key() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));

    let result = helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    assert_eq!(opened.load(Ordering::SeqCst), 1, "one browser open");
    assert_eq!(result.access_token, "fresh-access-token");

    let expected = CredentialKey::new(
        &base,
        "",
        normalize_server_key(&base).expect("a normalizable server URL"),
    );
    let stored = store
        .load(&expected)
        .await
        .expect("a readable store")
        .expect("credentials under the three-part key");

    assert_eq!(stored.access_token(), "fresh-access-token");
    assert_eq!(stored.refresh_token(), Some("fresh-refresh-token"));
    assert_eq!(stored.client_id(), "preset-client");
    assert_eq!(stored.granted_scopes(), ["openid", "mcp:read"]);
    assert!(
        stored.expires_at().is_some_and(|at| at > unix_now()),
        "expires_in must become an absolute expiry in the future"
    );
}

/// The server component goes through `normalize_server_key`, so a path and a
/// trailing slash on the configured MCP server URL do not create a second
/// login.
#[tokio::test]
async fn the_server_component_is_normalized_so_a_path_does_not_fork_the_login() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let port = free_port();
    let (launcher, _opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        // A path AND a trailing slash — both dropped by `normalize_server_key`.
        mcp_server_url: Some(format!("{base}/mcp/")),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    let key = CredentialKey::new(&base, "", normalize_server_key(&base).expect("normalized"));
    assert!(
        store.load(&key).await.expect("readable").is_some(),
        "the path-and-slash form must key to the same normalized server"
    );
}

/// A second call with the same issuer, account and server reads the cache and
/// opens no browser.
#[tokio::test]
async fn a_second_call_with_the_same_key_hits_the_cache_and_opens_no_browser() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (first, first_opened) = helper_for(&base, &store, Some("preset-client"));
    let token = first.get_access_token().await.expect("a full flow");
    settle().await;
    assert_eq!(token, "fresh-access-token");
    assert_eq!(first_opened.load(Ordering::SeqCst), 1);

    let (second, second_opened) = helper_for(&base, &store, Some("preset-client"));
    let cached = second.get_access_token().await.expect("a cache hit");
    assert_eq!(cached, "fresh-access-token");
    assert_eq!(
        second_opened.load(Ordering::SeqCst),
        0,
        "a cache hit must not start an interactive flow"
    );
}

// ---------------------------------------------------------------------------
// Group B — the two collision classes
// ---------------------------------------------------------------------------

/// SEP-2352: credentials issued by one authorization server are not reachable
/// from another. The specification's MUST is satisfied by the KEY SHAPE — there
/// is no enforcement branch anywhere for this test to exercise.
///
/// The seeded entry deliberately carries NO issuer RECORD (`save`, not
/// `save_with_issuer`), so this row exercises the cache miss alone and does not
/// also trip D-18's substitution detection, which has its own rows.
#[tokio::test]
async fn credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");

    let foreign = CredentialKey::new("https://other-as.example", "", &server_key);
    store
        .save(
            &foreign,
            &StoredCredentials::new("CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER", "foreign-client")
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding the foreign entry");

    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));
    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(
        token, "CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER",
        "a token issued by a different authorization server must never be returned"
    );
    assert_eq!(token, "fresh-access-token");
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "the miss must produce a real flow, not a silent reuse"
    );

    // The foreign entry is untouched — a miss is not a delete.
    let survivor = store
        .load(&foreign)
        .await
        .expect("readable")
        .expect("the foreign entry survives");
    assert_eq!(
        survivor.access_token(),
        "CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER"
    );
}

/// **D-116-R1 — this is the load-bearing row of this plan.**
///
/// Two MCP servers behind ONE authorization server, under ONE account, hold
/// different dynamic registrations, different client IDs and different granted
/// scopes. The two-part `(issuer, account)` key this phase replaced collapses
/// them into a single entry, so whichever authenticated last silently
/// overwrites the other and a logout on one deletes the other's credentials.
///
/// The assertion is on the credential CONTENTS, not merely on the key count: a
/// two-part key produces "1 != 2" without showing WHAT was lost, and what was
/// lost is the whole point.
///
/// The second server's entry is seeded rather than driven — see this file's
/// module documentation for why that is the only way to build the collision
/// while RFC 9728 is deferred.
#[tokio::test]
async fn two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let server_a = normalize_server_key(&base).expect("normalized");
    let server_b = normalize_server_key("https://other-mcp.example").expect("normalized");
    assert_ne!(server_a, server_b);

    // Server B: same issuer, same (empty) account, different server.
    let key_b = CredentialKey::new(&base, "", &server_b);
    store
        .save(
            &key_b,
            &StoredCredentials::new("CANARY-BELONGING-TO-SERVER-B", "server-b-client")
                .with_granted_scopes(["mcp:write"])
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding server B");

    let (helper, opened) = helper_for(&base, &store, Some("server-a-client"));
    let token = helper.get_access_token().await.expect("a full flow for A");
    settle().await;

    // (a) A's load did NOT see B's credentials.
    assert_ne!(token, "CANARY-BELONGING-TO-SERVER-B");
    assert_eq!(token, "fresh-access-token");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // (b) Two distinct keys now exist, differing ONLY in the server component.
    let key_a = CredentialKey::new(&base, "", &server_a);
    let a = store
        .load(&key_a)
        .await
        .expect("readable")
        .expect("server A's credentials");
    let b = store
        .load(&key_b)
        .await
        .expect("readable")
        .expect("server B's credentials must survive A's write");
    assert_eq!(a.access_token(), "fresh-access-token");
    assert_eq!(a.client_id(), "server-a-client");
    assert_eq!(b.access_token(), "CANARY-BELONGING-TO-SERVER-B");
    assert_eq!(b.client_id(), "server-b-client");
    assert_eq!(b.granted_scopes(), ["mcp:write"]);
    assert_eq!(key_a.issuer(), key_b.issuer());
    assert_eq!(key_a.account(), key_b.account());
    assert_ne!(key_a.server(), key_b.server());

    // (c) Deleting one leaves the other intact.
    store.delete(&key_a).await.expect("deleting A");
    assert!(store.load(&key_a).await.expect("readable").is_none());
    let survivor = store
        .load(&key_b)
        .await
        .expect("readable")
        .expect("B survives A's logout");
    assert_eq!(survivor.access_token(), "CANARY-BELONGING-TO-SERVER-B");
}

/// The account scope is the third axis. A helper scoped to one account must not
/// reach another's credentials, and the default scope is the empty string.
#[tokio::test]
async fn a_different_account_scope_is_a_cache_miss_and_stores_its_own_entry() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");

    let other_account = CredentialKey::new(&base, "cognito-sub-123", &server_key);
    store
        .save(
            &other_account,
            &StoredCredentials::new("CANARY-FOR-ANOTHER-ACCOUNT", "other-account-client")
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding the other account");

    // Default scope — the single-user CLI case.
    let (helper, _opened) = helper_for(&base, &store, Some("preset-client"));
    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;
    assert_ne!(token, "CANARY-FOR-ANOTHER-ACCOUNT");
    assert!(store
        .load(&CredentialKey::new(&base, "", &server_key))
        .await
        .expect("readable")
        .is_some());

    // A helper scoped to that account stores under it.
    let port = free_port();
    let (launcher, _) = CountingCallbackLauncher::new(port);
    let scoped = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("scoped-client".to_string()),
        dcr_enabled: false,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone())
    .with_account_scope("cognito-sub-456");

    scoped.authorize_with_details().await.expect("a full flow");
    settle().await;

    let scoped_key = CredentialKey::new(&base, "cognito-sub-456", &server_key);
    assert_eq!(
        store
            .load(&scoped_key)
            .await
            .expect("readable")
            .expect("the scoped entry")
            .client_id(),
        "scoped-client"
    );
    // And the seeded account is still exactly as it was.
    assert_eq!(
        store
            .load(&other_account)
            .await
            .expect("readable")
            .expect("untouched")
            .access_token(),
        "CANARY-FOR-ANOTHER-ACCOUNT"
    );
}

// ---------------------------------------------------------------------------
// Group C — D-17, the legacy issuer-less cache
// ---------------------------------------------------------------------------

/// The flat `oauth-tokens.json` this module used to write records NO issuer, so
/// it cannot be re-keyed without GUESSING which authorization server issued it —
/// which is exactly what SEP-2352 forbids. It is never opened for reading, and
/// it is left on disk for the user to delete.
///
/// The planted token is distinctive so that "never read" is asserted by its
/// ABSENCE from every result, not by the presence of something else.
#[tokio::test]
async fn the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    let planted = json!({
        "access_token": "CANARY-FROM-THE-LEGACY-FLAT-CACHE",
        "refresh_token": "CANARY-LEGACY-REFRESH",
        "expires_at": unix_now() + 9_000,
        "scopes": ["openid"],
    })
    .to_string();
    std::fs::write(&legacy, &planted).expect("planting the legacy cache");

    let (_server, _mocks, base) = authorization_server(false).await;
    let concrete = Arc::new(InMemoryCredentialStore::new());
    let store: Arc<dyn CredentialStore> = concrete.clone();

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(
        token, "CANARY-FROM-THE-LEGACY-FLAT-CACHE",
        "the issuer-less cache must never be read as credentials"
    );
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "discarding the legacy cache means exactly one forced re-login"
    );

    // The canary reached NOTHING in the new store.
    let listed = concrete.list_keys().await.expect("listable");
    assert!(!listed.is_empty(), "the flow must have stored something");
    for key in listed {
        let record = store
            .load(&key)
            .await
            .expect("readable")
            .expect("a listed key resolves");
        assert_ne!(record.access_token(), "CANARY-FROM-THE-LEGACY-FLAT-CACHE");
        assert_ne!(record.refresh_token(), Some("CANARY-LEGACY-REFRESH"));
    }

    // Left in place, byte for byte — not deleted, not renamed, not rewritten.
    let after = std::fs::read_to_string(&legacy).expect("the legacy file still exists");
    assert_eq!(after, planted, "the legacy file must be left for the user");
}

/// The DEFAULT (uninjected) store lives BESIDE the legacy file, never on top of
/// it.
///
/// This row exists because the row above cannot see the difference: it injects a
/// store, so it never exercises default resolution at all. Pointing the
/// issuer-keyed store at `cache_file` itself would both read the legacy document
/// — which D-17 forbids — and overwrite it, when the whole instruction is to
/// leave it for the user. Measured: with the store pointed at the legacy file,
/// the row above still PASSES and this one fails.
#[tokio::test]
async fn the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    let planted = json!({
        "access_token": "CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE",
        "refresh_token": "CANARY-DEFAULT-PATH-REFRESH",
        "expires_at": unix_now() + 9_000,
        "scopes": ["openid"],
    })
    .to_string();
    std::fs::write(&legacy, &planted).expect("planting the legacy cache");

    let (_server, _mocks, base) = authorization_server(false).await;

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    // NO injected store — this is the default-resolution path.
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(token, "CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // The legacy file is byte-identical: not read, not rewritten, not removed.
    assert_eq!(
        std::fs::read_to_string(&legacy).expect("still there"),
        planted,
        "the default store must not write over the legacy flat cache"
    );

    // And the issuer-keyed document appeared BESIDE it, carrying the new token.
    let store_path = dir.path().join("oauth-cache.json");
    let document = std::fs::read_to_string(&store_path)
        .expect("the issuer-keyed store must live beside the legacy file");
    assert!(document.contains("fresh-access-token"));
    assert!(!document.contains("CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE"));

    // A second helper over the same directory reads that document back.
    let port2 = free_port();
    let (launcher2, opened2) = CountingCallbackLauncher::new(port2);
    let second = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port2,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher2);
    assert_eq!(
        second.get_access_token().await.expect("a cache hit"),
        "fresh-access-token"
    );
    assert_eq!(opened2.load(Ordering::SeqCst), 0);
}

/// An injected store REPLACES the file store entirely: the whole flow touches no
/// file at all, which is what makes the helper usable in an environment with no
/// home directory.
#[tokio::test]
async fn an_injected_store_makes_the_flow_touch_no_file_at_all() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    std::fs::write(&legacy, "{}").expect("planting");

    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let port = free_port();
    let (launcher, _opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readable dir")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "an injected store must create nothing on disk; found {entries:?}"
    );
    assert_eq!(entries[0], "oauth-tokens.json");
}

/// Construction performs no filesystem access — the store, and therefore any
/// home-directory resolution, happens on FIRST USE.
#[tokio::test]
async fn constructing_a_helper_touches_no_filesystem() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let missing = dir.path().join("a").join("b").join("oauth-tokens.json");

    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some("https://mcp.example".to_string()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(missing.clone()),
        ..OAuthConfig::default()
    })
    .expect("helper");

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readable dir")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert!(
        entries.is_empty(),
        "OAuthHelper::new must not create anything; found {entries:?}"
    );
    assert!(!missing.exists());
    drop(helper);
}

/// With no `cache_file` and no injected store the helper persists NOTHING, which
/// is what `cargo pmcp auth login --no-cache` has always meant
/// (`cargo-pmcp/src/commands/auth.rs`: `no_cache` sets `cache_file` to `None`).
///
/// The observable is that a completed flow succeeds and creates no file in the
/// only directory it could plausibly reach.
#[tokio::test]
async fn no_cache_file_and_no_injected_store_persists_nothing() {
    let (_server, _mocks, base) = authorization_server(false).await;

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: None,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let result = helper.authorize_with_details().await.expect("a full flow");
    settle().await;
    assert_eq!(result.access_token, "fresh-access-token");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // A second helper over the same configuration has nothing to read, so it
    // runs the interactive flow again rather than reusing anything.
    let port2 = free_port();
    let (launcher2, opened2) = CountingCallbackLauncher::new(port2);
    let second = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: None,
        redirect_port: port2,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher2);
    second.get_access_token().await.expect("a second full flow");
    settle().await;
    assert_eq!(
        opened2.load(Ordering::SeqCst),
        1,
        "nothing was persisted, so nothing can be reused"
    );
}

// ---------------------------------------------------------------------------
// Group D — what the record carries for 116-12
// ---------------------------------------------------------------------------

/// A DCR flow stores the DCR-ISSUED `client_id` — not the `None` that lives in
/// `config.client_id` — together with the `application_type` the authorization
/// server registered. Both are what makes SEP-2352's "MUST re-register with the
/// new authorization server" automatic: a client id issued by AS-A is stored
/// under AS-A's key and is unreachable from AS-B.
#[tokio::test]
async fn a_dcr_flow_stores_the_issued_client_id_and_the_registered_application_type() {
    let (_server, _mocks, base) = authorization_server(true).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (helper, _opened) = helper_for(&base, &store, None);
    let result = helper.authorize_with_details().await.expect("a DCR flow");
    settle().await;
    assert_eq!(result.client_id, "dcr-issued-id");

    let key = CredentialKey::new(&base, "", normalize_server_key(&base).expect("normalized"));
    let stored = store
        .load(&key)
        .await
        .expect("readable")
        .expect("the DCR record");
    assert_eq!(stored.client_id(), "dcr-issued-id");
    assert_eq!(
        stored.registered_application_type(),
        Some("native"),
        "the registered application_type must reach the store (116-10 -> 116-11)"
    );
}
