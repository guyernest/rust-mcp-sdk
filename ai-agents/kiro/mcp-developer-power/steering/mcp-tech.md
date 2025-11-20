---
inclusion: always
---

# MCP Technology Stack

## Core Technologies

### pmcp SDK (Rust)

**Current Version**: 1.8.3+ (always use latest from crates.io)

**Key Characteristics**:
- **Performance**: 16x faster than TypeScript SDK
- **Memory**: 50x lower memory usage (~2MB per server)
- **Safety**: Zero unwraps in production code
- **Quality**: 80%+ test coverage, zero clippy warnings

**Installation**:
```toml
[dependencies]
pmcp = "1.8"  # Uses latest 1.8.x version
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"  # For JSON schema generation
anyhow = "1"  # For error handling context
```

### Essential Crates

#### pmcp (Core SDK)
```toml
pmcp = "1.8"
```

**Provides**:
- `Server` builder and runtime
- `TypedTool` for type-safe tools
- Transport implementations (stdio, HTTP, WebSocket)
- Error types and result handling
- Protocol types (requests, responses, notifications)

#### pmcp-macros (Procedural Macros)
```toml
pmcp-macros = "0.2"  # Optional but recommended
```

**Provides**:
- `#[pmcp::server]` - Server implementation macro
- `#[pmcp::tool]` - Tool definition macro
- `#[pmcp::resource]` - Resource definition macro

**Note**: Macros are optional. You can use the builder API directly.

#### Tokio (Async Runtime)
```toml
tokio = { version = "1", features = ["full"] }
```

**Required Features**:
- `macros` - For `#[tokio::main]`
- `rt-multi-thread` - Multi-threaded runtime
- `io-util` - I/O utilities
- `sync` - Synchronization primitives

#### Serde (Serialization)
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Usage**: All tool inputs/outputs must be `Serialize + Deserialize`

#### Schemars (JSON Schema)
```toml
schemars = "0.8"
```

**Usage**: Tool inputs must derive `JsonSchema` for auto-generated schemas

#### Anyhow (Error Handling)
```toml
anyhow = "1"
```

**Usage**: Use `.context("msg")` to add error context

### Optional But Recommended

#### reqwest (HTTP Client)
```toml
reqwest = { version = "0.11", features = ["json"] }
```

**Use For**: External API calls, HTTP requests

#### sqlx (Database)
```toml
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }
```

**Use For**: Database access in resource-heavy servers

#### tracing (Logging)
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Use For**: Structured logging and observability

## Transport Patterns

### HTTP Transport (Development)

**Use Case**: Development with hot-reload, easy testing

**Advantages**:
- Easy to test with curl/Postman
- Browser-based MCP Inspector support
- Hot-reload friendly
- CORS support for web clients

**Configuration**:
```rust
use pmcp::Server;
use pmcp::transport::http::StreamableHttpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = build_my_server()?;

    let http_server = StreamableHttpServer::new(server)
        .bind("0.0.0.0:3000")?;

    println!("MCP server running on http://0.0.0.0:3000");
    http_server.run().await
}
```

**cargo-pmcp Integration**:
```bash
cargo pmcp dev --server myserver --port 3000
```

**MCP Client Configuration** (for testing):
```json
{
  "mcpServers": {
    "myserver": {
      "command": "node",
      "args": ["/path/to/http-sse-mcp-proxy.js"],
      "env": {
        "MCP_SERVER_URL": "http://0.0.0.0:3000"
      }
    }
  }
}
```

### stdio Transport (Production)

**Use Case**: Production deployment, Claude Code/Kiro integration

**Advantages**:
- Standard MCP transport
- Works with all MCP clients
- Secure (no network exposure)
- Simple deployment

**Configuration**:
```rust
use pmcp::Server;
use pmcp::transport::StdioTransport;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = build_my_server()?;

    let transport = StdioTransport::new();
    transport.run(server).await
}
```

**MCP Client Configuration**:
```json
{
  "mcpServers": {
    "myserver": {
      "command": "/path/to/myserver-server",
      "args": []
    }
  }
}
```

### WebSocket Transport (Advanced)

