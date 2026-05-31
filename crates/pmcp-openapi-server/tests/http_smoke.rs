//! SC-1 — curated-only (no-spec) boot + streamable-HTTP smoke (D-03 / CF-1).
//!
//! Drives the REAL binary path ([`run_serving`]) against a curated-only config
//! (NO `--spec`, D-03 proof) over a wiremock REST backend: binds `127.0.0.1:0`,
//! confirms a basic MCP `initialize` + `tools/list` succeeds over streamable
//! HTTP and reflects the configured tools, then ABORTS the server handle
//! (bounded shutdown — no leaked spawned server across tests).
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-openapi-server --test http_smoke -- --test-threads=1
//! ```

use std::time::Duration;

use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{
    ClientRequest, Implementation, InitializeRequest, ListToolsRequest, Request, RequestId,
};
use pmcp_openapi_server::{run_serving, Args};
use url::Url;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A curated-only config (NO spec) pointed at the wiremock backend, with one
/// single-call tool + a code_mode block (so the no-spec + code-mode warn-and-proceed
/// path also exercises here).
fn curated_only_config(backend_url: &str) -> String {
    format!(
        r#"
[server]
name = "openapi-smoke"
version = "0.1.0"

[backend]
base_url = "{backend_url}"

[code_mode]
enabled = true
token_secret = "${{OPENAPI_SMOKE_SECRET}}"

[[tools]]
name = "get_status"
description = "Fetch a status object"
path = "/status"
method = "GET"
"#
    )
}

#[tokio::test]
async fn http_smoke_curated_only_no_spec_boots_and_lists_tools() {
    // The ${OPENAPI_SMOKE_SECRET} token_secret resolves at code-mode wiring time.
    std::env::set_var("OPENAPI_SMOKE_SECRET", "smoke-secret-min-16-bytes-ok");

    // Stand up a wiremock backend (the dispatch builds a connector to it lazily;
    // the smoke test only drives MCP, so the backend need not be hit).
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&backend)
        .await;

    // Write the curated-only config to a temp file — NO --spec (D-03).
    let config_path =
        std::env::temp_dir().join(format!("openapi-smoke-{}.toml", std::process::id()));
    std::fs::write(&config_path, curated_only_config(&backend.uri())).expect("write config");

    let args = Args {
        config: config_path.clone(),
        spec: None, // D-03: curated-only, no spec
        http: "127.0.0.1:0".to_string(),
    };

    // Drive the REAL binary path. A timeout guards against an accidental hang.
    let (bound, handle) = tokio::time::timeout(Duration::from_secs(10), run_serving(&args))
        .await
        .expect("run_serving must not hang")
        .expect("run_serving must boot a curated-only (no-spec) server");

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

    // 1. initialize.
    let init = TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
            Implementation::new("openapi-smoke-client", "1.0.0"),
            Default::default(),
        )))),
    };
    transport.send(init).await.expect("send initialize");
    let init_resp = tokio::time::timeout(Duration::from_secs(10), transport.receive())
        .await
        .expect("initialize response must arrive")
        .expect("receive initialize response");
    assert!(
        matches!(init_resp, TransportMessage::Response(ref r) if r.id == RequestId::from(1i64)),
        "initialize echoes the request id over streamable HTTP (CF-1)"
    );

    // 2. tools/list — must reflect the configured curated tool + code-mode tools.
    let list = TransportMessage::Request {
        id: RequestId::from(2i64),
        request: Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        }))),
    };
    transport.send(list).await.expect("send tools/list");
    let list_resp = tokio::time::timeout(Duration::from_secs(10), transport.receive())
        .await
        .expect("tools/list response must arrive")
        .expect("receive tools/list response");
    match list_resp {
        TransportMessage::Response(json_response) => {
            let rendered = serde_json::to_string(&json_response).expect("serialize response");
            assert!(
                rendered.contains("get_status"),
                "tools/list reflects the configured single-call tool: {rendered}"
            );
            assert!(
                rendered.contains("validate_code") && rendered.contains("execute_code"),
                "tools/list reflects the code-mode tools (no-spec + code-mode runs): {rendered}"
            );
        },
        other => panic!("expected a tools/list response, got {other:?}"),
    }

    // Bounded shutdown — no leaked spawned server.
    transport.close().await.ok();
    handle.abort();
    let _ = std::fs::remove_file(&config_path);
}
