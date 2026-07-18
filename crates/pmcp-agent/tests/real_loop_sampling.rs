//! AGNT-04 / D-03 full-loop proof: the complete [`AgentEngine`] runs over a
//! [`SamplingSource`] against a real [`Server::run`] + [`Client`], dispatching a
//! host-chosen tool through the invoker and terminating at
//! [`RunOutcome::Completed`].
//!
//! Unlike `sampling_source.rs` (which proves a single `tool_use` block survives
//! the source), this drives the WHOLE loop: the host answers the first sampling
//! request with a `tool_use` and the second with an `end_turn`, so the engine
//! must dispatch the tool via the seam and then complete.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::host::HostSamplingHandlerWithTools;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Role};
use pmcp::{ClientBuilder, RequestHandlerExtra, Server, ToolHandler};

use pmcp_agent::sources::SamplingSource;
use pmcp_agent::{
    AgentEngine, InMemoryStore, ResolvedAgentConfig, RunOutcome, ToolCall, ToolCallResult,
    ToolInvoker,
};

/// An invoker that records how many tool calls it dispatched and echoes ok.
#[derive(Clone, Default)]
struct CountingInvoker {
    dispatched: Arc<AtomicUsize>,
}

#[async_trait]
impl ToolInvoker for CountingInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        self.dispatched.fetch_add(1, Ordering::SeqCst);
        ToolCallResult::ok(call.id, json!({ "searched": call.name }))
    }
}

/// A tool that runs the FULL agent engine over a request-scoped `SamplingSource`
/// and reports the terminal outcome + dispatch count as JSON.
struct FullLoopTool;

#[async_trait]
impl ToolHandler for FullLoopTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra
            .peer()
            .expect("peer must be attached on the stock loop")
            .clone();
        let source = SamplingSource::new(peer);
        let invoker = CountingInvoker::default();
        let config = ResolvedAgentConfig::new("drive the loop", "host-model", 100_000, 5);
        let engine = AgentEngine::new(source, invoker.clone(), InMemoryStore::new(), config);

        let outcome = engine.run("real-loop-run").await;
        let tag = match outcome {
            RunOutcome::Completed { .. } => "completed",
            RunOutcome::LimitReached => "limit_reached",
            RunOutcome::RetryRequired { .. } => "retry_required",
            RunOutcome::Failed { .. } => "failed",
            _ => "unknown",
        };
        Ok(json!({
            "outcome": tag,
            "dispatched": invoker.dispatched.load(Ordering::SeqCst),
        }))
    }
}

/// Host handler: `tool_use` on the first sampling call, `end_turn` on the rest.
struct ToolThenEndTurn {
    calls: AtomicUsize,
}

#[async_trait]
impl HostSamplingHandlerWithTools for ToolThenEndTurn {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResultWithTools> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(CreateMessageResultWithTools::new(
                "host-model",
                Role::Assistant,
                vec![SamplingMessageContent::ToolUse {
                    name: "search".to_string(),
                    id: "call-1".to_string(),
                    input: json!({ "q": "rust" }),
                    meta: None,
                }],
            )
            .with_stop_reason("tool_use"))
        } else {
            Ok(CreateMessageResultWithTools::new(
                "host-model",
                Role::Assistant,
                vec![SamplingMessageContent::Text {
                    text: "here is the answer".to_string(),
                    meta: None,
                }],
            )
            .with_stop_reason("end_turn"))
        }
    }
}

#[tokio::test]
async fn full_engine_over_sampling_source_reaches_completed() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let server = Server::builder()
        .name("agent-full-loop")
        .version("0.1.0")
        .tool("agent", FullLoopTool)
        .build()
        .expect("server builds");
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling_with_tools(ToolThenEndTurn {
            calls: AtomicUsize::new(0),
        })
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

    // The tool returns `{ "outcome": ..., "dispatched": ... }`, serialized by the
    // server into the result's text content. Parse it back and assert the loop
    // completed and dispatched exactly one tool call.
    let text = result
        .content
        .iter()
        .find_map(|c| match c {
            pmcp::types::Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .expect("result has text content");
    let parsed: Value = serde_json::from_str(&text).expect("tool returned JSON");
    assert_eq!(
        parsed["outcome"], "completed",
        "the full loop must reach RunOutcome::Completed: {text}"
    );
    assert_eq!(
        parsed["dispatched"], 1,
        "the loop must have dispatched exactly one tool call: {text}"
    );

    drop(client);
    server_handle.abort();
}
