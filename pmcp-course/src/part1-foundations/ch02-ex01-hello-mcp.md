# Exercise: Your First MCP Server

::: exercise
id: ch02-01-hello-mcp
difficulty: beginner
time: 20 minutes
:::

Create your first MCP server - one that responds to a simple "greet" tool. This might seem simple, but you're learning the foundation that every production MCP server builds upon.

::: objectives
thinking:
  - How MCP servers are structured (builder pattern)
  - The relationship between server, tools, and responses
  - Why typed inputs matter for AI interactions

doing:
  - Create an MCP server using Server::builder()
  - Define a tool with typed input parameters
  - Return a properly formatted response
:::

::: discussion
- What do you think an MCP server does? How is it different from a REST API?
- Why might we want to define input types (schemas) for our tools?
- When Claude or another AI calls a tool, what information does it need?
:::

## Your Task

Complete the `greet` tool implementation below. The tool should:
- Take a `name` parameter (required)
- Take an optional `formal` parameter (boolean)
- Return "Hello, {name}!" or "Good day, {name}." based on the formal flag

::: starter file="src/main.rs" language=rust
```rust
//! Your First MCP Server
//!
//! This server provides a simple "greet" tool that returns a personalized
//! greeting. It demonstrates the fundamental patterns of PMCP development.

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::Deserialize;
use schemars::JsonSchema;
use anyhow::Result;

/// Input for the greet tool
#[derive(Deserialize, JsonSchema)]
struct GreetInput {
    /// The name of the person to greet
    name: String,

    // TODO: Add an optional "formal" field (bool) to control greeting style
    // Hint: Use Option<bool> for optional fields
}

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: Build the MCP server
    //
    // Steps:
    // 1. Create a server using Server::builder()
    // 2. Set the name to "hello-mcp"
    // 3. Set the version to "1.0.0"
    // 4. Configure capabilities to include tools
    // 5. Add a "greet" tool using .tool()
    // 6. Build the server
    //
    // The greet tool should:
    // - Take GreetInput as input
    // - Return "Hello, {name}!" or "Good day, {name}." based on formal flag

    todo!("Build your MCP server here")
}
```
:::

::: hint level=1 title="Starting the Builder"
```rust
let server = Server::builder()
    .name("hello-mcp")
    .version("1.0.0")
    // ...continue building
```
:::

::: hint level=2 title="Adding Capabilities and Tools"
```rust
.capabilities(ServerCapabilities {
    tools: Some(ToolCapabilities::default()),
    ..Default::default()
})
.tool("greet", TypedTool::new(...))
```
:::

::: hint level=3 title="Complete Structure"
```rust
let server = Server::builder()
    .name("hello-mcp")
    .version("1.0.0")
    .capabilities(ServerCapabilities {
        tools: Some(ToolCapabilities::default()),
        ..Default::default()
    })
    .tool("greet", TypedTool::new("greet", |input: GreetInput| {
        Box::pin(async move {
            let greeting = if input.formal.unwrap_or(false) {
                format!("Good day, {}.", input.name)
            } else {
                format!("Hello, {}!", input.name)
            };
            Ok(serde_json::json!({ "message": greeting }))
        })
    }))
    .build()?;
```
:::

::: solution
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::Deserialize;
use schemars::JsonSchema;
use anyhow::Result;

#[derive(Deserialize, JsonSchema)]
struct GreetInput {
    /// The name of the person to greet
    name: String,
    /// Whether to use a formal greeting style
    formal: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("hello-mcp")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("greet", TypedTool::new("greet", |input: GreetInput| {
            Box::pin(async move {
                let greeting = if input.formal.unwrap_or(false) {
                    format!("Good day, {}.", input.name)
                } else {
                    format!("Hello, {}!", input.name)
                };
                Ok(serde_json::json!({ "message": greeting }))
            })
        }))
        .build()?;

    println!("Server '{}' v{} ready!", server.name(), server.version());
    Ok(())
}
```

### Explanation

- **`#[derive(Deserialize)]`** - Parses JSON input from clients
- **`#[derive(JsonSchema)]`** - Generates schema for AI to understand inputs
- **`Server::builder()`** - Fluent API for server configuration
- **`TypedTool::new`** - Wraps your handler with type information
- **`Box::pin(async move { ... })`** - Creates an async future for the handler
:::

::: reflection
- Why do we use a struct with derive macros instead of parsing JSON manually?
- What happens if a client sends an input that doesn't match the schema?
- How would you extend this server to greet in different languages?
:::
