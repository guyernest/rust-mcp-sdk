//! Example: calling `extra.peer().sample()` from INSIDE a real
//! [`ToolHandler`].
//!
//! Phase 70 / PARITY-HANDLER-01. Contrasts against
//! `examples/s30_tool_with_sampling.rs` which uses the registration-time
//! `SamplingHandler` trait. This example demonstrates BOTH halves of Phase 70:
//!
//!   1. Cross-middleware `extensions` insert/retrieve (inside handler)
//!   2. In-handler `peer.sample()` round-trip (against an in-example
//!      `MockPeer`)
//!
//! The `MockPeer` is for demonstration only — it returns canned responses
//! without crossing any real transport, so the example finishes in <5s.
//!
//! Run with: `cargo run --example s43_handler_peer_sample`

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use pmcp::server::roots::ListRootsResult;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use pmcp::types::{Content, ProgressToken};
use pmcp::{PeerHandle, RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};

/// Typed value middleware injects into extensions before the handler runs.
#[derive(Clone, Debug)]
struct RequestContext {
    user_id: u64,
}

/// In-example mock peer. For demonstration only — does not cross any
/// transport. Returns canned responses synchronously.
struct MockPeer;

#[async_trait]
impl PeerHandle for MockPeer {
    async fn sample(&self, _params: CreateMessageParams) -> pmcp::Result<CreateMessageResult> {
        // Use REAL constructor — `CreateMessageResult` has no Default impl.
        Ok(CreateMessageResult::new(
            Content::text("mock response"),
            "mock-model",
        ))
    }

    async fn list_roots(&self) -> pmcp::Result<ListRootsResult> {
        Ok(ListRootsResult { roots: Vec::new() })
    }

    async fn progress_notify(
        &self,
        _token: ProgressToken,
        _progress: f64,
        _total: Option<f64>,
        _message: Option<String>,
    ) -> pmcp::Result<()> {
        Ok(())
    }
}

/// Real [`ToolHandler`] that reads middleware-populated state AND calls
/// `peer.sample()` from inside its `handle()` body. This is the intended
/// Phase 70 usage pattern.
struct PeerSamplingTool;

#[async_trait]
impl ToolHandler for PeerSamplingTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // --- Half 1: cross-middleware extension read ---
        let ctx = extra
            .extensions()
            .get::<RequestContext>()
            .cloned()
            .unwrap_or(RequestContext { user_id: 0 });

        // --- Half 2: in-handler peer.sample() round-trip ---
        let sample_result = if let Some(peer) = extra.peer() {
            // Use REAL constructor — `CreateMessageParams` has no Default
            // impl.
            let params = CreateMessageParams::new(Vec::new());
            let result = peer.sample(params).await?;
            format!("peer sampled via model: {}", result.model)
        } else {
            "no peer attached to extra".to_string()
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("user_id={}, {}", ctx.user_id, sample_result)
            }],
            "isError": false
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Construct the extra the way a real ServerCore would, but in-process:
    // middleware inserts state, runtime attaches a peer, then the handler
    // runs.
    let mut extra =
        RequestHandlerExtra::default().with_peer(Arc::new(MockPeer) as Arc<dyn PeerHandle>);
    extra
        .extensions_mut()
        .insert(RequestContext { user_id: 42 });

    let handler = PeerSamplingTool;
    let result = handler.handle(Value::Null, extra).await?;
    println!("handler returned: {}", result);
    Ok(())
}
