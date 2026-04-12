# pmcp-code-mode-derive

Derive macro for Code Mode validation and execution in MCP servers. Generates `register_code_mode_tools` to wire `validate_code` and `execute_code` tools onto a `ServerBuilder` with a single attribute.

Supports all Code Mode languages: GraphQL (default), JavaScript/OpenAPI, SQL, and MCP composition.

## Quick Start

### GraphQL Server (default)

```rust,ignore
use pmcp_code_mode::{
    CodeModeConfig, TokenSecret, NoopPolicyEvaluator, CodeExecutor,
    ExecutionError, ValidationContext,
};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

struct MyExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for MyExecutor {
    async fn execute(
        &self,
        code: &str,
        _variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        Ok(serde_json::json!({"result": "ok"}))
    }
}

#[derive(CodeMode)]
#[code_mode(context_from = "get_context")]
struct MyGraphQLServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

impl MyGraphQLServer {
    fn get_context(&self, extra: &pmcp::RequestHandlerExtra) -> ValidationContext {
        ValidationContext::new("user-123", "session-456", "schema-v1", "perms-v1")
    }
}

let server = Arc::new(MyGraphQLServer { /* ... */ });
let builder = server.register_code_mode_tools(pmcp::Server::builder())?;
```

### JavaScript/OpenAPI Server (e.g. Cost Coach)

Requires the `openapi-code-mode` feature on `pmcp-code-mode`.

```rust,ignore
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "javascript")]
struct CostCoachServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<JsExecutor>,  // Holds its own ExecutionConfig
}
```

The generated handler calls `validate_javascript_code` (sync) instead of `validate_graphql_query_async`. The executor holds its own `ExecutionConfig` with `max_api_calls`, `timeout_seconds`, `max_loop_iterations` — no change to the `CodeExecutor` trait.

### SQL Server

Requires the `sql-code-mode` feature on `pmcp-code-mode`.

```rust,ignore
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "sql")]
struct MySqlServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<SqlExecutor>,
}
```

### MCP Composition Server

Requires the `mcp-code-mode` feature on `pmcp-code-mode`.

```rust,ignore
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "mcp")]
struct McpRouter {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<McpCompositionExecutor>,
}
```

### Testing: Without `context_from`

Omitting `context_from` uses placeholder context values and marks the generated method `#[deprecated]` to guide toward the production path.

```rust,ignore
#[derive(CodeMode)]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

#[allow(deprecated)]
let builder = server.register_code_mode_tools(pmcp::Server::builder())?;
// ^ Compiler emits: "use #[code_mode(context_from = ...)] to bind tokens to real user context"
```

## Struct-Level Attributes

| Attribute | Type | Default | Effect |
|-----------|------|---------|--------|
| `context_from` | `"method_name"` | *(none)* | Method on your struct returning `ValidationContext`. Enables real token binding. Changes receiver to `self: &Arc<Self>`. |
| `language` | `"language_name"` | `"graphql"` | Selects the validation method and tool metadata at compile time. |

### Supported Languages

| Value | Validation Method | Async | Feature Required |
|-------|-------------------|-------|------------------|
| `"graphql"` (default) | `validate_graphql_query_async` | yes | *(none)* |
| `"javascript"` or `"js"` | `validate_javascript_code` | no | `openapi-code-mode` |
| `"sql"` | `validate_sql_query` | no | `sql-code-mode` |
| `"mcp"` | `validate_mcp_composition` | yes | `mcp-code-mode` |

**How it works:** The derive macro emits different `quote!` blocks based on the `language` value — the dispatch is entirely at compile time, zero runtime cost. Using a language without its feature flag produces a standard "method not found" compile error. Unknown language values produce a clear compile error listing all supported values.

**Adding a new language:** Add a variant to `CodeLanguage` in `pmcp-code-mode/src/types.rs` and a match arm in `gen_validation_call()` in this crate. A sync test enforces both sides stay aligned.

### `context_from` Method Signature

```rust,ignore
impl MyServer {
    fn get_context(&self, extra: &pmcp::RequestHandlerExtra) -> ValidationContext {
        // Build context from the MCP session/auth info
        ValidationContext::new(user_id, session_id, schema_hash, permissions_hash)
    }
}
```

The method receives `&pmcp::RequestHandlerExtra` which carries the MCP request context (session ID, auth metadata, etc.). The returned `ValidationContext` is hashed into the approval token — tokens become bound to the specific user, session, and schema version.

## Required Fields

The macro identifies fields by **fixed well-known names**. All four must be present:

| Field Name | Required Type | Purpose |
|------------|---------------|---------|
| `code_mode_config` | `CodeModeConfig` | Validation pipeline configuration |
| `token_secret` | `TokenSecret` | HMAC signing secret (zeroize-on-drop) |
| `policy_evaluator` | `Arc<impl PolicyEvaluator>` | Policy evaluation backend |
| `code_executor` | `Arc<impl CodeExecutor>` | Code execution backend |

Missing any field produces a compile error listing all absent fields:

```text
error: #[derive(CodeMode)] requires field `token_secret` (type: TokenSecret).
       Required fields: code_mode_config, token_secret, policy_evaluator, code_executor
```

