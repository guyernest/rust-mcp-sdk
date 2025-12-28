# The Calculator Server

Let's examine the calculator server in detail. This simple example demonstrates all the patterns you'll use in production MCP servers.

## Server Entry Point

The `main.rs` file is the server's entry point:

```rust
// servers/calculator/src/main.rs
use pmcp::prelude::*;
use server_common::serve_http;
use std::net::{Ipv4Addr, SocketAddr};

mod tools;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Build the MCP server
    let server = Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", tools::AddTool)
        .tool("subtract", tools::SubtractTool)
        .tool("multiply", tools::MultiplyTool)
        .tool("divide", tools::DivideTool)
        .build()?;

    // Start HTTP server
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 3000);
    tracing::info!("Starting calculator server");
    
    serve_http(server, addr).await
}
```

**Key elements:**

| Line | Purpose |
|------|---------|
| `use pmcp::prelude::*` | Imports common types (Server, ServerCapabilities, etc.) |
| `mod tools` | Includes the tools module |
| `#[tokio::main]` | Enables async main function |
| `Server::builder()` | Fluent API for server configuration |
| `.tool("name", Handler)` | Registers each tool |
| `serve_http(server, addr)` | Starts the HTTP transport |

## Tool Module Structure

Tools are organized in the `tools/` directory:

```
src/tools/
├── mod.rs          # Module exports
└── calculator.rs   # Tool implementations
```

The `mod.rs` file exports the tool handlers:

```rust
// src/tools/mod.rs
mod calculator;

pub use calculator::{AddTool, SubtractTool, MultiplyTool, DivideTool};
```

## Anatomy of a Tool

Let's examine the `AddTool` in detail:

```rust
// src/tools/calculator.rs
use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Input arguments for the add operation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArgs {
    /// First number to add
    pub a: f64,
    /// Second number to add  
    pub b: f64,
}

/// Result of the add operation
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddResult {
    /// The sum of a and b
    pub result: f64,
    /// Human-readable expression
    pub expression: String,
}

/// Tool that adds two numbers
pub struct AddTool;

#[async_trait]
impl ToolHandler for AddTool {
    async fn handle(
        &self, 
        args: Value, 
        _extra: RequestHandlerExtra
    ) -> Result<Value, Error> {
        // Parse and validate arguments
        let input: AddArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        // Perform the calculation
        let sum = input.a + input.b;
        
        // Return structured result
        let result = AddResult {
            result: sum,
            expression: format!("{} + {} = {}", input.a, input.b, sum),
        };
        
        Ok(serde_json::to_value(result)?)
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(AddArgs);
        Some(pmcp::types::ToolInfo::new(
            "add",
            Some("Add two numbers together".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}
```

### Breaking It Down

#### 1. Input Type with Schema

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArgs {
    /// First number to add
    pub a: f64,
    /// Second number to add  
    pub b: f64,
}
```

- `Deserialize` - Parses JSON into this struct
- `JsonSchema` - Generates JSON Schema for validation
- Doc comments (`///`) become field descriptions in the schema

The generated schema tells Claude exactly what parameters the tool accepts:

```json
{
  "type": "object",
  "properties": {
    "a": { "type": "number", "description": "First number to add" },
    "b": { "type": "number", "description": "Second number to add" }
  },
  "required": ["a", "b"]
}
```

#### 2. Output Type

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddResult {
    pub result: f64,
    pub expression: String,
}
```

- `Serialize` - Converts the struct to JSON
- Structured output helps Claude understand and use the result

#### 3. The Handler

```rust
#[async_trait]
impl ToolHandler for AddTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        // Implementation
    }
}
```

- `async` - All handlers are async for consistency
- `args: Value` - Raw JSON input from the client
- `_extra: RequestHandlerExtra` - Additional context (we'll use this later)
- Returns `Result<Value, Error>` - JSON value or error

#### 4. Metadata for Discovery

```rust
fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
    let schema = schemars::schema_for!(AddArgs);
    Some(pmcp::types::ToolInfo::new(
        "add",
        Some("Add two numbers together".to_string()),
        serde_json::to_value(&schema).unwrap_or_default(),
    ))
}
```

This tells MCP clients:
- Tool name: `"add"`
- Description: `"Add two numbers together"`
- Input schema: Generated from `AddArgs`

## Error Handling: The Divide Tool

The divide tool shows proper error handling:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DivideArgs {
    /// The dividend (number to be divided)
    pub dividend: f64,
    /// The divisor (number to divide by)
    pub divisor: f64,
}

pub struct DivideTool;

#[async_trait]
impl ToolHandler for DivideTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: DivideArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        // Validate: prevent division by zero
        if input.divisor == 0.0 {
            return Err(Error::validation("Cannot divide by zero"));
        }
        
        let quotient = input.dividend / input.divisor;
        
        Ok(json!({
            "result": quotient,
            "expression": format!("{} ÷ {} = {}", input.dividend, input.divisor, quotient)
        }))
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(DivideArgs);
        Some(pmcp::types::ToolInfo::new(
            "divide",
            Some("Divide two numbers. Returns an error if divisor is zero.".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}
```

