//! Example demonstrating the refactored server architecture with protocol/transport split.
//!
//! This example shows how to use the new transport-independent ServerCore
//! with different transport adapters.

use async_trait::async_trait;
use pmcp::server::adapters::{StdioAdapter, TransportAdapter};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::ToolHandler;
use pmcp::Result;
use serde_json::{json, Value};
use std::sync::Arc;

/// Example tool that echoes input back
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        Ok(json!({
            "echo": args,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Example tool that performs calculations
struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("add");
        let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b != 0.0 {
                    a / b
                } else {
                    return Ok(json!({"error": "Division by zero"}));
                }
            },
            _ => return Ok(json!({"error": "Unknown operation"})),
        };

        Ok(json!({ "result": result }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Build the server core using the builder pattern
    let server_core = ServerCoreBuilder::new()
        .name("refactored-example-server")
        .version("0.1.0")
        .tool("echo", EchoTool)
        .tool("calculator", CalculatorTool)
        .build()?;

    // Convert to Arc for sharing with transport adapter
    let handler: Arc<dyn ProtocolHandler> = Arc::new(server_core);

    // Choose transport adapter based on environment
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("Starting server with STDIO transport...");
        let adapter = StdioAdapter::new();
        adapter.serve(handler).await?;
    }

    #[cfg(target_arch = "wasm32")]
    {
        println!("WASM environment detected - use WASI HTTP adapter");
        // In WASM, you would use the WasiHttpAdapter
        // This would typically be integrated with the WASI HTTP world
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::server::adapters::MockAdapter;
    use pmcp::types::{ClientRequest, Implementation, InitializeParams, Request, RequestId};

    #[tokio::test]
    async fn test_refactored_server() {
        // Create server
        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("echo", EchoTool)
            .build()
            .unwrap();

        let handler: Arc<dyn ProtocolHandler> = Arc::new(server);

        // Create mock adapter for testing
        let adapter = MockAdapter::new();

        // Add initialization request
        let init_request = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: pmcp::types::ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));

        adapter
            .add_request(RequestId::from(1i64), init_request)
            .await;

        // Serve the requests
        adapter.serve(handler).await.unwrap();

        // Check responses
        let responses = adapter.get_responses().await;
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].id, RequestId::from(1i64));
    }
}
