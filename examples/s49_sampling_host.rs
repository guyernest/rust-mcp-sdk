//! Example: a sampling **host** — a [`Client`](pmcp::Client) that ANSWERS an
//! inbound spec-direction `sampling/createMessage` from its registered
//! [`HostSamplingHandler`](pmcp::client::host::HostSamplingHandler).
//!
//! This is the MCP host direction (server asks client to run an LLM), the
//! INVERSE of the legacy "LLM-server pattern" that [`Client::create_message`]
//! uses (client asks server). See `pmcp::client::host` for the disambiguation.
//!
//! ## Why a hand-rolled mock server
//!
//! The client answers server -> client requests only while one of its own
//! requests is in flight (there is no background receive pump). And the
//! high-level `Server::run` handles requests on a single serialized loop, so a
//! tool that blocks on `extra.peer().sample()` cannot be answered by the same
//! client whose response that loop must read. So this example drives the server
//! side by hand over an in-process duplex transport: it answers `initialize`,
//! then pushes a raw `sampling/createMessage` frame while the client has a
//! `tools/list` in flight, and prints the completion the client's host handler
//! produced.
//!
//! Run with: `cargo run --example s49_sampling_host`

#![cfg(not(target_arch = "wasm32"))]

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::mpsc;

use pmcp::client::host::HostSamplingHandler;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::jsonrpc::JSONRPCResponse;
use pmcp::types::protocol::{ClientRequest, Request};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResult, SamplingMessage, SamplingMessageContent,
};
use pmcp::types::{
    ClientCapabilities, Content, Implementation, InitializeResult, RequestId, Role,
    ServerCapabilities,
};
use pmcp::ClientBuilder;

// ---------------------------------------------------------------------------
// A minimal in-process duplex transport (examples cannot import test helpers).
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    fn pair() -> (Self, Self) {
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
    async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
        self.tx
            .send(message)
            .map_err(|_| pmcp::Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| pmcp::Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> pmcp::Result<()> {
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

// ---------------------------------------------------------------------------
// The mock "LLM" host handler.
// ---------------------------------------------------------------------------

/// Answers inbound sampling requests with a canned completion. Replace the body
/// with a real LLM call (OpenAI-compatible, Anthropic, etc.) in production.
struct MockLlmHost;

#[async_trait]
impl HostSamplingHandler for MockLlmHost {
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResult> {
        let prompt = params
            .messages
            .first()
            .and_then(|m| match &m.content {
                SamplingMessageContent::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        println!("  [host handler] received sampling request: {prompt:?}");
        Ok(CreateMessageResult::new(
            Content::text(format!("mock completion for: {prompt}")),
            "mock-llm-model",
        ))
    }
}

// ---------------------------------------------------------------------------
// Hand-rolled mock server that requests sampling from the client.
// ---------------------------------------------------------------------------

async fn recv_request_id(t: &mut DuplexTransport) -> RequestId {
    loop {
        if let TransportMessage::Request { id, .. } = t.receive().await.expect("recv request") {
            return id;
        }
    }
}

async fn run_mock_server(mut server_t: DuplexTransport) -> Result<String> {
    // 1. Answer initialize.
    let init_id = recv_request_id(&mut server_t).await;
    let init = InitializeResult::new(
        Implementation::new("mock-server", "1.0.0"),
        ServerCapabilities::tools_only(),
    );
    server_t
        .send(TransportMessage::Response(JSONRPCResponse::success(
            init_id,
            serde_json::to_value(init)?,
        )))
        .await?;

    // 2. Wait for the client's in-flight tools/list (skips `initialized`).
    let call_id = recv_request_id(&mut server_t).await;

    // 3. Push a sampling/createMessage while the call is in flight. A real
    //    transport delivers inbound sampling as the CLIENT-alias variant.
    let params = CreateMessageParams::new(vec![SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: "What is the capital of France?".to_string(),
            meta: None,
        },
    )]);
    let sample_id = RequestId::from("sample-1".to_string());
    server_t
        .send(TransportMessage::Request {
            id: sample_id,
            request: Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(params)))),
        })
        .await?;

    // 4. Read the client's completion.
    let completion = loop {
        if let TransportMessage::Response(r) = server_t.receive().await? {
            break r;
        }
    };
    let value = serde_json::to_value(&completion)?;
    let result: CreateMessageResult = serde_json::from_value(
        value
            .get("result")
            .ok_or_else(|| anyhow!("sampling response was an error"))?
            .clone(),
    )?;
    let text = match &result.content {
        Content::Text { text, .. } => text.clone(),
        _ => "<non-text completion>".to_string(),
    };

    // 5. Answer the original tools/list so the client returns.
    server_t
        .send(TransportMessage::Response(JSONRPCResponse::success(
            call_id,
            json!({ "tools": [] }),
        )))
        .await?;

    Ok(format!("model={}, text={text:?}", result.model))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Sampling host example (client answers inbound sampling/createMessage)\n");

    let (client_t, server_t) = DuplexTransport::pair();
    let server = tokio::spawn(run_mock_server(server_t));

    // Register the host sampling handler on the client.
    let mut client = ClientBuilder::new(client_t)
        .on_sampling(MockLlmHost)
        .build();
    client.initialize(ClientCapabilities::default()).await?;

    // An in-flight request keeps the receive loop alive so the nested inbound
    // sampling request is answered by the host handler.
    println!("  [client] issuing tools/list (keeps the receive loop alive)");
    let _tools = client.list_tools(None).await?;

    let summary = server.await??;
    println!("\nRound-trip complete. Server received completion: {summary}");
    Ok(())
}
