# MCP Macros Guide

**Covers:** `#[mcp_tool]`, `#[mcp_prompt]`, `#[mcp_server]`, `State<T>`, `TypedPrompt`
**Shipped:** Phases 58 + 59

## Setup

Add `macros` to your pmcp features (or use `full` which includes it):

```toml
[dependencies]
pmcp = { version = "2.0", features = ["macros"] }
```

Then import:

```rust
use pmcp::{mcp_tool, mcp_prompt, mcp_server};
```

No separate `pmcp-macros` dependency needed.

## Quick Start

### Tool (replaces TypedTool + Box::pin)

```rust
use pmcp::mcp_tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    result: f64,
}

#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { result: args.a + args.b })
}

// Register:
server_builder.tool("add", add())
```

### Prompt (replaces manual PromptHandler + HashMap parsing)

```rust
use pmcp::mcp_prompt;
use pmcp::types::{Content, GetPromptResult, PromptMessage};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    /// The programming language
    language: String,
    /// Number of issues to find
    max_issues: u32,        // auto-coerced from string "5" → u32
    /// Include security review
    security: bool,         // auto-coerced from string "true" → bool
}

#[mcp_prompt(description = "Review code for quality issues")]
async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Review this {} code. Find up to {} issues.{}",
            args.language,
            args.max_issues,
            if args.security { " Include security review." } else { "" }
        )))],
        None,
    ))
}

// Register:
server_builder.prompt("code_review", code_review())
```

### Server (mixed tools + prompts in one impl block)

```rust
use pmcp::{mcp_server, mcp_tool, mcp_prompt};

struct MyServer {
    db: Arc<Database>,
}

#[mcp_server]
impl MyServer {
    #[mcp_tool(description = "Query database")]
    async fn query(&self, args: QueryArgs) -> pmcp::Result<QueryResult> {
        self.db.execute(&args.sql).await
    }

    #[mcp_prompt(description = "Generate a SQL query")]
    async fn sql_prompt(&self, args: SqlArgs) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(
            vec![PromptMessage::user(Content::text(format!(
                "Write a SQL query for: {}", args.description
            )))],
            None,
        ))
    }
}

// Register all tools + prompts at once:
server_builder.mcp_server(MyServer { db })
```

## Prompt Argument Type Coercion

MCP sends prompt arguments as `HashMap<String, String>`. The SDK automatically coerces string values to native types before deserialization:

| String value | Coerced to | Works with |
|---|---|---|
| `"42"` | `Value::Number(42)` | `u32`, `i64`, `f64` |
| `"3.14"` | `Value::Number(3.14)` | `f64` |
| `"true"` / `"false"` | `Value::Bool` | `bool` |
| `"null"` | `Value::Null` | `Option<T>` |
| `"hello"` | `Value::String` | `String` |

This means you can use native Rust types in prompt arg structs:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct AnalysisArgs {
    topic: String,         // stays as string
    depth: u32,            // "3" → 3
    include_examples: bool, // "true" → true
    max_tokens: Option<u32>, // "1000" → Some(1000), absent → None
}
```

Tool args are `serde_json::Value` (already typed JSON), so they don't need coercion — native types always work in `#[mcp_tool]`.

## State Injection

### Standalone functions: `State<T>`

```rust
#[mcp_tool(description = "Query database")]
async fn query(args: QueryArgs, db: State<Database>) -> pmcp::Result<Value> {
    let rows = db.execute(&args.sql).await?;  // db auto-derefs to &Database
    Ok(json!({ "rows": rows }))
}

// Provide state at registration:
server_builder.tool("query", query().with_state(shared_db))
```

### Impl blocks: `&self`

```rust
struct MyServer { db: Arc<Database> }

#[mcp_server]
impl MyServer {
    #[mcp_tool(description = "Query")]
    async fn query(&self, args: QueryArgs) -> pmcp::Result<Value> {
        self.db.execute(&args.sql).await  // natural &self access
    }
}

server_builder.mcp_server(MyServer { db })
```

### Generic impl blocks (composition servers)