## Generated Method

The macro generates `register_code_mode_tools` on your struct:

```rust,ignore
// With context_from:
impl MyServer {
    pub fn register_code_mode_tools(
        self: &Arc<Self>,
        builder: pmcp::ServerBuilder,
    ) -> Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> { ... }
}

// Without context_from (deprecated):
impl MyServer {
    #[deprecated(note = "use #[code_mode(context_from = ...)] to bind tokens to real user context")]
    pub fn register_code_mode_tools(
        &self,
        builder: pmcp::ServerBuilder,
    ) -> Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> { ... }
}
```

The method:

1. Creates a shared `Arc<ValidationPipeline>` from the struct's config, secret, and policy evaluator
2. Registers a `validate_code` tool handler (language-specific validation + policy evaluation + HMAC token)
3. Registers an `execute_code` tool handler (token verification + `CodeExecutor::execute`)
4. Returns the builder with both tools added, or `TokenError` if the HMAC secret is invalid

### Why `Result`?

`register_code_mode_tools` returns `Result<ServerBuilder, TokenError>` because it constructs the `ValidationPipeline` internally, which validates the HMAC secret length. This catches misconfigured secrets at server startup rather than panicking at runtime.

```rust,ignore
// Fails at startup with a clear error:
let secret = TokenSecret::new(b"short".to_vec()); // < 16 bytes
let result = server.register_code_mode_tools(builder);
// Err(TokenError::SecretTooShort { minimum: 16, actual: 5 })
```

### Why `Arc<Self>`?

When `context_from` is set, the generated `ValidateCodeHandler` needs to call a method on your server struct during request handling. Since handlers are `Send + Sync + 'static` (they live inside the MCP server's async runtime), the handler holds an `Arc<MyServer>` reference. This requires the registration call to use `self: &Arc<Self>`.

Without `context_from`, handlers don't reference the parent struct, so `&self` suffices.

## Compile-Time Safety

The macro enforces several constraints at compile time:

**Missing fields:** A single error lists all absent required fields with expected types.

**`Send + Sync`:** The macro generates a compile-time assertion that the struct is `Send + Sync`. Non-threadsafe fields (e.g., `Rc<T>`) produce a clear compiler error:

```text
error: `Rc<String>` cannot be sent between threads safely
```

**Invalid `context_from`:** If the `context_from` value is not a valid Rust identifier, the macro emits a compile error instead of panicking:

```text
error: `context_from = "not-valid"` is not a valid Rust identifier
```

**Unsupported language:** Unknown `language` values produce a compile error listing all supported values:

```text
error: `language = "python"` is not a supported language.
       Supported values: "graphql" (default), "javascript", "sql", "mcp"
```

**Missing feature flag:** Using `language = "javascript"` without `openapi-code-mode` enabled produces a standard Rust "method not found" error — the feature-gated method doesn't exist.

**Missing `context_from` method:** If you specify `context_from = "get_context"` but don't define the method, the generated code produces a standard Rust "method not found" error pointing at your struct.

## Breaking Changes in v0.1.0

`register_code_mode_tools` now returns `Result<ServerBuilder, TokenError>` instead of `ServerBuilder`:

```rust,ignore
// Before (v0.0.x):
let builder = server.register_code_mode_tools(builder);

// After (v0.1.0):
let builder = server.register_code_mode_tools(builder)?;
```

`language` attribute now selects the validation path, not just metadata. Servers that previously used `language = "javascript"` as a documentation hint will now get their code routed through `validate_javascript_code` instead of the GraphQL parser (which is the correct behavior).

See [CHANGELOG.md](./CHANGELOG.md) for the full list of changes.

## Dependencies

This is a proc-macro crate. It depends on `syn`, `quote`, `proc-macro2`, and `darling` for attribute parsing and macro expansion. At runtime, the generated code depends on `pmcp` and `pmcp-code-mode`.

## Related Crates

- [`pmcp-code-mode`](../pmcp-code-mode/README.md) — Core types, validation pipeline, and execution framework
- [`pmcp`](https://crates.io/crates/pmcp) — PMCP SDK with `ServerBuilder` and `ToolHandler`

## Feedback Welcome

Key questions for reviewers:

- **Fixed field names vs. attribute mapping** — is `code_mode_config`/`token_secret`/`policy_evaluator`/`code_executor` workable, or do you need `#[code_mode(config = "my_field")]` style overrides?
- **`context_from` sync only** — the context method is currently sync (`fn get_context(&self, extra) -> ValidationContext`). Do you need an async version for cases where context requires a network call?
- **`CodeExecutor` per language** — the executor holds its own config (e.g., `ExecutionConfig` for JS). Is this pattern working for your backend, or do you need language-specific executor traits?
- **SQL dialect** — what SQL dialects and parameterization styles does your team need?
- **MCP composition** — what validation should `validate_mcp_composition` perform? Schema compatibility, tool existence, or structural checks?
- **Error handling** — is `TokenError` sufficient, or should `register_code_mode_tools` return a broader error type?

File issues or discuss in the `#pmcp-sdk` channel.

## License

MIT
