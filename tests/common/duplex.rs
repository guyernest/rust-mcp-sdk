//! Shared in-process duplex transport + call helpers for dispatcher tests.
//!
//! Extracted from the (previously copy-pasted) harness in
//! `tests/tool_output_passthrough.rs` / `tests/tool_with_result.rs` et al.:
//! an mpsc-backed client<->server [`Transport`] pair, plus helpers that drive
//! a `tools/call` through a real [`Client`] against either a high-level
//! [`Server`] (own transport loop) or a `ServerCore` [`ProtocolHandler`]
//! (request/response pump).
//!
//! Each file in `tests/` is compiled as a separate integration crate; include
//! this module per-crate via `#[path = "common/duplex.rs"] mod duplex;`.

#![allow(dead_code)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::core::ProtocolHandler;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{CallToolResult, ClientCapabilities};
use pmcp::{Client, Error, Result, Server};
use serde_json::Value;
use tokio::sync::mpsc;

/// One half of an in-process duplex transport. The client side sends Requests
/// and receives Responses; the server side does the reverse.
#[derive(Debug)]
pub struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    /// Create a connected client/server transport pair.
    pub fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}

/// Drive a `tools/call` through a real [`Client`] against a high-level
/// [`Server`] running its own transport loop.
pub async fn call_via_server(server: Server, name: &str, args: Value) -> CallToolResult {
    let (client_t, server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against server");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against server")
}

/// Drive a `tools/call` through a real [`Client`] against a `ServerCore`
/// served by a request/response pump over the duplex transport.
pub async fn call_via_core(
    core: Arc<dyn ProtocolHandler>,
    name: &str,
    args: Value,
) -> CallToolResult {
    let (client_t, mut server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        while let Ok(message) = server_t.receive().await {
            if let TransportMessage::Request { id, request } = message {
                let response = core.handle_request(id, request, None).await;
                if server_t
                    .send(TransportMessage::Response(response))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against core");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against core")
}