### Error Types

PMCP provides error types that map to MCP error codes:

| Error Type | When to Use | MCP Code |
|------------|-------------|----------|
| `Error::validation(msg)` | Invalid input from client | -32602 |
| `Error::internal(msg)` | Server-side failures | -32603 |
| `Error::not_found(msg)` | Resource doesn't exist | -32001 |
| `Error::permission_denied(msg)` | Authorization failure | -32002 |

When Claude sees a validation error, it understands the request was malformed and can try again with corrected input.

## The Complete Calculator Module

Here's the full `calculator.rs` with all four operations:

```rust
use async_trait::async_trait;
use pmcp::{Error, RequestHandlerExtra, ToolHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// === Shared Types ===

#[derive(Debug, Serialize, JsonSchema)]
pub struct CalculationResult {
    pub result: f64,
    pub expression: String,
}

// === Add Tool ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArgs {
    /// First number
    pub a: f64,
    /// Second number
    pub b: f64,
}

pub struct AddTool;

#[async_trait]
impl ToolHandler for AddTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: AddArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        let result = input.a + input.b;
        Ok(serde_json::to_value(CalculationResult {
            result,
            expression: format!("{} + {} = {}", input.a, input.b, result),
        })?)
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(AddArgs);
        Some(pmcp::types::ToolInfo::new(
            "add",
            Some("Add two numbers".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}

// === Subtract Tool ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubtractArgs {
    /// Number to subtract from
    pub a: f64,
    /// Number to subtract
    pub b: f64,
}

pub struct SubtractTool;

#[async_trait]
impl ToolHandler for SubtractTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: SubtractArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        let result = input.a - input.b;
        Ok(serde_json::to_value(CalculationResult {
            result,
            expression: format!("{} - {} = {}", input.a, input.b, result),
        })?)
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(SubtractArgs);
        Some(pmcp::types::ToolInfo::new(
            "subtract",
            Some("Subtract two numbers".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}

// === Multiply Tool ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MultiplyArgs {
    /// First factor
    pub a: f64,
    /// Second factor
    pub b: f64,
}

pub struct MultiplyTool;

#[async_trait]
impl ToolHandler for MultiplyTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: MultiplyArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        let result = input.a * input.b;
        Ok(serde_json::to_value(CalculationResult {
            result,
            expression: format!("{} × {} = {}", input.a, input.b, result),
        })?)
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(MultiplyArgs);
        Some(pmcp::types::ToolInfo::new(
            "multiply",
            Some("Multiply two numbers".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}

// === Divide Tool ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DivideArgs {
    /// The dividend
    pub dividend: f64,
    /// The divisor (cannot be zero)
    pub divisor: f64,
}

pub struct DivideTool;

#[async_trait]
impl ToolHandler for DivideTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: DivideArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
        
        if input.divisor == 0.0 {
            return Err(Error::validation("Cannot divide by zero"));
        }
        
        let result = input.dividend / input.divisor;
        Ok(serde_json::to_value(CalculationResult {
            result,
            expression: format!("{} ÷ {} = {}", input.dividend, input.divisor, result),
        })?)
    }
    
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(DivideArgs);
        Some(pmcp::types::ToolInfo::new(
            "divide",
            Some("Divide two numbers (divisor cannot be zero)".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}
```

## What Claude Sees

When Claude connects to your server, it receives the tool list:

```json
{
  "tools": [
    {
      "name": "add",
      "description": "Add two numbers",
      "inputSchema": {
        "type": "object",
        "properties": {
          "a": { "type": "number", "description": "First number" },
          "b": { "type": "number", "description": "Second number" }
        },
        "required": ["a", "b"]
      }
    },
    {
      "name": "divide",
      "description": "Divide two numbers (divisor cannot be zero)",
      "inputSchema": {
        "type": "object",
        "properties": {
          "dividend": { "type": "number", "description": "The dividend" },
          "divisor": { "type": "number", "description": "The divisor (cannot be zero)" }
        },
        "required": ["dividend", "divisor"]
      }
    }
  ]
}
```

Claude uses this information to:
1. Understand what tools are available
2. Know what arguments each tool requires
3. Generate valid tool calls automatically

## Hands-On Exercise

Ready to build your own calculator? Head to the exercises page:

**[Chapter 2 Exercises](./ch02-exercises.md)** - Build a calculator MCP server with proper error handling (Exercise 2)

---

*Next, let's dive deeper into the patterns and conventions used in this code.*

*Continue to [Understanding the Generated Code](./ch02-04-code-walkthrough.md) →*
