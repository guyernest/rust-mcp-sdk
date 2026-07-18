//! AGNT-04 real-loop proof: [`SamplingSource`] drives a completion over the
//! server-side peer on the **stock** `Server::run` loop, and a `tool_use` block
//! chosen by the hosting client survives end-to-end into the
//! `CreateMessageResultWithTools` the source returns.
//!
//! Unlike a unit test with a stub peer, this builds a real [`Server`] whose tool
//! constructs a `SamplingSource` from `extra.peer()`, calls `create_message`,
//! and reports the first `tool_use` it observed; a real `Client` (built with
//! `on_sampling_with_tools`) answers the sampling request with a ToolUse block.
//! This rides the Phase 108-01 Transport Actor fix (D-106-A) + the WithTools
//! client-host path.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::host::HostSamplingHandlerWithTools;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessage, SamplingMessageContent,
    ToolChoice,
};
use pmcp::types::{ClientCapabilities, Role};
use pmcp::{ClientBuilder, RequestHandlerExtra, Server, ToolHandler};

use pmcp_agent::seams::CompletionSource;
use pmcp_agent::sources::SamplingSource;

/// A tool that builds a `SamplingSource` from the request-scoped peer, drives a
/// completion carrying tools + a `ToolChoice`, and reports the first observed
/// `tool_use` block (`name#id`) — the AGNT-04 assertion surface.
struct AgentTool;

#[async_trait]
impl ToolHandler for AgentTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra
            .peer()
            .expect("peer must be attached on the stock loop")
            .clone();
        let source = SamplingSource::new(peer);

        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "pick a tool".to_string(),
                meta: None,
            },
        )])
        .with_tool_choice(ToolChoice::auto());

        let result: CreateMessageResultWithTools = source
            .create_message(params)
            .await
            .map_err(|e| pmcp::Error::internal(format!("completion failed: {e}")))?;

        let tool_use = result
            .content
            .iter()
            .find_map(|c| match c {
                SamplingMessageContent::ToolUse { id, name, .. } => Some(format!("{name}#{id}")),
                _ => None,
            })
            .unwrap_or_else(|| "none".to_string());
        Ok(json!(format!("tooluse:{tool_use}")))
    }
}

/// `WithTools` host handler answering with a `tool_use` block.
struct ToolUseSampling;

#[async_trait]
impl HostSamplingHandlerWithTools for ToolUseSampling {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResultWithTools> {
        Ok(CreateMessageResultWithTools::new(
            "host-tool-model",
            Role::Assistant,
            vec![SamplingMessageContent::ToolUse {
                name: "search".to_string(),
                id: "call-42".to_string(),
                input: json!({ "q": "rust" }),
                meta: None,
            }],
        ))
    }
}

fn result_text(result: &pmcp::types::CallToolResult) -> String {
    serde_json::to_value(result).unwrap().to_string()
}

#[tokio::test]
async fn sampling_source_preserves_tool_use_end_to_end() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let server = Server::builder()
        .name("agent-sampling-source")
        .version("0.1.0")
        .tool("agent", AgentTool)
        .build()
        .expect("server builds");
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling_with_tools(ToolUseSampling)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("agent".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    assert!(
        result_text(&result).contains("tooluse:search#call-42"),
        "tool_use block (name + id) must survive through SamplingSource: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}
