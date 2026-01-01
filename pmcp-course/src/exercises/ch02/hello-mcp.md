::: exercise
id: ch02-01-hello-mcp
difficulty: beginner
time: 20 minutes
:::

Every journey starts with a first step. In this exercise, you'll create
your first MCP server - one that responds to a simple "greet" tool.

This might seem simple, but you're learning the foundation that every
production MCP server builds upon. By the end, you'll understand:
- How MCP servers are structured
- How tools receive and process input
- How to return results to clients

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

::: starter file="src/main.rs"
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
///
/// The #[derive] macros automatically:
/// - Deserialize: Parse JSON input from clients
/// - JsonSchema: Generate schema for AI to understand the inputs
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
    // - Default to informal greeting if formal is not specified

    todo!("Build your MCP server here")
}
```
:::

::: hint level=1 title="Start with the builder"
Start with the server builder:
```rust
let server = Server::builder()
    .name("hello-mcp")
    .version("1.0.0")
    // ...continue building
```
:::

::: hint level=2 title="Configure capabilities"
You need to configure capabilities and add a tool:
```rust
.capabilities(ServerCapabilities {
    tools: Some(ToolCapabilities::default()),
    ..Default::default()
})
.tool("greet", TypedTool::new(...))
```
:::

::: hint level=3 title="Complete structure"
The complete structure looks like:
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
            // Your greeting logic here
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

    // In a real server, you'd run this with a transport
    // For now, we just verify it builds
    println!("Server '{}' v{} ready!", server.name(), server.version());

    Ok(())
}
```

### Explanation

Let's break down what this code does:

**1. Input Definition (GreetInput)**
- `#[derive(Deserialize)]` - Allows parsing JSON input from clients
- `#[derive(JsonSchema)]` - Generates a schema that tells AI what inputs are valid
- `Option<bool>` - Makes the `formal` field optional

**2. Server Builder Pattern**
- `Server::builder()` - Starts building a server configuration
- `.name()` / `.version()` - Metadata that identifies your server
- `.capabilities()` - Declares what the server can do (tools, resources, etc.)
- `.tool()` - Registers a tool that clients can call

**3. TypedTool**
- Wraps your handler function with type information
- Automatically deserializes JSON input to your struct
- The closure receives typed input and returns a JSON result

**4. Async Handler**
- `Box::pin(async move { ... })` - Creates an async future
- Returns `Result<Value>` - Either a JSON response or an error

**Why This Pattern?**
- Type safety catches errors at compile time
- Schemas help AI understand how to call your tools
- The builder pattern makes configuration clear and extensible
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_informal_greeting() {
        let input = GreetInput {
            name: "Alice".to_string(),
            formal: None,
        };
        let result = create_greeting(&input);
        assert!(result.contains("Hello"));
        assert!(result.contains("Alice"));
    }

    #[test]
    fn test_formal_greeting() {
        let input = GreetInput {
            name: "Dr. Smith".to_string(),
            formal: Some(true),
        };
        let result = create_greeting(&input);
        assert!(result.contains("Good day"));
        assert!(result.contains("Dr. Smith"));
    }

    #[test]
    fn test_explicit_informal() {
        let input = GreetInput {
            name: "Bob".to_string(),
            formal: Some(false),
        };
        let result = create_greeting(&input);
        assert!(result.contains("Hello"));
    }

    fn create_greeting(input: &GreetInput) -> String {
        if input.formal.unwrap_or(false) {
            format!("Good day, {}.", input.name)
        } else {
            format!("Hello, {}!", input.name)
        }
    }
}
```
:::

::: reflection
- Why do we use a struct with derive macros instead of just parsing JSON manually?
- What happens if a client sends an input that doesn't match the schema?
- How might you extend this server to greet in different languages?
- What would change if you wanted to add a second tool to this server?
:::

## Related Examples

For more patterns and variations, explore these SDK examples:

- **[02_server_basic.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/02_server_basic.rs)** - Basic server with calculator tool using `ToolHandler` trait
- **[32_typed_tools.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/32_typed_tools.rs)** - Type-safe tools with automatic schema generation using `TypedTool`

Run locally with:
```bash
cargo run --example 02_server_basic
cargo run --example 32_typed_tools --features schema-generation
```
