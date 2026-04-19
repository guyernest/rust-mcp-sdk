# pmcp-macros

Procedural macros that eliminate boilerplate for MCP tools, prompts, resources, and
server routers. Shipped as part of `pmcp` via the `macros` feature flag. All four
macros plug into pmcp's compile-time schema generation, `State<T>` injection, and
handler registry, so your code focuses on behavior instead of protocol plumbing.

## Installation

Add `pmcp` with the `macros` feature. This pulls `pmcp-macros` transitively — you
do not need to add it as a direct dependency.

```toml
[dependencies]
pmcp = { version = "2.3", features = ["macros"] }
schemars = "1.0"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.46", features = ["full"] }
```

## Overview

pmcp-macros provides four attribute macros:

| Macro             | Applied to            | Purpose                                                                          |
| ----------------- | --------------------- | -------------------------------------------------------------------------------- |
| `#[mcp_tool]`     | `async fn` or `fn`    | Define a tool handler with a typed arg struct and compile-time schema            |
| `#[mcp_server]`   | `impl` block          | Collect tools and prompts on a type into a single registerable `McpServer`       |
| `#[mcp_prompt]`   | `async fn` or `fn`    | Define a prompt template with typed arguments and auto-generated argument schema |
| `#[mcp_resource]` | `async fn` or `fn`    | Define a resource handler with URI template matching and parameter extraction    |

Each macro generates the glue code and schema wiring that MCP servers need. Schemas
come from `schemars::JsonSchema` derives on your argument and result types, so the
wire protocol stays in sync with your Rust types automatically.

## `#[mcp_tool]`

### Purpose

`#[mcp_tool]` turns a plain async or sync function into a full `ToolHandler` with a
compile-time JSON Schema derived from its argument struct. The macro eliminates the
`Box::pin(async move { ... })` boilerplate required by manual `ToolHandler` impls,
enforces a `description` at compile time, and supports shared state via `State<T>`.

### Attributes

- `description = "..."` — optional as of pmcp-macros 0.6.0. Human-readable description
  exposed via the MCP `tools/list` response. If omitted, the function's rustdoc
  comment is used instead (see "Rustdoc-derived descriptions" below). If both are
  present, the attribute wins.
- `name = "..."` — optional. Defaults to the function name.
- `annotations(...)` — optional. MCP standard annotations: `read_only = bool`,
  `destructive = bool`, `idempotent = bool`, `open_world = bool`.
- `ui = "..."` — optional. Widget resource URI for MCP Apps integrations.

### Example

```rust,no_run
use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    /// First addend
    a: f64,
    /// Second addend
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    /// The sum of `a` and `b`
    sum: f64,
}

#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let _server = ServerBuilder::new()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", add())
        .build()?;
    Ok(())
}
```

The macro expands the annotated function into a zero-arg constructor (`add()`) that
returns a generated `AddTool` struct implementing `ToolHandler`. Register it with
`ServerBuilder::tool(name, handler)`.

### Rustdoc-derived descriptions (pmcp-macros 0.6.0+)

When you omit the `description = "..."` attribute, pmcp-macros harvests the
function's rustdoc comment and uses it as the tool description. This eliminates
the duplication of writing the same prose in both a `///` block and the macro
attribute.

```rust,no_run
use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs { a: f64, b: f64 }

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult { sum: f64 }

/// Add two numbers and return their sum.
#[mcp_tool]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}

# fn main() { let _ = add(); }
```

**Precedence:** when both a rustdoc comment and a `description = "..."` attribute
are present, the attribute wins. This is silent — no compiler warning — so that
rustdoc can be used freely for meta-commentary above tools that already specify
an explicit description.

**Normalization:** each rustdoc line is trimmed (leading/trailing whitespace
stripped); empty post-trim lines are dropped; remaining lines are joined with
`"\n"`.

**Error when both are absent:** if a `#[mcp_tool]`-annotated function has no
rustdoc and no `description = "..."` attribute, compilation fails with:

```text
error: mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function
```

**Requires:** pmcp-macros ≥ 0.6.0 (shipped with pmcp ≥ 2.4.0 — see CHANGELOG).

#### Limitations

