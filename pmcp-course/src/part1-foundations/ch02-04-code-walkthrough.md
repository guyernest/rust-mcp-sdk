# Understanding the Generated Code

Now that you've seen the calculator server, let's understand the patterns and conventions that make PMCP code production-ready.

## The Prelude Pattern

Most PMCP code starts with:

```rust
use pmcp::prelude::*;
```

This imports commonly used types:

| Type | Purpose |
|------|---------|
| `Server` | The MCP server instance |
| `ServerBuilder` | Fluent API for building servers |
| `ServerCapabilities` | Declares what the server supports |
| `ToolHandler` | Trait for implementing tools |
| `RequestHandlerExtra` | Additional context for handlers |
| `Error` | PMCP error types |

You can also import types explicitly:

```rust
use pmcp::{Server, ServerBuilder, ServerCapabilities, ToolHandler, Error};
```

## Server Builder Pattern

The `ServerBuilder` uses the builder pattern for flexible configuration:

```rust
let server = Server::builder()
    .name("my-server")           // Required: server name
    .version("1.0.0")            // Required: semantic version
    .capabilities(caps)          // Required: what the server supports
    .tool("tool_name", handler)  // Add tools
    .resource("uri", provider)   // Add resources
    .prompt("name", template)    // Add prompts
    .build()?;                   // Finalize and validate
```

### Server Capabilities

Capabilities tell clients what your server supports:

```rust
// Only tools
let caps = ServerCapabilities::tools_only();

// Only resources
let caps = ServerCapabilities::resources_only();

// Tools and resources
let caps = ServerCapabilities {
    tools: Some(pmcp::types::ToolCapabilities::default()),
    resources: Some(pmcp::types::ResourceCapabilities::default()),
    ..Default::default()
};

// Everything
let caps = ServerCapabilities::all();
```

Declaring capabilities correctly helps clients understand your server's features.

## The ToolHandler Trait

Every tool implements `ToolHandler`:

```rust
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Handle a tool invocation
    async fn handle(
        &self,
        args: Value,
        extra: RequestHandlerExtra,
    ) -> Result<Value, Error>;
    
    /// Return tool metadata (name, description, schema)
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        None  // Default: no metadata
    }
}
```

### Why `async_trait`?

Rust doesn't natively support async functions in traits (yet). The `#[async_trait]` macro bridges this gap:

```rust
use async_trait::async_trait;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        // Can use .await here
        let data = fetch_data().await?;
        Ok(json!({ "data": data }))
    }
}
```

### The `RequestHandlerExtra` Parameter

The `extra` parameter provides context about the request:

```rust
async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value, Error> {
    // Access request metadata
    if let Some(meta) = &extra.meta {
        tracing::info!("Request ID: {:?}", meta.progress_token);
    }
    
    // ... handle request
}
```

We'll use this more in later chapters for authentication and progress reporting.

## Type-Safe Arguments with Serde

The pattern for parsing arguments:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MyToolArgs {
    pub required_field: String,
    
    #[serde(default)]
    pub optional_field: Option<i32>,
    
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 { 10 }
```

### Serde Attributes

| Attribute | Effect |
|-----------|--------|
| `#[serde(default)]` | Use `Default::default()` if missing |
| `#[serde(default = "fn")]` | Use custom default function |
| `#[serde(rename = "name")]` | Use different JSON field name |
| `#[serde(skip)]` | Don't serialize/deserialize |
| `#[serde(flatten)]` | Inline nested struct fields |

### Parsing Pattern

Always parse with proper error handling:

```rust
let input: MyToolArgs = serde_json::from_value(args)
    .map_err(|e| Error::validation(format!("Invalid arguments: {}", e)))?;
```

This converts parsing errors into MCP validation errors that clients understand.

## JSON Schema Generation

The `JsonSchema` derive generates schemas automatically:

```rust
use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct SearchArgs {
    /// The search query string
    pub query: String,
    
    /// Maximum results to return (1-100)
    #[schemars(range(min = 1, max = 100))]
    pub limit: u32,
    
    /// Filter by status
    pub status: Option<Status>,
}

#[derive(JsonSchema)]
pub enum Status {
    Active,
    Inactive,
    Pending,
}
```

Generated schema:

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query string"
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100,
      "description": "Maximum results to return (1-100)"
    },
    "status": {
      "type": "string",
      "enum": ["Active", "Inactive", "Pending"],
      "description": "Filter by status"
    }
  },
  "required": ["query", "limit"]
}
```

### Schema Attributes

| Attribute | Effect |
|-----------|--------|
| `/// comment` | Becomes `description` |
| `#[schemars(range(min, max))]` | Adds numeric bounds |
| `#[schemars(length(min, max))]` | Adds string length bounds |
| `#[schemars(regex(pattern))]` | Adds pattern validation |

## Error Handling Patterns

### Validation Errors (Client's Fault)

```rust
// Missing required field
if input.query.is_empty() {
    return Err(Error::validation("Query cannot be empty"));
}

// Invalid value
if input.limit > 100 {
    return Err(Error::validation("Limit cannot exceed 100"));
}

// Invalid format
if !input.email.contains('@') {
    return Err(Error::validation("Invalid email format"));
}
```

