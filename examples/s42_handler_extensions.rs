//! Example: cross-middleware state transfer via `RequestHandlerExtra.extensions`.
//!
//! Demonstrates typed-key middleware→handler state passing. The handler
//! retrieves a typed value that middleware would have inserted before the
//! handler runs.
//!
//! Run with: `cargo run --example s42_handler_extensions`

use anyhow::Result;
use async_trait::async_trait;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};

/// Typed value middleware injects into `extra.extensions` before the handler runs.
#[derive(Clone, Debug)]
struct RequestContext {
    user_id: u64,
    request_source: String,
}

/// Real [`ToolHandler`] that reads cross-middleware state from
/// [`RequestHandlerExtra::extensions`].
struct InspectExtensionsTool;

#[async_trait]
impl ToolHandler for InspectExtensionsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let ctx = extra.extensions().get::<RequestContext>();
        let message = match ctx {
            Some(c) => format!(
                "cross-middleware value retrieved: user_id={}, source={}",
                c.user_id, c.request_source
            ),
            None => "no RequestContext found in extensions".to_string(),
        };
        Ok(json!({
            "content": [{"type": "text", "text": message}],
            "isError": false
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Simulate the middleware layer by pre-populating an Extensions entry on
    // a freshly-constructed RequestHandlerExtra, then drive the handler
    // in-process as a real server would.
    let mut extra = RequestHandlerExtra::default();
    extra.extensions_mut().insert(RequestContext {
        user_id: 42,
        request_source: "example".to_string(),
    });

    let handler = InspectExtensionsTool;
    let result = handler.handle(Value::Null, extra).await?;
    println!("handler returned: {}", result);
    Ok(())
}