**Use Case**: Browser clients, real-time updates, persistent connections

**Configuration**:
```rust
use pmcp::transport::websocket::WebSocketServer;

let ws_server = WebSocketServer::new(server)
    .bind("0.0.0.0:8080")?;

ws_server.run().await
```

## Async Patterns

### All Handlers Are Async

MCP tool handlers must return `Future`:

```rust
use pmcp::{Result, TypedTool};

// Async handler
async fn my_tool_handler(input: MyInput, extra: RequestHandlerExtra) -> Result<MyOutput> {
    // Can await async operations
    let data = fetch_from_api().await?;
    let processed = process_data(data).await?;

    Ok(MyOutput { result: processed })
}

// Register with TypedTool
let tool = TypedTool::new("my_tool", |input, extra| {
    Box::pin(my_tool_handler(input, extra))
});
```

### Async Best Practices

#### Use `tokio::spawn` for Parallelism
```rust
async fn fetch_multiple(urls: Vec<String>) -> Result<Vec<Response>> {
    let mut tasks = vec![];

    for url in urls {
        tasks.push(tokio::spawn(async move {
            reqwest::get(&url).await
        }));
    }

    // Wait for all tasks
    let results = futures::future::try_join_all(tasks).await?;
    Ok(results)
}
```

#### Use `tokio::select!` for Timeouts
```rust
use tokio::time::{sleep, Duration};

async fn fetch_with_timeout(url: &str) -> Result<Response> {
    tokio::select! {
        result = reqwest::get(url) => {
            result.context("Request failed")
        }
        _ = sleep(Duration::from_secs(10)) => {
            Err(Error::validation("Request timeout"))
        }
    }
}
```

#### Use Channels for Background Tasks
```rust
use tokio::sync::mpsc;

async fn background_processor() {
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn processor
    tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            process_item(item).await;
        }
    });

    // Send items
    tx.send(item).await.unwrap();
}
```

## Error Handling Philosophy

### Zero Tolerance for Unwraps

**Never use** in production code:
- `unwrap()`
- `expect()` (except in tests)
- `panic!()`

**Always use**:
- `?` operator for error propagation
- `ok_or_else()` for Option → Result
- `.context()` for error context

### pmcp Error Types

```rust
use pmcp::Error;

// Validation errors (4xx - client's fault)
Error::validation("Invalid email format")
Error::validation(format!("City '{}' not found", city))

// Internal errors (5xx - server's fault)
Error::internal("Database connection failed")
Error::internal(format!("Failed to parse config: {}", err))

// Protocol errors (MCP protocol violations)
Error::protocol("Invalid request format")
```

### Error Context Pattern

```rust
use anyhow::Context;
use pmcp::Result;

async fn fetch_user_data(user_id: u64) -> Result<UserData> {
    let response = reqwest::get(&format!("https://api.example.com/users/{}", user_id))
        .await
        .context("Failed to connect to user API")?;

    if !response.status().is_success() {
        return Err(Error::validation(
            format!("User {} not found", user_id)
        ));
    }

    let user_data = response
        .json::<UserData>()
        .await
        .context("Failed to parse user data")?;

    Ok(user_data)
}
```

### Error Handling in Tools

```rust
async fn my_tool(input: MyInput, _extra: RequestHandlerExtra) -> Result<MyOutput> {
    // 1. Validate inputs
    if input.email.is_empty() {
        return Err(Error::validation("Email cannot be empty"));
    }

    if !input.email.contains('@') {
        return Err(Error::validation(format!(
            "Invalid email format: '{}'", input.email
        )));
    }

    // 2. Perform operations with context
    let user = db.find_user(&input.email)
        .await
        .context("Database query failed")?
        .ok_or_else(|| Error::validation(format!(
            "User '{}' not found", input.email
        )))?;

    // 3. Return success
    Ok(MyOutput { user_id: user.id })
}
```

## Type Safety Patterns

### Input/Output Types

**Always** define explicit types for tool inputs and outputs:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Tool input - must derive these traits
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]  // Reject unknown fields
pub struct CalculateInput {
    /// The operation to perform
    #[schemars(description = "Operation: add, subtract, multiply, divide")]
    pub operation: String,