```rust
struct ArithmeticsServer<F: FoundationClient> {
    foundation: Arc<F>,
}

#[mcp_server]
impl<F: FoundationClient + 'static> ArithmeticsServer<F> {
    #[mcp_tool(description = "Calculate discriminant")]
    async fn discriminant(&self, args: DiscriminantInput) -> Result<DiscriminantResult> {
        calculate_discriminant(self.foundation.as_ref(), args).await
    }

    #[mcp_prompt(description = "Analyze quadratic equation")]
    async fn analyze(&self, args: QuadraticArgs) -> Result<GetPromptResult> {
        // &self.foundation available directly — zero Arc clones
        Ok(GetPromptResult::new(vec![...], None))
    }
}
```

## RequestHandlerExtra (opt-in)

Only declare `extra` in your signature if you need cancellation, progress, or auth:

```rust
#[mcp_tool(description = "Long-running export")]
async fn export(args: ExportArgs, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
    for (i, chunk) in data.chunks(100).enumerate() {
        extra.report_progress(i as f64, total as f64).await;
        if extra.is_cancelled() { break; }
    }
    Ok(json!({ "exported": true }))
}
```

If you don't need it, just omit it — the macro handles the unused parameter.

## MCP Tool Annotations

```rust
#[mcp_tool(
    description = "Delete a record",
    annotations(destructive = true, idempotent = false),
)]
async fn delete(args: DeleteArgs) -> pmcp::Result<Value> { ... }
```

## UI Widget Attachment (MCP Apps)

```rust
#[mcp_tool(description = "Add numbers", ui = CALCULATOR_WIDGET_URI)]
fn add(&self, args: AddInput) -> Result<ArithmeticResult> { ... }
```

## Sync vs Async

The macro auto-detects from `fn` vs `async fn` — no flags needed:

```rust
// Async (default for network/IO operations)
#[mcp_tool(description = "Fetch data")]
async fn fetch(args: FetchArgs) -> pmcp::Result<Value> { ... }

// Sync (for pure computation)
#[mcp_tool(description = "Calculate sum")]
fn sum(args: SumArgs) -> pmcp::Result<Value> { ... }
```

## Migration from Manual Implementations

### Tools: TypedTool → #[mcp_tool]

Before:
```rust
.tool(
    "calculator",
    TypedTool::new("calculator", |args: CalcArgs, _extra| {
        Box::pin(async move {
            Ok(json!({ "result": args.a + args.b }))
        })
    })
    .with_description("Calculate"),
)
```

After:
```rust
#[mcp_tool(description = "Calculate")]
async fn calculator(args: CalcArgs) -> pmcp::Result<Value> {
    Ok(json!({ "result": args.a + args.b }))
}
// .tool("calculator", calculator())
```

### Prompts: PromptHandler → #[mcp_prompt]

Before:
```rust
struct ReviewPrompt;

#[async_trait]
impl PromptHandler for ReviewPrompt {
    async fn handle(&self, args: HashMap<String, String>, _extra: RequestHandlerExtra) -> Result<GetPromptResult> {
        let language = args.get("language").unwrap_or(&"rust".to_string()).clone();
        Ok(GetPromptResult::new(
            vec![PromptMessage::user(Content::text(format!("Review {language} code")))],
            None,
        ))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo::new("review")
            .with_description("Review code")
            .with_arguments(vec![
                PromptArgument::new("language").with_description("Language").required(),
            ]))
    }
}
```

After:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    /// The programming language
    language: String,
}

#[mcp_prompt(description = "Review code")]
async fn review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!("Review {} code", args.language)))],
        None,
    ))
}
```

### Existing standalone functions

If you have standalone `async fn`s, wrap them in `#[mcp_server]`:

```rust
// Existing function — unchanged
pub async fn calculate_discriminant<F: FoundationClient>(
    foundation: &F, input: DiscriminantInput
) -> Result<DiscriminantResult> { ... }

// Thin wrapper
#[mcp_server]
impl<F: FoundationClient + 'static> ArithmeticsServer<F> {
    #[mcp_tool(description = "Calculate discriminant")]
    async fn discriminant(&self, args: DiscriminantInput) -> Result<DiscriminantResult> {
        calculate_discriminant(self.foundation.as_ref(), args).await
    }
}
```

## Examples

- `examples/63_mcp_tool_macro.rs` — Standalone tools, State<T>, sync detection
- `examples/64_mcp_prompt_macro.rs` — Standalone prompts, mixed tools+prompts, numeric args

Run with:
```bash
cargo run --example 63_mcp_tool_macro --features full
cargo run --example 64_mcp_prompt_macro --features full
```
