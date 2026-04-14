# Chapter 12.9: Code Mode — LLM Code Validation and Execution

Code Mode enables MCP servers to validate, explain, and execute LLM-generated code (GraphQL, JavaScript, SQL, MCP compositions) with cryptographic approval tokens that bind the validated code to a specific user, session, and schema version.

## The Problem

When an LLM generates code to run against your backend — a GraphQL query, a JavaScript plan, a SQL statement — you face a security challenge: how do you ensure that the exact code a user approved is what gets executed, and that it respects your authorization policies?

Without Code Mode, servers must hand-roll validation, token generation, and policy enforcement for every code-generating workflow. This leads to inconsistent security boundaries and duplicated boilerplate.

## How It Works

```text
LLM generates code
        │
        ▼
validate_code(code) ──────► ValidationPipeline
        │                     ├── Parse (language-specific)
        │                     ├── Security scan
        │                     ├── Policy evaluation (Cedar/AVP/custom)
        │                     ├── Explain (human-readable)
        │                     └── HMAC-sign
        ▼
approval_token ◄──────────── (code hash + user + session + expiry)
        │
User reviews explanation, approves
        │
        ▼
execute_code(code, token) ─► Token verification
        │                     ├── Signature check
        │                     ├── Code hash match
        │                     ├── Expiry check
        │                     └── Context match
        ▼
CodeExecutor::execute(code) → Result
```

The HMAC-SHA256 token binds: code hash, user ID, session ID, server ID, context hash, risk level, and expiry. Any modification to the code after validation invalidates the token.

## Using the Derive Macro

The `#[derive(CodeMode)]` macro eliminates ~80 lines of handler boilerplate per server:

```rust,ignore
use pmcp_code_mode::{CodeModeConfig, TokenSecret, NoopPolicyEvaluator, ValidationContext};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "graphql")]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

impl MyServer {
    fn get_context(&self, extra: &pmcp::RequestHandlerExtra) -> ValidationContext {
        // Build real context from the MCP session
        ValidationContext::new("user-123", "session-456", "schema-v1", "perms-v1")
    }
}

let server = Arc::new(MyServer { /* ... */ });
let builder = server.register_code_mode_tools(pmcp::Server::builder())?;
```

### Supported Languages

The `language` attribute selects the validation path at compile time:

| Language | Attribute | Feature Required |
|----------|----------|------------------|
| GraphQL | `"graphql"` (default) | *(none)* |
| JavaScript | `"javascript"` | `openapi-code-mode` |
| SQL | `"sql"` | `sql-code-mode` |
| MCP | `"mcp"` | `mcp-code-mode` |

### Standard Adapters

For JavaScript, SDK, and MCP servers, use the standard adapters instead of implementing `CodeExecutor` manually:

```rust,ignore
// JavaScript + HTTP calls (e.g., Cost Coach)
let executor = Arc::new(JsCodeExecutor::new(http_client, ExecutionConfig::default()));

// JavaScript + SDK operations
let executor = Arc::new(SdkCodeExecutor::new(sdk_client, ExecutionConfig::default()));

// MCP tool composition
let executor = Arc::new(McpCodeExecutor::new(mcp_router, ExecutionConfig::default()));
```

## Deployment Configuration

When deploying with `cargo pmcp deploy`, include a `config.toml` that declares your server's available operations. The pmcp.run platform reads this to populate the Code Mode policy page, allowing administrators to control operations by category.

```toml
[server]
name = "cost-coach"
type = "openapi-api"

[code_mode]
allow_writes = false
allow_deletes = false

[[code_mode.operations]]
name = "getCostAndUsage"
description = "Retrieve AWS cost and usage data"
path = "/ce/GetCostAndUsage"
method = "POST"

[[code_mode.operations]]
name = "deleteBudget"
description = "Delete a budget"
path = "/budgets/DeleteBudget"
method = "POST"
destructive_hint = true
```

Operations are automatically categorized (read/write/delete/admin) based on the server type and HTTP method, GraphQL operation type, or explicit `operation_category` overrides.

## Policy Evaluation

Code Mode supports pluggable policy evaluation between validation and token generation:

- **Cedar:** Local policy evaluation via the `cedar` feature flag
- **AWS Verified Permissions:** Remote evaluation via external crate
- **Custom:** Implement the `PolicyEvaluator` trait

The policy evaluator runs after parsing and security scanning, before the approval token is generated. A denied operation never receives a token.

## Security Properties

- Tokens are stateless (HMAC-verified, not stored server-side)
- Default TTL: 5 minutes
- `TokenSecret` uses `secrecy::SecretBox` with zeroize-on-drop
- Minimum 16-byte secret enforced at construction (returns `Result`, no panic)
- Code canonicalization prevents whitespace-based bypass

## Next Steps

- See the [pmcp-code-mode README](https://docs.rs/pmcp-code-mode) for the full API reference
- See the [pmcp-code-mode-derive README](https://docs.rs/pmcp-code-mode-derive) for derive macro details
- Run the example: `cargo run --example s41_code_mode_graphql --features full`