    /// First operand
    #[schemars(description = "The first number")]
    pub a: f64,

    /// Second operand
    #[schemars(description = "The second number")]
    pub b: f64,
}

// Tool output
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CalculateOutput {
    /// The result of the calculation
    pub result: f64,

    /// Human-readable description
    pub description: String,
}
```

### Enum-Based Input Validation

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CalculateInput {
    pub operation: Operation,  // Type-safe enum instead of String
    pub a: f64,
    pub b: f64,
}

async fn calculate(input: CalculateInput, _extra: RequestHandlerExtra) -> Result<f64> {
    let result = match input.operation {
        Operation::Add => input.a + input.b,
        Operation::Subtract => input.a - input.b,
        Operation::Multiply => input.a * input.b,
        Operation::Divide => {
            if input.b == 0.0 {
                return Err(Error::validation("Cannot divide by zero"));
            }
            input.a / input.b
        }
    };

    Ok(result)
}
```

### Using TypedTool for Type Safety

```rust
use pmcp::{Server, TypedTool};

fn build_server() -> pmcp::Result<Server> {
    Server::builder()
        .name("calculator")
        .version("1.0.0")
        .tool(
            "calculate",
            TypedTool::new("calculate", |input: CalculateInput, extra| {
                Box::pin(calculate(input, extra))
            })
            .with_description("Perform arithmetic operations")
        )
        .build()
}
```

## Server Builder Pattern

### Basic Server

```rust
use pmcp::{Server, ServerBuilder};
use pmcp::types::{ServerCapabilities, ToolCapabilities};

fn build_server() -> pmcp::Result<Server> {
    ServerBuilder::new()
        .name("myserver")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true)
            }),
            resources: Some(Default::default()),
            prompts: Some(Default::default()),
            ..Default::default()
        })
        .build()
}
```

### Adding Tools

```rust
use pmcp::TypedTool;

let server = ServerBuilder::new()
    .name("myserver")
    .version("1.0.0")
    .tool(
        "my_tool",
        TypedTool::new("my_tool", |input: MyInput, extra| {
            Box::pin(my_tool_handler(input, extra))
        })
        .with_description("Description of what this tool does")
    )
    .build()?;
```

### Adding Resources

```rust
use pmcp::resource::{Resource, ResourceContent};

let server = ServerBuilder::new()
    .name("myserver")
    .version("1.0.0")
    .resource_handler(|uri: &str| async move {
        // Dynamic resource discovery
        match uri {
            uri if uri.starts_with("myserver://data/") => {
                let data = fetch_data(uri).await?;
                Ok(ResourceContent::text(data))
            }
            _ => Err(Error::validation(format!("Unknown resource: {}", uri)))
        }
    })
    .build()?;
```

### Adding Prompts

```rust
use pmcp::prompt::{Prompt, PromptMessage};

let server = ServerBuilder::new()
    .name("myserver")
    .version("1.0.0")
    .prompt("code_review", |args| async move {
        let code = args.get("code")
            .ok_or(Error::validation("Missing 'code' argument"))?;

        Ok(vec![
            PromptMessage::user(format!(
                "Please review this code for:\n\
                 - Security vulnerabilities\n\
                 - Performance issues\n\
                 - Best practices\n\n\
                 Code:\n```\n{}\n```", code
            ))
        ])
    })
    .build()?;
```

## Authentication Patterns (pmcp 1.8.0+)

### OAuth Auth Context Pass-Through

```rust
use pmcp::{RequestHandlerExtra, Result};

async fn authenticated_tool(
    input: MyInput,
    extra: RequestHandlerExtra
) -> Result<MyOutput> {
    // Extract OAuth token from metadata
    let token = extra.metadata
        .get("oauth_token")
        .ok_or(Error::validation("Missing authentication token"))?;

    // Use token for API calls
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.example.com/data")
        .bearer_auth(token)
        .send()
        .await
        .context("API request failed")?;

    // Process response
    let data = response.json().await?;
    Ok(data)
}
```

### HTTP Transport with OAuth

