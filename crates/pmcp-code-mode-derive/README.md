# pmcp-code-mode-derive

Derive macro for Code Mode validation and execution in MCP servers. Generates `register_code_mode_tools` to wire `validate_code` and `execute_code` tools onto a `ServerBuilder` with a single attribute.

## Quick Start

```rust,ignore
use pmcp_code_mode::{CodeModeConfig, TokenSecret, NoopPolicyEvaluator, CodeExecutor, ExecutionError};
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
        Ok(serde_json::json!({"status": "ok"}))
    }
}

#[derive(CodeMode)]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

// Use the generated method to register tools
let builder = pmcp::Server::builder();
let builder = server.register_code_mode_tools(builder);
```

## Required Field Names (v0.1.0)

The macro identifies fields by **well-known names**. All four fields must be present:

| Field Name | Required Type | Purpose |
|------------|---------------|---------|
| `code_mode_config` | `CodeModeConfig` | Validation pipeline configuration |
| `token_secret` | `TokenSecret` | HMAC signing secret (zeroize-on-drop) |
| `policy_evaluator` | `Arc<impl PolicyEvaluator>` | Policy evaluation backend |
| `code_executor` | `Arc<impl CodeExecutor>` | Code execution backend |

If any required field is missing, the macro emits a **single** compile error listing all absent fields:

```text
error: #[derive(CodeMode)] is missing required field(s): `token_secret`, `code_executor`.
       Required fields: code_mode_config, token_secret, policy_evaluator, code_executor
```

**Note:** Attribute-based field mapping (e.g., `#[code_mode(evaluator = "my_eval")]`) is planned for v0.2.0. The v0.1.0 convention uses fixed names for simplicity and clarity.

## Generated Method

The macro generates one method on your struct:

```rust,ignore
impl MyServer {
    pub fn register_code_mode_tools(
        &self,
        builder: pmcp::ServerBuilder,
    ) -> pmcp::ServerBuilder { ... }
}
```

This follows the by-value fluent pattern used throughout the PMCP SDK. The method:

1. Creates a `ValidationPipeline` from the struct's `code_mode_config` and `token_secret`
2. Registers a `validate_code` tool handler (GraphQL validation + HMAC token generation)
3. Registers an `execute_code` tool handler (token verification + `CodeExecutor::execute`)
4. Returns the builder with both tools added

## Compile-Time Safety

The macro generates a `Send + Sync` assertion for the annotated struct. If any field is not `Send + Sync`, compilation fails with a clear error:

```text
error: `Rc<String>` cannot be sent between threads safely
```

This ensures the server struct is safe to share across async tasks.

## Dependencies

This is a proc-macro crate. It depends on `syn`, `quote`, `proc-macro2`, and `darling` for macro expansion. At runtime, the generated code depends on `pmcp` and `pmcp-code-mode`.

## Related Crates

- [`pmcp-code-mode`](https://crates.io/crates/pmcp-code-mode) -- Core types, validation pipeline, and execution framework
- [`pmcp`](https://crates.io/crates/pmcp) -- PMCP SDK with `ServerBuilder` and `ToolHandler`

## License

MIT
