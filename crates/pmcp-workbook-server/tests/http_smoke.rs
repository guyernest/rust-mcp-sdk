//! Task 1 (Plan 95-02) — ephemeral-port HTTP `initialize` smoke test.
//!
//! Builds a [`pmcp::Server`] from the committed golden bundle, [`serve`]s it on
//! `127.0.0.1:0`, captures the REAL bound address, then drives a FULL MCP
//! `initialize` round-trip over the SDK [`StreamableHttpTransport`] and asserts
//! the response echoes the request id.
//!
//! The assertion is on the `initialize` RESPONSE — NOT merely on the bind
//! result (Gemini #10). A bound-but-dead socket would accept the connection and
//! then never answer; gating on the protocol response proves the server is
//! ACTUALLY LISTENING and answering MCP, then `handle.abort()` tears it down
//! deterministically (T-95-08).
//!
//! Run with (single-threaded — ephemeral port):
//! ```sh
//! cargo test -p pmcp-workbook-server --test http_smoke -- --test-threads=1
//! ```

use std::path::PathBuf;
use std::time::Duration;

use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientRequest, Implementation, InitializeRequest, Request, RequestId};
use pmcp_workbook_server::{build_server, serve, Args};
use url::Url;

/// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
/// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR`.
fn golden_bundle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

#[tokio::test]
async fn http_initialize_smoke_against_golden_bundle() {
    let args = Args {
        bundle_dir: golden_bundle_dir(),
        bundle_id: None,
        http: "127.0.0.1:0".to_string(),
    };
    let server = build_server(&args).expect("golden bundle assembles a server");

    // Bind on an ephemeral loopback port (--test-threads=1 safe) and get the
    // REAL bound addr back.
    let addr = "127.0.0.1:0".parse().expect("loopback addr parses");
    let (bound, handle) = serve(server, addr).await.expect("serve must start");

    // Drive a basic MCP initialize over streamable HTTP via the SDK transport
    // (config struct transfers verbatim from pmcp-sql-server's http smoke).
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
            Implementation::new("workbook-smoke-client", "1.0.0"),
            Default::default(),
        )))),
    };
    transport.send(init).await.expect("send initialize");

    // The assertion is on the RESPONSE — proving the server is actually
    // listening and answering protocol, not merely that the socket bound.
    let response = tokio::time::timeout(Duration::from_secs(10), transport.receive())
        .await
        .expect("initialize response must arrive (no hang)")
        .expect("receive initialize response");
    match response {
        TransportMessage::Response(json_response) => {
            assert_eq!(
                json_response.id,
                RequestId::from(1i64),
                "the live server echoes the initialize request id"
            );
        },
        other => panic!("expected an MCP initialize response, got {other:?}"),
    }

    transport.close().await.ok();
    handle.abort();
}
