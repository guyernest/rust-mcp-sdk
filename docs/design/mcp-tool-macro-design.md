# `#[mcp_tool]` / `#[mcp_prompt]` / `#[mcp_server]` Macro Design

**Status:** Shipped (Phases 58 + 59)
**Phase:** 58 (tools), 59 (prompts)
**Date:** 2026-03-21

## Problem

Building MCP tools with the PMCP SDK requires significant Rust boilerplate that intimidates developers unfamiliar with Rust's async and type system:

```rust
// TODAY: 15 lines of ceremony for a simple tool
.tool(
    "calculator",
    TypedTool::new("calculator", |args: CalculatorArgs, _extra| {
        Box::pin(async move {
            let result = args.a + args.b;
            Ok(json!({ "result": result }))
        })
    })
    .with_description("Add two numbers"),
)
```

Pain points:
- `Box::pin(async move { ... })` on every async tool (Rust-specific, confusing to newcomers)
- Tool name repeated in `.tool("name", TypedTool::new("name", ...))` (DRY violation)
- `_extra` parameter required even when unused
- Shared state requires `Arc::clone()` + `move` closure ceremony
- Description separated from the function, easy to forget
- No compile-time enforcement of good practices (description, typed output)

## Proposed Design

### Before and After

**Before (today):**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

let db = Arc::clone(&shared_db);
server_builder.tool(
    "calculator",
    TypedTool::new("calculator", move |args: CalculatorArgs, _extra| {
        let db = db.clone();
        Box::pin(async move {
            let result = db.compute(args.a, args.b).await?;
            Ok(json!({ "result": result }))
        })
    })
    .with_description("Perform arithmetic operations"),
)
```

**After (with `#[mcp_tool]`):**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

#[mcp_tool(description = "Perform arithmetic operations")]
async fn calculator(args: CalculatorArgs, db: State<Database>) -> Result<Value> {
    let result = db.compute(args.a, args.b).await?;
    Ok(json!({ "result": result }))
}

// Registration: one line, no name repetition
server_builder.tool("calculator", calculator())
```

### Core Principles

1. **Looks like a normal async fn** — no `Box::pin`, no `move`, no closure
2. **Description is mandatory** — enforced at compile time (not optional like today)
3. **State injection via `State<T>` extractor** — no manual Arc cloning
4. **Name derived from function** — but overridable via `name = "custom"`
5. **Output type encouraged** — `Result<TypedOutput>` generates outputSchema automatically
6. **`extra` is optional** — only declare it if you need cancellation/progress/auth

### API Surface

#### Minimal tool (no state, no extra)
```rust
#[mcp_tool(description = "Echo a message back")]
async fn echo(args: EchoArgs) -> Result<Value> {
    Ok(json!({ "message": args.text }))
}
```

#### Tool with typed output (generates outputSchema)
```rust
#[mcp_tool(description = "Perform arithmetic")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult> {
    // CalculatorResult: Serialize + JsonSchema
    // outputSchema auto-derived from CalculatorResult
    Ok(CalculatorResult { value: args.a + args.b })
}
```

#### Tool with shared state
```rust
#[mcp_tool(description = "Query the database")]
async fn query(args: QueryArgs, db: State<Database>) -> Result<Value> {
    // db: &Database (auto-deref from Arc<Database>)
    let rows = db.execute(&args.sql).await?;
    Ok(json!({ "rows": rows }))
}

