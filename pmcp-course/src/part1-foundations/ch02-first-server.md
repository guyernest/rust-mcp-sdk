# Your First Production Server

In this chapter, you'll build your first MCP server using the PMCP SDK. Unlike typical tutorials, we'll build it **production-ready from the start**.

## What We'll Build

A calculator MCP server that:
- Has properly typed inputs and outputs
- Validates all inputs
- Returns structured, typed responses
- Includes appropriate error handling
- Is ready for cloud deployment

This isn't about math—it's about learning the patterns you'll use for every enterprise server.

## Creating Your Workspace

```bash
cargo pmcp new enterprise-mcp --tier foundation
cd enterprise-mcp
```

The `--tier foundation` flag indicates this is a "foundation" server—one that provides data access rather than orchestration.

This creates:

```
enterprise-mcp/
├── Cargo.toml              # Workspace manifest
├── pmcp.toml               # PMCP configuration
├── server-common/          # Shared HTTP bootstrap code
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── servers/                # Your MCP servers go here
```

## Adding Your First Server

```bash
cargo pmcp add server calculator --template calculator
```

This creates:

```
servers/calculator/
├── Cargo.toml
└── src/
    ├── main.rs             # Entry point
    └── tools/
        ├── mod.rs          # Tool definitions
        └── calculator.rs   # Calculator implementation
```

## Understanding the Generated Code

### Entry Point (`main.rs`)

```rust
use pmcp::prelude::*;
use server_common::create_http_server;

mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Build the MCP server
    let server = ServerBuilder::new("calculator", "1.0.0")
        .with_description("Enterprise calculator with validated inputs")
        .with_tool(tools::Add)
        .with_tool(tools::Subtract)
        .with_tool(tools::Multiply)
        .with_tool(tools::Divide)
        .build()?;

    // Start HTTP server
    let addr = "0.0.0.0:3000";
    tracing::info!("Starting calculator server on {}", addr);

    create_http_server(server)
        .serve(addr)
        .await
}
```

**Key points:**
- `ServerBuilder` provides a fluent API for configuration
- Tools are registered with `.with_tool()`
- Logging is configured from the start
- HTTP serving is handled by `server-common`

### Tool Definition (`tools/calculator.rs`)

```rust
use pmcp::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input for the add operation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddInput {
    /// First number to add
    a: f64,
    /// Second number to add
    b: f64,
}

/// Output from the add operation
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddOutput {
    /// The sum of a and b
    result: f64,
    /// Human-readable description
    description: String,
}

/// Add two numbers together
#[derive(TypedTool)]
#[tool(
    name = "add",
    description = "Add two numbers and return the sum",
    annotations(read_only = true, idempotent = true)
)]
pub struct Add;

impl Add {
    pub async fn run(&self, input: AddInput) -> Result<AddOutput> {
        Ok(AddOutput {
            result: input.a + input.b,
            description: format!("{} + {} = {}", input.a, input.b, input.a + input.b),
        })
    }
}
```

**Key points:**
- **Typed inputs and outputs**: `AddInput` and `AddOutput` use serde for serialization
- **JSON Schema**: `JsonSchema` derive generates validation schemas automatically
- **Tool annotations**: `read_only` and `idempotent` hints help AI clients
- **Async by default**: All tool handlers are async for consistency

### Division with Error Handling

```rust
/// Input for the divide operation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DivideInput {
    /// Dividend (number to be divided)
    dividend: f64,
    /// Divisor (number to divide by)
    divisor: f64,
}

/// Divide two numbers with proper error handling
#[derive(TypedTool)]
#[tool(
    name = "divide",
    description = "Divide two numbers. Returns an error if divisor is zero.",
    annotations(read_only = true)
)]
pub struct Divide;

impl Divide {
    pub async fn run(&self, input: DivideInput) -> Result<AddOutput> {
        // Validate: prevent division by zero
        if input.divisor == 0.0 {
            return Err(PmcpError::invalid_params(
                "Cannot divide by zero"
            ));
        }

        let result = input.dividend / input.divisor;

        Ok(AddOutput {
            result,
            description: format!(
                "{} ÷ {} = {}",
                input.dividend,
                input.divisor,
                result
            ),
        })
    }
}
```

**Key points:**
- **Input validation**: Check for invalid inputs before processing
- **Proper errors**: Use `PmcpError::invalid_params()` for client errors
- **Descriptive messages**: Help users understand what went wrong

## Running Locally

Start the server:

```bash
cargo run --package calculator-server
```

You should see:

```
2024-01-15T10:30:00.000Z  INFO Starting calculator server on 0.0.0.0:3000
```

## Testing with MCP Inspector

In another terminal:

```bash
npx @anthropic-ai/mcp-inspector http://localhost:3000
```

This opens a web UI where you can:
1. See available tools and their schemas
2. Call tools with different inputs
3. View the JSON-RPC messages

Try calling the `add` tool:

```json
{
  "a": 10,
  "b": 5
}
```

You should get:

```json
{
  "result": 15,
  "description": "10 + 5 = 15"
}
```

Try dividing by zero:

```json
{
  "dividend": 10,
  "divisor": 0
}
```

You should get a proper error:

```json
{
  "error": {
    "code": -32602,
    "message": "Cannot divide by zero"
  }
}
```

## Connecting Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "calculator": {
      "url": "http://localhost:3000",
      "transport": "streamable-http"
    }
  }
}
```

Restart Claude Desktop. You should see the calculator tools available.

Try asking Claude:
- "What is 1234 + 5678?"
- "Calculate 100 divided by 7"
- "What's 15% of 250?" (Claude will use multiply)

## What Makes This Production-Ready?

Even this simple example includes:

| Feature | Implementation |
|---------|---------------|
| Type safety | Rust's type system prevents runtime errors |
| Input validation | `JsonSchema` generates validation rules |
| Error handling | Proper error types and messages |
| Logging | `tracing` for structured logs |
| Annotations | Hints for AI clients |
| HTTP server | Production-ready with `server-common` |

Compare this to a typical Python tutorial that just `print()`s results.

## Exercises

1. **Add a power tool**: Implement `power(base, exponent)` with validation for edge cases

2. **Add a percentage tool**: Implement `percentage(value, percent)` that calculates percentages

3. **Add input ranges**: Modify `DivideInput` to reject very small divisors (e.g., < 0.0001)

4. **Add tool grouping**: Create a `Statistics` tool that calculates mean, median, and mode

## Next Steps

In the next chapter, we'll dive deeper into the generated code and understand:
- How the server-common bootstrap works
- How tools are registered and discovered
- How JSON-RPC requests are routed

---

*Continue to [Development Environment Setup](./ch02-01-setup.md) →*
