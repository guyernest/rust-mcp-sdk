//! SC-1 (Plan 85-05) — non-SQLite lazy startup + streamable-HTTP smoke.
//!
//! Two guarantees land here:
//!
//! 1. **Lazy startup (D-09).** For the Athena reference configs (open-images,
//!    imdb), [`dispatch`] + [`build_server`] produce a [`pmcp::Server`] whose
//!    curated tool is registered WITHOUT any live cloud connection and WITHOUT
//!    credentials in the env. The whole build is wrapped in a
//!    [`tokio::time::timeout`] so an accidental network / credential wait FAILS
//!    fast (timeout = test failure) rather than hanging the suite.
//!
//! 2. **HTTP smoke.** [`serve`] binds `127.0.0.1:0`, returns the REAL bound
//!    addr, and a basic MCP `initialize` over streamable HTTP succeeds against a
//!    server assembled from the data-bearing Chinook config (full 29-scenario
//!    replay is Plan 06).
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-sql-server --test http_lazy_startup -- --test-threads=1
//! ```

#![cfg(all(feature = "sqlite", feature = "athena"))]

use std::time::Duration;

use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientRequest, Implementation, InitializeRequest, Request, RequestId};
use pmcp_server_toolkit::ServerConfig;
use pmcp_sql_server::{build_server, dispatch, serve};
use url::Url;

const CHINOOK_CONFIG: &str = include_str!("fixtures/reference-config.toml");
const CHINOOK_DDL: &str = include_str!("fixtures/chinook.ddl");
const OPEN_IMAGES_CONFIG: &str =
    include_str!("../../pmcp-server-toolkit/tests/fixtures/open-images-config.toml");
const IMDB_CONFIG: &str = include_str!("../../pmcp-server-toolkit/tests/fixtures/imdb-config.toml");

fn chinook_db_path() -> String {
    format!("{}/tests/fixtures/chinook.db", env!("CARGO_MANIFEST_DIR"))
}

/// SC-1 lazy-startup assertion for one Athena config + its first curated tool.
///
/// Sets `CODE_MODE_SECRET` (the `${CODE_MODE_SECRET}` token_secret resolves at
/// code-mode wiring time) but NO AWS credentials. The dispatch + build is wrapped
/// in a 10s timeout: a hang (accidental network/credential wait) trips it and
/// fails the test, proving D-09 lazy startup.
async fn assert_lazy_startup(config_toml: &str, curated_tool: &str) {
    std::env::set_var("CODE_MODE_SECRET", "sc1-lazy-startup-secret-min-16");
    // Belt-and-braces: ensure no stray creds are present (the dispatch arm sets
    // an explicit region so aws_config never probes IMDS — see Plan 04).
    std::env::remove_var("AWS_ACCESS_KEY_ID");
    std::env::remove_var("AWS_SECRET_ACCESS_KEY");
    std::env::remove_var("AWS_SESSION_TOKEN");

    let cfg = ServerConfig::from_toml_strict_validated(config_toml)
        .expect("athena config must parse + validate");

    let server = tokio::time::timeout(Duration::from_secs(10), async {
        let connector = dispatch(&cfg).await.expect("athena dispatch (offline, no creds)");
        build_server(&cfg, connector, "-- no schema for lazy startup --".to_string())
            .expect("build_server must assemble without a live backend")
    })
    .await
    .expect("dispatch + build_server must NOT hang (D-09 lazy startup) — timeout = network/credential wait");

    assert!(
        server.get_tool(curated_tool).is_some(),
        "curated tool '{curated_tool}' served lazily with no creds / no live backend"
    );
}

#[tokio::test]
async fn open_images_athena_lazy_startup_no_creds() {
    // open-images first [[tools]] entry is `explore_category`.
    assert_lazy_startup(OPEN_IMAGES_CONFIG, "explore_category").await;
}

#[tokio::test]
async fn imdb_athena_lazy_startup_no_creds() {
    // imdb first [[tools]] entry is `search_movies`.
    assert_lazy_startup(IMDB_CONFIG, "search_movies").await;
}

#[tokio::test]
async fn http_initialize_smoke_against_chinook() {
    std::env::set_var("CODE_MODE_SECRET", "http-smoke-secret-min-16-bytes");

    let mut cfg = ServerConfig::from_toml_strict_validated(CHINOOK_CONFIG)
        .expect("chinook config must parse + validate");
    cfg.database.file_path = Some(chinook_db_path());

    let connector = dispatch(&cfg).await.expect("sqlite dispatch");
    let server =
        build_server(&cfg, connector, CHINOOK_DDL.to_string()).expect("build_server for chinook");

    // Bind on an ephemeral loopback port (--test-threads=1 safe) and get the
    // REAL bound addr back.
    let addr = "127.0.0.1:0".parse().expect("loopback addr parses");
    let (bound, handle) = serve(server, addr).await.expect("serve must start");

    // Drive a basic MCP initialize over streamable HTTP via the SDK transport
    // (pattern from tests/streamable_http_integration.rs).
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{bound}")).expect("server url parses"),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };
    let mut transport = StreamableHttpTransport::new(client_config);

    let init = TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
            Implementation::new("sc1-smoke-client", "1.0.0"),
            Default::default(),
        )))),
    };
    transport.send(init).await.expect("send initialize");

    let response = tokio::time::timeout(Duration::from_secs(10), transport.receive())
        .await
        .expect("initialize response must arrive (no hang)")
        .expect("receive initialize response");
    match response {
        TransportMessage::Response(json_response) => {
            assert_eq!(
                json_response.id,
                RequestId::from(1i64),
                "echoes the request id"
            );
        },
        other => panic!("expected an MCP initialize response, got {other:?}"),
    }

    transport.close().await.ok();
    handle.abort();
}