// At registration, provide the state:
server_builder.tool("query", query().with_state(shared_db))
```

#### Tool with RequestHandlerExtra (cancellation, progress, auth)
```rust
#[mcp_tool(description = "Long-running export")]
async fn export(args: ExportArgs, extra: RequestHandlerExtra) -> Result<Value> {
    for (i, chunk) in data.chunks(100).enumerate() {
        extra.report_progress(i as f64, data.len() as f64).await;
        if extra.is_cancelled() { break; }
        process(chunk).await?;
    }
    Ok(json!({ "exported": true }))
}
```

#### Tool with state AND extra
```rust
#[mcp_tool(description = "Search with auth")]
async fn search(
    args: SearchArgs,
    db: State<Database>,
    extra: RequestHandlerExtra,
) -> Result<SearchResult> {
    let user = extra.auth_info.as_ref().ok_or("Unauthorized")?;
    db.search_for_user(&args.query, &user.sub).await
}
```

#### Sync tool (no async)
```rust
#[mcp_tool(description = "Get server version", sync)]
fn version() -> Result<Value> {
    Ok(json!({ "version": env!("CARGO_PKG_VERSION") }))
}
```

#### MCP standard annotations
```rust
#[mcp_tool(
    description = "Delete a record",
    annotations(destructive = true, idempotent = false),
)]
async fn delete(args: DeleteArgs, db: State<Database>) -> Result<Value> {
    db.delete(args.id).await?;
    Ok(json!({ "deleted": true }))
}
```

### `#[mcp_server]` — Router Macro (Impl Block)

For servers with multiple tools, an impl-block macro collects all `#[mcp_tool]` methods:

```rust
struct MyServer {
    db: Arc<Database>,
    cache: Arc<Cache>,
}

#[mcp_server]
impl MyServer {
    #[mcp_tool(description = "Query database")]
    async fn query(&self, args: QueryArgs) -> Result<QueryResult> {
        self.db.execute(&args.sql).await
    }

    #[mcp_tool(description = "Clear cache")]
    async fn clear_cache(&self) -> Result<Value> {
        self.cache.clear().await;
        Ok(json!({ "cleared": true }))
    }

    #[mcp_tool(description = "Health check", annotations(read_only = true))]
    async fn health(&self) -> Result<Value> {
        Ok(json!({ "status": "ok" }))
    }
}

// Registration: all tools from the impl block
let my_server = MyServer { db, cache };
server_builder.mcp_server(my_server)
```

Key DX advantages of `#[mcp_server]`:
- **`&self` gives natural access to shared state** — no `State<T>` extractors needed
- **All tools declared together** — easy to see the full server surface
- **Single registration call** — `.mcp_server(instance)` registers all tools at once
- **Familiar pattern** — looks like implementing a trait or a REST controller

### Parameter Signature Rules

The macro inspects function parameters by type to determine their role:

| Parameter type | Role | Required? |
|---|---|---|
| First struct param (JsonSchema + Deserialize) | Tool input args | Yes (unless no-arg tool) |
| `State<T>` | Shared state injection | No |
| `RequestHandlerExtra` | Cancellation, progress, auth | No |
| `&self` (in `#[mcp_server]` block) | Server instance state | No |

Order is flexible — the macro matches by type, not position.

### What the Macro Generates

For each `#[mcp_tool]` function, the macro generates:

1. **A struct** named `{FunctionName}Tool` (e.g., `CalculatorTool`)
2. **`ToolHandler` impl** with correct `handle()` and `metadata()` methods
3. **Input schema** from the args type via `schemars::schema_for!`
4. **Output schema** (if return type is `Result<T>` where T: JsonSchema + Serialize)
5. **A constructor function** `fn calculator() -> CalculatorTool` for ergonomic registration

For `#[mcp_server]` on an impl block:

1. **`impl McpServer for MyServer`** with `tools()` and `handle_tool()` methods
2. **Registration helper** `.mcp_server(instance)` on the builder

### Design Decisions

| Decision | Rationale |
|---|---|
| `#[mcp_tool]` not `#[tool]` | Distinguishes MCP tools from generic tool patterns in agent frameworks |
| Description mandatory | Enforces good practice — LLMs need descriptions to use tools effectively |
| `State<T>` extractor pattern | Familiar from Axum/Actix; avoids manual Arc ceremony |
| `extra` optional | Most tools don't need cancellation/progress — don't force the import |
| Typed output encouraged | Generates `outputSchema` for server-to-server composition |
| Sync auto-detected from `fn` vs `async fn` | No redundant flags — the function signature is the source of truth |
| `ui = "..."` attribute | MCP Apps servers need widget attachment at the tool level |
| Generic impl blocks supported | Composition servers use `impl<F: Trait> Server<F>` |
| `#[mcp_server]` separate from `#[mcp_tool]` | Clear separation between single-tool and multi-tool patterns |
| Function name = tool name | Convention over configuration; override with `name = "..."` |

