# Chapter 2: Your First MCP Server

In this chapter, you'll build your first Model Context Protocol server using PMCP. We'll start with a simple calculator server and gradually add more features.

## Basic Server Structure

Every MCP server needs:

1. **Tool handlers** - Functions that clients can call
2. **Server configuration** - Name, version, capabilities  
3. **Transport layer** - How clients connect (stdio, WebSocket, HTTP)

Let's build a calculator server step by step.

## Step 1: Project Setup

Create a new Rust project:

```bash
cargo new mcp-calculator
cd mcp-calculator
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "2.0", features = ["full"] }
tokio = { version = "1.0", features = ["full"] }
serde_json = "1.0"
async-trait = "0.1"
```

## Step 2: Basic Calculator Tool

Replace `src/main.rs` with:

```rust
use pmcp::{Server, ToolHandler, RequestHandlerExtra, Result};
use serde_json::{json, Value};
use async_trait::async_trait;

// Define our calculator tool handler
struct Calculator;

#[async_trait]
impl ToolHandler for Calculator {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Extract arguments
        let a = args.get("a")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| pmcp::Error::validation("Missing or invalid parameter 'a'"))?;
            
        let b = args.get("b")
            .and_then(|v| v.as_f64())  
            .ok_or_else(|| pmcp::Error::validation("Missing or invalid parameter 'b'"))?;
            
        let operation = args.get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("add");

        // Perform calculation
        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(pmcp::Error::validation("Division by zero"));
                }
                a / b
            }
            _ => return Err(pmcp::Error::validation("Unknown operation")),
        };

        // Return structured response
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} {} {} = {}", a, operation, b, result)
            }],
            "isError": false
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create and configure the server
    let server = Server::builder()
        .name("calculator-server")
        .version("1.0.0")
        .tool("calculate", Calculator)
        .build()?;

    println!("🧮 Calculator MCP Server starting...");
    println!("Connect using any MCP client on stdio");
    
    // Run on stdio (most common for MCP servers)
    server.run_stdio().await
}
```

## Step 3: Test Your Server

Run the server:

```bash
cargo run
```

You should see:
```
🧮 Calculator MCP Server starting...
Connect using any MCP client on stdio
```

The server is now running and waiting for MCP protocol messages on stdin/stdout.

## Step 4: Test with a Client

Create a simple test client. Add this to `src/bin/test-client.rs`:

```rust
use pmcp::{Client, ClientCapabilities};
use serde_json::json;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Create a client
    let mut client = Client::builder()
        .name("calculator-client")
        .version("1.0.0")
        .capabilities(ClientCapabilities::default())
        .build()?;

    // Connect via stdio to our server
    // In practice, you'd connect via WebSocket or HTTP
    println!("🔗 Connecting to calculator server...");
    
    // For testing, we'll create a manual request
    let request = json!({
        "method": "tools/call",
        "params": {
            "name": "calculate",
            "arguments": {
                "a": 10,
                "b": 5,
                "operation": "multiply"
            }
        }
    });

    println!("📤 Sending request: {}", serde_json::to_string_pretty(&request)?);
    
    // In a real client, you'd send this via the transport and get a response
    println!("✅ Calculator server is ready to receive requests!");
    
    Ok(())
}
```

Build the test client:

```bash
cargo build --bin test-client
```

## Step 5: Enhanced Server with Multiple Tools

Let's add more tools to make our server more useful. Update `src/main.rs`:

```rust
use pmcp::{Server, ToolHandler, RequestHandlerExtra, Result, ServerCapabilities, ToolCapabilities};
use serde_json::{json, Value};
use async_trait::async_trait;
use std::collections::HashMap;

// Calculator tool (same as before)
struct Calculator;

#[async_trait]
impl ToolHandler for Calculator {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let a = args.get("a").and_then(|v| v.as_f64())
            .ok_or_else(|| pmcp::Error::validation("Missing parameter 'a'"))?;
        let b = args.get("b").and_then(|v| v.as_f64())  
            .ok_or_else(|| pmcp::Error::validation("Missing parameter 'b'"))?;
        let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("add");

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(pmcp::Error::validation("Division by zero"));
                }
                a / b
            }
            _ => return Err(pmcp::Error::validation("Unknown operation")),
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} {} {} = {}", a, operation, b, result)
            }],
            "isError": false
        }))
    }
}

// Statistics tool - demonstrates stateful operations
struct Statistics {
    calculations: tokio::sync::Mutex<Vec<f64>>,
}

impl Statistics {
    fn new() -> Self {
        Self {
            calculations: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ToolHandler for Statistics {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let operation = args.get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| pmcp::Error::validation("Missing 'operation' parameter"))?;

        let mut calculations = self.calculations.lock().await;

        match operation {
            "add_value" => {
                let value = args.get("value").and_then(|v| v.as_f64())
                    .ok_or_else(|| pmcp::Error::validation("Missing 'value' parameter"))?;
                calculations.push(value);
                
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Added {} to statistics. Total values: {}", value, calculations.len())
                    }],
                    "isError": false
                }))
            }
            "get_stats" => {
                if calculations.is_empty() {
                    return Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": "No data available for statistics"
                        }],
                        "isError": false
                    }));
                }

                let sum: f64 = calculations.iter().sum();
                let count = calculations.len();
                let mean = sum / count as f64;
                let min = calculations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max = calculations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Statistics:\n• Count: {}\n• Sum: {:.2}\n• Mean: {:.2}\n• Min: {:.2}\n• Max: {:.2}",
                            count, sum, mean, min, max
                        )
                    }],
                    "isError": false
                }))
            }
            "clear" => {
                calculations.clear();
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "Statistics cleared"
                    }],
                    "isError": false
                }))
            }
            _ => Err(pmcp::Error::validation("Unknown statistics operation")),
        }
    }
}

// System info tool - demonstrates environment interaction
struct SystemInfo;

#[async_trait]
impl ToolHandler for SystemInfo {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let info = json!({
            "server": "calculator-server",
            "version": "1.0.0",
            "protocol_version": "2025-06-18",
            "features": ["calculation", "statistics", "system_info"],
            "uptime": "Just started", // In a real app, you'd track actual uptime
            "rust_version": env!("RUSTC_VERSION")
        });

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("System Information:\n{}", serde_json::to_string_pretty(&info)?)
            }],
            "isError": false
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create shared statistics handler
    let stats_handler = Statistics::new();

    // Create and configure the enhanced server
    let server = Server::builder()
        .name("calculator-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        })
        .tool("calculate", Calculator)
        .tool("statistics", stats_handler)
        .tool("system_info", SystemInfo)
        .build()?;

    println!("🧮 Enhanced Calculator MCP Server starting...");
    println!("Available tools:");
    println!("  • calculate - Basic arithmetic operations");
    println!("  • statistics - Statistical calculations on datasets");
    println!("  • system_info - Server information");
    println!();
    println!("Connect using any MCP client on stdio");
    
    // Run the server
    server.run_stdio().await
}
```

