//! Phase 109 Plan 00 — end-to-end proof that a client-supplied namespaced
//! request `_meta` key round-trips through a REAL server tool handler.
//!
//! Task 1 added the `RequestMeta.other` flatten map + `RequestHandlerExtra.request_meta`
//! propagation; Task 2 added `Client::call_tool_with_meta` /
//! `call_tool_with_task_and_meta`. This test wires them together: a tool echoes
//! back the `_meta` it observed via `extra.request_meta`, the client sends a
//! custom key via `call_tool_with_meta`, and we assert the key is observed
//! server-side (proving Task 1 propagation is reachable from the new client API).

#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use pmcp::types::protocol::RequestMeta;
use pmcp::types::{ClientCapabilities, Content};
use pmcp::{Client, RequestHandlerExtra, Result, Server, ToolHandler};
use serde_json::{json, Value};

#[path = "common/duplex.rs"]
mod duplex;
use duplex::DuplexTransport;

/// A tool that echoes back the `_meta` object it observed on the request.
///
/// Returning the raw `_meta` `Value` as the handler payload means the wire
/// `CallToolResult` carries the observed keys in its (text-wrapped) content, so
/// the client-side assertion can confirm end-to-end propagation.
struct EchoMetaTool;

#[async_trait]
impl ToolHandler for EchoMetaTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // `extra.request_meta` is the request's `_meta` object (raw JSON),
        // populated by the ServerCore normal tool-call path in Task 1.
        Ok(extra.request_meta.clone().unwrap_or(Value::Null))
    }
}

fn echo_server() -> Server {
    Server::builder()
        .name("request-meta-echo")
        .version("1.0.0")
        .tool("echo_meta", EchoMetaTool)
        .build()
        .expect("server builds")
}

/// Extract the concatenated text content of a `CallToolResult`.
fn text_of(result: &pmcp::types::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>()
}

#[tokio::test]
async fn call_tool_with_meta_forwards_custom_key_to_handler() {
    let (client_t, server_t) = DuplexTransport::pair();
    let server = echo_server();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes");

    let meta = RequestMeta::new().with_meta("x-pmcp-team-depth", json!(3));
    let result = client
        .call_tool_with_meta("echo_meta".to_string(), json!({}), meta)
        .await
        .expect("call_tool_with_meta succeeds");

    // The handler echoed the observed `_meta`; the custom key must be present,
    // proving the client forwarded it AND Task 1 propagated it to the handler.
    let observed = text_of(&result);
    assert!(
        observed.contains("x-pmcp-team-depth"),
        "custom _meta key not observed server-side: {observed}"
    );
    assert!(
        observed.contains('3'),
        "custom _meta value not observed server-side: {observed}"
    );
}

#[tokio::test]
async fn call_tool_with_meta_empty_behaves_like_plain_call() {
    let (client_t, server_t) = DuplexTransport::pair();
    let server = echo_server();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes");

    // An empty RequestMeta serializes to `{}`, so the handler still observes a
    // (present-but-empty) `_meta` object and no custom keys leak in.
    let result = client
        .call_tool_with_meta("echo_meta".to_string(), json!({}), RequestMeta::new())
        .await
        .expect("call_tool_with_meta with empty meta succeeds");

    let observed = text_of(&result);
    assert!(
        !observed.contains("x-pmcp-team-depth"),
        "no custom key should be present for empty meta: {observed}"
    );
}
