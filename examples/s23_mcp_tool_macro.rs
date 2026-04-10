//! Example demonstrating `#[mcp_tool]` and `#[mcp_server]` proc macros
//!
//! This example shows the DX improvement over `TypedTool`/`TypedToolWithOutput`:
//! - No `Box::pin(async move {})` boilerplate
//! - No manual `Arc` cloning for shared state
//! - Description enforced at compile time
//! - Typed output generates `outputSchema` automatically
//!
//! Compare with `examples/32_typed_tools.rs` for the "before" pattern.
//!
//! # Run
//!
//! ```bash
//! cargo run --example 63_mcp_tool_macro --features full
//! ```

use pmcp::{ServerBuilder, ServerCapabilities, State, ToolHandler};
use pmcp_macros::{mcp_server, mcp_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

// === Input/Output types (same as always) ===

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    /// First number
    a: f64,
    /// Second number
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    /// The sum
    result: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Name to greet
    name: String,
}

// === Standalone #[mcp_tool] functions ===

/// Minimal tool -- just args and return.
#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult {
        result: args.a + args.b,
    })
}

/// Tool with shared state via `State<T>`.
#[mcp_tool(description = "Greet with prefix from config")]
async fn greet(args: GreetArgs, config: State<AppConfig>) -> pmcp::Result<Value> {
    Ok(json!({ "greeting": format!("{}, {}!", config.greeting_prefix, args.name) }))
}

/// Sync tool -- auto-detected from `fn` (not `async fn`).
#[mcp_tool(description = "Get server version", annotations(read_only = true))]
fn version() -> pmcp::Result<Value> {
    Ok(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

struct AppConfig {
    greeting_prefix: String,
}

// === #[mcp_server] impl block ===

struct MathServer;

#[mcp_server]
impl MathServer {
    #[mcp_tool(description = "Multiply two numbers")]
    async fn multiply(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult {
            result: args.a * args.b,
        })
    }

    #[mcp_tool(description = "Health check", annotations(read_only = true))]
    async fn health(&self) -> pmcp::Result<Value> {
        Ok(json!({ "status": "ok" }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = Arc::new(AppConfig {
        greeting_prefix: "Hello".into(),
    });

    // Standalone tools registered individually
    let builder = ServerBuilder::new()
        .name("macro-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", add())
        .tool("greet", greet().with_state(config))
        .tool("version", version());

    // Impl-block tools registered in bulk
    let math = MathServer;
    let builder = builder.mcp_server(math);

    // Build the server (validates name + version are set)
    let _server = builder.build()?;
    tracing::info!("Server built with macro-defined tools");

    // Quick verification: call a tool directly
    let tool_handler = add();
    let result = tool_handler
        .handle(
            json!({"a": 3.0, "b": 4.0}),
            pmcp::RequestHandlerExtra::default(),
        )
        .await?;
    tracing::info!("add(3, 4) = {}", result);

    // Check metadata
    let meta = tool_handler.metadata().expect("should have metadata");
    tracing::info!(
        "Tool '{}': {}",
        meta.name,
        meta.description.as_deref().unwrap_or("")
    );
    if meta.output_schema.is_some() {
        tracing::info!("  Has outputSchema (typed output)");
    }

    Ok(())
}