## Step 6: Error Handling Best Practices

PMCP provides comprehensive error handling. Here's how to handle different error scenarios:

```rust
use pmcp::Error;

// Input validation errors
if args.is_null() {
    return Err(Error::validation("Arguments cannot be null"));
}

// Protocol errors  
if unsupported_feature {
    return Err(Error::protocol(
        pmcp::ErrorCode::InvalidRequest,
        "This feature is not supported"
    ));
}

// Internal errors
if let Err(e) = some_operation() {
    return Err(Error::internal(format!("Operation failed: {}", e)));
}

// Custom errors with structured data
return Err(Error::custom(
    -32001,  // Custom error code
    "Custom error occurred",
    Some(json!({
        "error_type": "custom",
        "context": "additional_info"
    }))
));
```

## Step 7: Testing Your Server

Create comprehensive tests in `src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_calculator_basic_operations() {
        let calculator = Calculator;
        let extra = RequestHandlerExtra::new(
            "test".to_string(),
            tokio_util::sync::CancellationToken::new(),
        );

        // Test addition
        let args = json!({"a": 5, "b": 3, "operation": "add"});
        let result = calculator.handle(args, extra.clone()).await.unwrap();
        
        assert!(!result["isError"].as_bool().unwrap_or(true));
        assert!(result["content"][0]["text"].as_str().unwrap().contains("5 add 3 = 8"));

        // Test division by zero
        let args = json!({"a": 5, "b": 0, "operation": "divide"});
        let result = calculator.handle(args, extra.clone()).await;
        assert!(result.is_err());
    }

    #[tokio::test] 
    async fn test_statistics() {
        let stats = Statistics::new();
        let extra = RequestHandlerExtra::new(
            "test".to_string(),
            tokio_util::sync::CancellationToken::new(),
        );

        // Add some values
        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            let args = json!({"operation": "add_value", "value": value});
            let result = stats.handle(args, extra.clone()).await.unwrap();
            assert!(!result["isError"].as_bool().unwrap_or(true));
        }

        // Get statistics
        let args = json!({"operation": "get_stats"});
        let result = stats.handle(args, extra.clone()).await.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        
        assert!(text.contains("Count: 5"));
        assert!(text.contains("Mean: 3.00"));
    }
}
```

Run the tests:

```bash
cargo test
```

## Step 8: Production Considerations

For production deployment, consider these enhancements:

### Logging and Tracing

```rust
use tracing::{info, warn, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting calculator server");
    
    // Your server code here...
    
    Ok(())
}
```

### Configuration

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct Config {
    server_name: String,
    max_connections: usize,
    log_level: String,
}

fn load_config() -> Config {
    // Load from environment variables, config file, etc.
    Config {
        server_name: std::env::var("SERVER_NAME")
            .unwrap_or_else(|_| "calculator-server".to_string()),
        max_connections: std::env::var("MAX_CONNECTIONS")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .unwrap_or(100),
        log_level: std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string()),
    }
}
```

### Metrics and Health Checks

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

struct Metrics {
    requests_total: AtomicU64,
    errors_total: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
        }
    }
    
    fn increment_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }
    
    fn increment_errors(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }
}

// Use metrics in your tool handlers
#[async_trait]
impl ToolHandler for Calculator {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Increment request counter
        self.metrics.increment_requests();
        
        // Your tool logic here...
        
        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                self.metrics.increment_errors();
                Err(e)
            }
        }
    }
}
```

## What's Next?

You've built a complete MCP server with:

- ✅ Multiple tool handlers
- ✅ Proper error handling  
- ✅ Stateful operations
- ✅ Comprehensive tests
- ✅ Production considerations

In the next chapter, we'll build a client that connects to your server and demonstrates the full request-response cycle.

## Complete Example

The complete working example is available in the PMCP repository:
- **Server**: `examples/02_server_basic.rs`
- **Enhanced Server**: `examples/calculator_server.rs` 
- **Tests**: `tests/calculator_tests.rs`

## Key Takeaways

1. **Tool handlers are the core** - They define what your server can do
2. **Error handling is crucial** - Use PMCP's error types for protocol compliance
3. **State management works** - Use Rust's sync primitives for shared state
4. **Testing is straightforward** - PMCP handlers are easy to unit test  
5. **Production readiness matters** - Consider logging, metrics, and configuration

Ready to build a client? Let's go to Chapter 3!