### Internal Errors (Server's Fault)

```rust
// Database failure
let result = db.query(&sql).await
    .map_err(|e| Error::internal(format!("Database error: {}", e)))?;

// External service failure
let response = client.get(url).await
    .map_err(|e| Error::internal(format!("API error: {}", e)))?;
```

### Resource Errors

```rust
// Not found
let user = db.find_user(id).await?
    .ok_or_else(|| Error::not_found(format!("User {} not found", id)))?;

// Permission denied
if !user.can_access(resource) {
    return Err(Error::permission_denied("Access denied"));
}
```

## Structured Logging with Tracing

PMCP uses the `tracing` crate for structured logging:

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self, extra))]
async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value, Error> {
    info!(tool = "my_tool", "Processing request");
    
    let input: MyArgs = serde_json::from_value(args)?;
    debug!(query = %input.query, "Parsed arguments");
    
    match do_work(&input).await {
        Ok(result) => {
            info!(result_count = result.len(), "Request completed");
            Ok(serde_json::to_value(result)?)
        }
        Err(e) => {
            error!(error = %e, "Request failed");
            Err(Error::internal(e.to_string()))
        }
    }
}
```

### Log Levels

| Level | Use For |
|-------|---------|
| `error!` | Failures that need attention |
| `warn!` | Unexpected but handled situations |
| `info!` | Normal operational messages |
| `debug!` | Detailed debugging info |
| `trace!` | Very verbose debugging |

### The `#[instrument]` Macro

Automatically creates a span with function arguments:

```rust
#[instrument(skip(db), fields(user_id = %user_id))]
async fn get_user(db: &Database, user_id: i64) -> Result<User, Error> {
    // Logs: get_user{user_id=123}
    db.find(user_id).await
}
```

## Async Patterns

### Sequential Operations

```rust
let user = db.get_user(user_id).await?;
let orders = db.get_orders(user_id).await?;
let total = calculate_total(&orders);
```

### Parallel Operations

```rust
use tokio::try_join;

let (user, orders, preferences) = try_join!(
    db.get_user(user_id),
    db.get_orders(user_id),
    db.get_preferences(user_id),
)?;
```

### Timeout Handling

```rust
use tokio::time::{timeout, Duration};

let result = timeout(Duration::from_secs(5), slow_operation())
    .await
    .map_err(|_| Error::internal("Operation timed out"))??;
```

## Testing Tools

### Unit Testing a Handler

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_add_tool() {
        let tool = AddTool;
        let args = json!({ "a": 10.0, "b": 5.0 });
        let extra = RequestHandlerExtra::default();
        
        let result = tool.handle(args, extra).await.unwrap();
        
        assert_eq!(result["result"], 15.0);
        assert_eq!(result["expression"], "10 + 5 = 15");
    }
    
    #[tokio::test]
    async fn test_divide_by_zero() {
        let tool = DivideTool;
        let args = json!({ "dividend": 10.0, "divisor": 0.0 });
        let extra = RequestHandlerExtra::default();
        
        let result = tool.handle(args, extra).await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("divide by zero"));
    }
}
```

### Testing Schema Generation

```rust
#[test]
fn test_args_schema() {
    let schema = schemars::schema_for!(AddArgs);
    let json = serde_json::to_value(&schema).unwrap();
    
    assert!(json["properties"]["a"].is_object());
    assert!(json["properties"]["b"].is_object());
    assert!(json["required"].as_array().unwrap().contains(&json!("a")));
}
```

## Summary: The PMCP Pattern

Every PMCP tool follows this pattern:

1. **Define input types** with `Deserialize` and `JsonSchema`
2. **Define output types** with `Serialize` and `JsonSchema`
3. **Implement `ToolHandler`** with proper error handling
4. **Provide metadata** for client discovery
5. **Register with `ServerBuilder`**
6. **Test thoroughly**

```rust
// 1. Input type
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MyToolArgs { /* ... */ }

// 2. Output type  
#[derive(Debug, Serialize, JsonSchema)]
pub struct MyToolResult { /* ... */ }

// 3. Handler implementation
pub struct MyTool;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let input: MyToolArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(e.to_string()))?;
        
        // Business logic here
        
        Ok(serde_json::to_value(result)?)
    }
    
    // 4. Metadata
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let schema = schemars::schema_for!(MyToolArgs);
        Some(pmcp::types::ToolInfo::new(
            "my_tool",
            Some("Description here".to_string()),
            serde_json::to_value(&schema).unwrap_or_default(),
        ))
    }
}

// 5. Registration
let server = Server::builder()
    .tool("my_tool", MyTool)
    .build()?;
```

## Hands-On Exercise: Code Review

Now that you understand the patterns, practice your code review skills with a hands-on exercise. Code review is critical when working with AI-generated code.

**[Chapter 2 Exercises](./ch02-exercises.md)** - Complete Exercise 3: Code Review Basics to practice identifying bugs, security issues, and anti-patterns in MCP server code.

---

*Next, let's learn how to debug and test your server with MCP Inspector.*

*Continue to [Testing with MCP Inspector](./ch02-05-inspector.md) →*