```rust
use pmcp::transport::http::{StreamableHttpServer, AuthProvider};

// Implement custom auth provider
struct MyAuthProvider;

impl AuthProvider for MyAuthProvider {
    async fn validate_request(&self, auth_header: Option<&str>) -> Result<AuthContext> {
        let header = auth_header
            .ok_or(Error::validation("Missing Authorization header"))?;

        let token = header.strip_prefix("Bearer ")
            .ok_or(Error::validation("Invalid Authorization header format"))?;

        // Validate token (call OAuth provider, check signature, etc.)
        validate_oauth_token(token).await?;

        Ok(AuthContext {
            token: token.to_string(),
            // ... other context
        })
    }
}

// Use in HTTP server
let http_server = StreamableHttpServer::new(server)
    .with_auth_provider(Arc::new(MyAuthProvider))
    .bind("0.0.0.0:3000")?;
```

## Testing Patterns

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_positive_numbers() {
        let input = CalculateInput {
            operation: Operation::Add,
            a: 5.0,
            b: 3.0,
        };

        let extra = RequestHandlerExtra::default();
        let result = calculate(input, extra).await.unwrap();

        assert_eq!(result, 8.0);
    }

    #[tokio::test]
    async fn test_divide_by_zero_error() {
        let input = CalculateInput {
            operation: Operation::Divide,
            a: 10.0,
            b: 0.0,
        };

        let extra = RequestHandlerExtra::default();
        let result = calculate(input, extra).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("divide by zero"));
    }
}
```

### Integration Tests with mcp-tester

```yaml
# scenarios/myserver/basic.yaml
name: "Basic Calculator Tests"
description: "Test arithmetic operations"
timeout: 60
stop_on_failure: false

steps:
  - name: "Test addition"
    operation:
      type: tool_call
      tool: "calculate"
      arguments:
        operation: "add"
        a: 10
        b: 5
    assertions:
      - type: success
      - type: equals
        path: "result"
        value: 15
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_addition_commutative(a: f64, b: f64) {
        let result1 = calculate_add(a, b);
        let result2 = calculate_add(b, a);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_division_by_nonzero(a: f64, b in 1.0..1000.0) {
        let result = calculate_divide(a, b);
        assert!(result.is_ok());
    }
}
```

## Performance Best Practices

### Use Appropriate Data Structures

```rust
use std::collections::HashMap;

// Fast lookups
let mut cache: HashMap<String, Value> = HashMap::new();

// Ordered iteration
use std::collections::BTreeMap;
let ordered: BTreeMap<String, Value> = BTreeMap::new();
```

### Connection Pooling for Databases

```rust
use sqlx::SqlitePool;

// Create pool once, reuse across requests
lazy_static! {
    static ref DB_POOL: SqlitePool = SqlitePool::connect("sqlite://db.sqlite")
        .await
        .expect("Failed to create pool");
}
```

### Lazy Static for Expensive Initialization

```rust
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref EMAIL_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();
}

fn validate_email(email: &str) -> bool {
    EMAIL_REGEX.is_match(email)
}
```

## Common Patterns Summary

### Error Handling
- ✅ Use `Result<T>` for fallible operations
- ✅ Use `.context()` to add error messages
- ✅ Use `Error::validation()` for client errors
- ✅ Use `Error::internal()` for server errors
- ❌ Never use `unwrap()` or `panic!()`

### Type Safety
- ✅ Define explicit input/output structs
- ✅ Derive `JsonSchema` for auto-generated schemas
- ✅ Use enums for constrained values
- ✅ Use `#[schemars(description = "...")]` for documentation

### Async Patterns
- ✅ All tool handlers are async
- ✅ Use `tokio::spawn` for parallelism
- ✅ Use `tokio::select!` for timeouts
- ✅ Use channels for background tasks

### Testing
- ✅ Unit tests for all functions
- ✅ Integration tests with mcp-tester
- ✅ Property tests for invariants
- ✅ 80%+ test coverage

### Performance
- ✅ Connection pooling for databases
- ✅ Lazy static for expensive initialization
- ✅ Appropriate data structures
- ✅ Async for I/O-bound operations

---

**Next**: Read `mcp-structure.md` for project organization patterns.