The rustdoc harvester supports the common `/// doc comment` and `#[doc = "..."]`
string-literal forms. The following forms are NOT supported in pmcp-macros 0.6.0
and may be revisited in a later phase:

- **`#[doc = include_str!("...")]`** — the `include_str!` macro expansion is
  not evaluated at attribute-harvest time, so the attribute is silently skipped.
  Workaround: pass the file contents via an explicit `description = "..."` attribute,
  or inline the documentation directly as `///` lines.
- **`#[cfg_attr(condition, doc = "...")]`** — the outer attribute path is
  `cfg_attr`, not `doc`, so the conditional doc contribution is silently skipped.
  Workaround: use unconditional `///` lines for text that should appear in the
  tool description.
- **Indented code fences inside doc blocks** — each rustdoc line is trimmed
  independently, so indentation inside a ```fenced code block``` is lost.
  This is acceptable because MCP clients render tool descriptions as plain text,
  not as rustdoc HTML. For rich formatting, use `description = "..."` with the
  desired whitespace preserved.
- **Explicit empty description (`description = ""`)** — treated as PRESENT, not
  absent. The empty string wins silently, rustdoc fallback is NOT triggered, and
  the tool metadata's description is the empty string. If you want rustdoc to
  supply the description, omit the `description` attribute entirely.

### Shared state with `State<T>`

Tools that need access to shared data (a database handle, a config struct, a client
pool) can take a `State<T>` parameter. The macro detects it automatically and the
builder wires it in at registration time via `.with_state(...)`. No `Arc::clone`,
no `move` captures — just declare the dependency in the function signature.

```rust,no_run
use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

struct AppConfig {
    greeting_prefix: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Name to greet
    name: String,
}

#[mcp_tool(description = "Greet with prefix from config")]
async fn greet(args: GreetArgs, config: State<AppConfig>) -> pmcp::Result<Value> {
    Ok(json!({
        "greeting": format!("{}, {}!", config.greeting_prefix, args.name),
    }))
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let config = Arc::new(AppConfig {
        greeting_prefix: "Hello".into(),
    });

    let _server = ServerBuilder::new()
        .name("greeter")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("greet", greet().with_state(config))
        .build()?;
    Ok(())
}
```

Sync tools work the same way — declare a plain `fn` instead of `async fn`. The macro
auto-detects and wraps it accordingly.

### Full runnable example

See the complete showcase covering standalone tools, state injection, annotations,
and impl-block tools:
<https://github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs>

## `#[mcp_server]`

### Purpose

`#[mcp_server]` promotes an `impl` block into a server bundle. Every method
annotated with `#[mcp_tool]` or `#[mcp_prompt]` inside the block is collected,
`ToolHandler`/`PromptHandler` structs are generated for each, and an
`impl McpServer for YourType` is added so `ServerBuilder::mcp_server(...)` can
register them all at once. A single `Arc<YourType>` is shared across every handler,
so `&self` state is accessible for free.

### Example

```rust,no_run
use pmcp::{mcp_server, mcp_tool, ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

struct Calculator;

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs { a: f64, b: f64 }

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult { sum: f64 }

#[mcp_server]
impl Calculator {
    #[mcp_tool(description = "Add two numbers")]
    async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult { sum: args.a + args.b })
    }

    #[mcp_tool(description = "Health check", annotations(read_only = true))]
    async fn health(&self) -> pmcp::Result<Value> {
        Ok(json!({ "status": "ok" }))
    }
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let calculator = Calculator;
    let _server = ServerBuilder::new()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .mcp_server(calculator)
        .build()?;
    Ok(())
}
```

Mix `#[mcp_tool]` and `#[mcp_prompt]` freely within the same impl block — the macro
routes each method to the appropriate handler type. Use standalone `.tool(...)` /
`.prompt(...)` calls alongside `.mcp_server(...)` for one-off handlers that do not
belong to the type.

### Full runnable example

`examples/s23_mcp_tool_macro.rs` also demonstrates `#[mcp_server]` alongside
standalone `#[mcp_tool]` registrations:
<https://github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs>

## `#[mcp_prompt]`

### Purpose