### UI Widget Attachment

MCP Apps servers attach HTML widget UIs to tools. The macro supports this via the `ui` attribute:

```rust
#[mcp_tool(description = "Add two numbers", ui = CALCULATOR_WIDGET_URI)]
fn add(&self, args: AddInput) -> Result<ArithmeticResult> {
    // ...
}
```

The generated code calls `.with_ui(CALCULATOR_WIDGET_URI)` on the tool's metadata. Constants, string literals, and expressions are all valid `ui` values.

### Generic Impl Blocks

Composition servers use generic foundation clients. `#[mcp_server]` preserves type parameters and trait bounds:

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
}
```

### Auto-Validation

If the input type implements `validator::Validate`, the macro auto-calls `.validate()` before invoking the handler:

```rust
#[derive(Deserialize, JsonSchema, Validate)]
struct SqrtInput {
    #[validate(range(min = 0.0))]
    value: f64,
}

#[mcp_tool(description = "Square root")]
fn sqrt(&self, args: SqrtInput) -> Result<SqrtResult> {
    // args.validate() already called — invalid input never reaches here
    Ok(SqrtResult { result: args.value.sqrt() })
}
```

### Migration Pattern for Existing Functions

Existing standalone async functions can't be directly annotated with `#[mcp_tool]` (the macro needs a specific signature). The expected migration path is thin wrappers in an `#[mcp_server]` impl:

```rust
// Existing function — unchanged
pub async fn calculate_discriminant<F: FoundationClient>(
    foundation: &F, input: DiscriminantInput
) -> Result<DiscriminantResult> { /* ... */ }

// Migration: thin wrapper in #[mcp_server]
#[mcp_server]
impl<F: FoundationClient + 'static> ArithmeticsServer<F> {
    #[mcp_tool(description = "Calculate discriminant")]
    async fn discriminant(&self, args: DiscriminantInput) -> Result<DiscriminantResult> {
        calculate_discriminant(self.foundation.as_ref(), args).await
    }
}
```

### Migration Path

The existing `TypedTool`, `TypedToolWithOutput`, and `TypedSyncTool` APIs remain unchanged. The macro generates code that uses these types internally — it's sugar, not a replacement. Teams can migrate one tool at a time.

### Not In Scope

- ~~`#[mcp_prompt]` macro~~ — **Shipped in Phase 59**
- `#[mcp_resource]` macro — future phase
- Auto-discovery / inventory of tools at compile time
- Hot-reload of tool definitions
- WASM target support for macros

## Team Feedback (Resolved)

| Question | Team Answer |
|---|---|
| Does `State<T>` feel natural? | Yes for standalone. For composition servers, `#[mcp_server]` with `&self` is the natural fit. Generic impl blocks (`impl<F: Trait>`) are critical. |
| Is mandatory description annoying? | Helpful. All 9 tools already have descriptions. Compile-time enforcement is correct. |
| Should `#[mcp_server]` be primary? | Yes for 3+ tools. Eliminates 6x `TypedToolWithOutput::new(...)` boilerplate. "Killer feature." |
| Do you use `RequestHandlerExtra`? | Zero of 9 tools use it. Opt-in is absolutely correct. |

### Issues Raised

| Issue | Priority | Resolution |
|---|---|---|
| No `.with_ui()` equivalent | P1 | Added `ui = "..."` attribute on `#[mcp_tool]` |
| Generic impl blocks | P1 | Confirmed — macro preserves type params and bounds |
| No annotation on existing functions | P2 | Documented thin-wrapper migration pattern |
| Auto-validate inputs | P3 | Added auto-`.validate()` if type implements `Validate` |
| Redundant `sync` flag | P3 | Removed — auto-detect from `fn` vs `async fn` |

### Expected Migration Impact

| Server | Today | After | Reduction |
|---|---|---|---|
| Calculator (6 sync tools + UI) | 106 lines | ~30 lines | 70% |
| Arithmetics (3 async, generic state) | 50 lines + 4 Arc clones | ~15 lines, 0 clones | 70% |

---

*Design document for Phase 58: #[mcp_tool] Proc Macro*
*Feedback: share this document and open issues or discussions*