`#[mcp_prompt]` defines a prompt template without the
`HashMap::get("x").ok_or()?.parse()?` boilerplate of hand-rolled `PromptHandler`
implementations. Arguments are derived from a struct that implements `JsonSchema`,
so the argument descriptions surface in the MCP `prompts/list` response without
duplication. MCP sends prompt arguments as strings, and the SDK coerces them into
numeric, boolean, or optional types automatically based on the struct definition.

### Attributes

- `description = "..."` — **required**. Human-readable description.
- `name = "..."` — optional. Defaults to the function name.

### Example

```rust,no_run
use pmcp::types::{Content, GetPromptResult, PromptMessage};
use pmcp::{mcp_prompt, ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    /// The programming language to review
    language: String,
    /// The code snippet to review
    code: String,
}

#[mcp_prompt(description = "Review code for quality issues")]
async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Please review this {} code for quality issues:\n\n```{}\n{}\n```",
            args.language, args.language, args.code,
        )))],
        Some("Code Review".to_string()),
    ))
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let _server = ServerBuilder::new()
        .name("review-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .prompt("code_review", code_review())
        .build()?;
    Ok(())
}
```

Like tools, prompts support `State<T>` injection — just add a `State<YourType>`
parameter to the function signature and wire it at registration with
`code_review().with_state(shared)`. Optional prompt arguments use `Option<T>` on the
struct field; the SDK fills in absent values with `None` automatically.

### Full runnable example

<https://github.com/paiml/pmcp/blob/main/examples/s24_mcp_prompt_macro.rs>

## `#[mcp_resource]`

> **Note:** `#[mcp_resource]` is currently imported directly from `pmcp_macros`
> rather than re-exported via `pmcp`. A future `pmcp` release will add it to the
> re-export alongside the other three macros, at which point the import can become
> `use pmcp::mcp_resource;`. Everything else about the macro is the same.

### Purpose

`#[mcp_resource]` defines a resource handler whose URI can carry template variables.
The macro generates a struct implementing `DynamicResourceProvider`, so at request
time an incoming URI is pattern-matched against the template and any captured
segments are handed to the function as typed parameters. No hand-rolled URI parsing,
no string indexing.

### Attributes

- `uri = "..."` — **required**. URI or URI template. Templates use RFC 6570-style
  `{variable_name}` placeholders (for example `docs://{topic}`).
- `description = "..."` — **required**. Human-readable description.
- `name = "..."` — optional. Defaults to the function name.
- `mime_type = "..."` — optional. Defaults to `"text/plain"`.

### URI template variables

The `uri` attribute accepts a URI template such as `docs://{topic}` or
`data://articles/{topic}`. Every `{variable_name}` placeholder inside the template
is **automatically extracted at request time and passed to the decorated function
as a parameter of type `String`**. The function parameter name must match the
template variable name exactly — the macro uses the name to route captured
substrings into the correct argument slot. You never have to parse the URI yourself.

For example, a resource declared as:

```text
#[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
async fn read_doc(topic: String) -> pmcp::Result<String> { ... }
```

will receive the `topic` segment of a request URI like `docs://rust-macros` bound
directly to the `topic: String` parameter. Multiple placeholders are supported —
declare one `String` parameter per `{variable_name}` in the template.

### Example

```rust,no_run
// Note: direct import until the `mcp_resource` re-export gap is closed.
use pmcp_macros::mcp_resource;

#[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
async fn read_doc(topic: String) -> pmcp::Result<String> {
    Ok(format!("# {topic}\n\nDocumentation content for `{topic}`."))
}

fn main() {
    // `read_doc()` is a zero-arg constructor that returns a resource provider.
    // Register it with `ResourceCollection::new().add_dynamic_provider(Arc::new(read_doc()))`
    // and hand the collection to the server via `.resources(collection)`.
    let _provider = read_doc();
}
```

Resources also support `State<T>` injection the same way tools and prompts do —
declare `db: State<MyDatabase>` alongside the template-variable parameters and the
SDK wires the shared state in at registration time.

## Feature flags

The `macros` feature flag on `pmcp` exists so users who do not need proc-macro
machinery can ship with a smaller compile surface. If you want any of the four
macros, enable `macros`. There is no reason to add `pmcp-macros` as a direct
dependency — always route through `pmcp`'s feature flag.

## License

MIT — see <https://github.com/paiml/pmcp/blob/main/LICENSE>
