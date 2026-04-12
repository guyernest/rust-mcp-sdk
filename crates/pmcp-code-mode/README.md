# pmcp-code-mode

Code Mode validation and execution framework for MCP servers built on the PMCP SDK.

Enables LLM-generated code (GraphQL, JavaScript, SQL, MCP compositions) to be **validated, explained, and executed** with HMAC-signed approval tokens that cryptographically bind code to its validation result.

> **Status:** v0.1.0 — migrated from `pmcp-run/built-in/shared/pmcp-code-mode` into the SDK workspace in Phase 67.1. The public API is stabilizing; feedback is welcome before the 1.0 contract is locked.

## How It Works

```text
                       ┌──────────────┐
                       │   LLM Client │
                       └──────┬───────┘
                              │
               1. describe_schema()  <- schema exposed per exposure policy
                              │
               2. LLM generates code (GraphQL, JS, SQL, MCP composition)
                              │
               3. validate_code(code) ──────────────────────┐
                              │                              │
                    ┌─────────▼──────────┐                   │
                    │ ValidationPipeline │                   │
                    │  ┌───────────────┐ │     ┌────────────▼────────────┐
                    │  │ Parse         │ │     │ PolicyEvaluator (Cedar, │
                    │  │ Security scan │ │────>│ AVP, or custom)         │
                    │  │ Explain       │ │     └─────────────────────────┘
                    │  │ HMAC sign     │ │
                    │  └───────────────┘ │
                    └─────────┬──────────┘
                              │
                    approval_token (HMAC-SHA256 signed)
                              │
               4. User reviews explanation, approves
                              │
               5. execute_code(code, token) ────────────────┐
                              │                              │
                    ┌─────────▼──────────┐     ┌────────────▼──────┐
                    │ Token verification │     │ CodeExecutor impl │
                    │ (hash, expiry, sig)│────>│ (your backend)    │
                    └────────────────────┘     └───────────────────┘
                              │
                    execution result (JSON)
```

The token ensures that the **exact code** the user approved is what gets executed — any modification after validation invalidates the token.

## Supported Languages

The `language` attribute on `#[derive(CodeMode)]` selects the validation path at compile time. Each language maps to a feature-gated validation method on `ValidationPipeline`:

| Language | Derive Attribute | Validation Method | Feature Required |
|----------|-----------------|-------------------|------------------|
| GraphQL | `"graphql"` (default) | `validate_graphql_query_async` | *(none)* |
| JavaScript | `"javascript"` or `"js"` | `validate_javascript_code` | `openapi-code-mode` |
| SQL | `"sql"` | `validate_sql_query` | `sql-code-mode` |
| MCP | `"mcp"` | `validate_mcp_composition` | `mcp-code-mode` |

The `CodeLanguage` enum in `pmcp_code_mode::types` is the runtime representation of these values. Unknown language strings produce a compile error at macro expansion time.

## Quick Start

### Minimal: Direct Pipeline Usage

All pipeline constructors return `Result` — invalid configuration (such as an HMAC secret shorter than 16 bytes) is caught at startup, not at runtime.

```rust
use pmcp_code_mode::{
    CodeModeConfig, TokenSecret, ValidationPipeline, ValidationContext,
};

let config = CodeModeConfig::enabled();
let secret = TokenSecret::new(b"my-secret-key-at-least-16-bytes!".to_vec());
let pipeline = ValidationPipeline::from_token_secret(config, &secret)?;

let ctx = ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash");
let result = pipeline.validate_graphql_query("query { users { id name } }", &ctx)?;

assert!(result.is_valid);
assert!(result.approval_token.is_some()); // HMAC-signed token
```

### With Policy Evaluator

Wire a policy evaluator (Cedar, AWS Verified Permissions, or custom) into the pipeline for authorization checks between parsing and token signing:

```rust
use pmcp_code_mode::{
    CodeModeConfig, TokenSecret, ValidationPipeline, NoopPolicyEvaluator,
};
use std::sync::Arc;

let config = CodeModeConfig::enabled();
let secret = TokenSecret::new(b"my-secret-key-at-least-16-bytes!".to_vec());
let evaluator = Arc::new(NoopPolicyEvaluator::new()); // Use a real evaluator in production

let pipeline = ValidationPipeline::with_policy_evaluator(
    config, secret.expose_secret().to_vec(), evaluator
)?;
```

The policy evaluator is stored as `Arc<dyn PolicyEvaluator>`, enabling shared ownership across handlers and async tasks.

### With `#[derive(CodeMode)]` (Recommended)

The derive macro eliminates ~80 lines of boilerplate per server and supports all four languages. See the [pmcp-code-mode-derive README](../pmcp-code-mode-derive/README.md) for the full derive guide.

**GraphQL server (default):**

```rust
use pmcp_code_mode::{CodeModeConfig, TokenSecret, NoopPolicyEvaluator, CodeExecutor};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

#[derive(CodeMode)]
#[code_mode(context_from = "get_context")]
struct MyGraphQLServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyGraphQLExecutor>,
}
```

**JavaScript/OpenAPI server (Cost Coach, etc.):**

```rust
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "javascript")]
struct MyCostCoachServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyJsExecutor>,
}
```

**SQL server:**

```rust
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "sql")]
struct MySqlServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MySqlExecutor>,
}
```

All derive-generated servers share the same pattern: the `language` attribute selects the parser, the `context_from` method binds tokens to real user identity, and `CodeExecutor` handles your backend-specific execution.

**Field name convention:** The derive macro identifies required fields by fixed names. Missing any field produces a compile error listing all absent fields.

| Field Name | Type | Purpose |
|------------|------|---------|
| `code_mode_config` | `CodeModeConfig` | Validation pipeline config |
| `token_secret` | `TokenSecret` | HMAC signing secret |
| `policy_evaluator` | `Arc<impl PolicyEvaluator>` | Authorization backend |
| `code_executor` | `Arc<impl CodeExecutor>` | Your execution backend |

### Implementing `CodeExecutor`

This is the only trait you need to implement. The executor holds its own configuration (timeouts, limits, etc.) — `CodeExecutor::execute()` is intentionally kept simple:

```rust
use pmcp_code_mode::{CodeExecutor, ExecutionError, async_trait};
use serde_json::Value;

struct MyGraphQLExecutor { pool: PgPool }

#[async_trait]
impl CodeExecutor for MyGraphQLExecutor {
    async fn execute(
        &self,
        code: &str,          // Validated code (already token-verified)
        variables: Option<&Value>,
    ) -> Result<Value, ExecutionError> {
        // Execute against your backend. The framework has already verified
        // the HMAC token — do NOT re-verify here.
        let result = self.pool.execute_graphql(code, variables).await?;
        Ok(serde_json::to_value(result)?)
    }
}
```

Execution patterns supported through this single trait:
- **GraphQL:** Direct query execution against a GraphQL backend
- **JavaScript/OpenAPI:** JS plan execution via HTTP/SDK/MCP calls (executor holds `ExecutionConfig` for limits)
- **SQL:** Direct SQL execution against a database pool
- **MCP composition:** Tool calls routed to other MCP servers

## Key Types

| Type | What It Does |
|------|-------------|
| `ValidationPipeline` | Orchestrates: parse -> policy check -> security analysis -> explanation -> token |
| `CodeModeConfig` | Controls what's allowed: mutations, introspection, blocked fields, max depth, TTL |
| `CodeLanguage` | Enum of supported languages: `GraphQL`, `JavaScript`, `Sql`, `Mcp` |
| `PolicyEvaluator` | Trait for pluggable authorization (Cedar, AWS Verified Permissions, custom) |
| `CodeExecutor` | Trait for executing validated code against your backend |
| `TokenSecret` | Zeroizing HMAC secret — backed by `secrecy::SecretBox<[u8]>`, no Debug/Clone/Serialize |
| `HmacTokenGenerator` | Creates HMAC-SHA256 tokens binding code hash + context to approval |
| `TokenError` | Error type for constructor failures (e.g. HMAC secret too short) |
| `ApprovalToken` | Signed token: code hash, user ID, session ID, expiry, risk level, context hash |
| `NoopPolicyEvaluator` | **Test-only** evaluator that allows everything — NOT for production |
| `ValidationResponse` | Handler-level response wrapping `ValidationResult` + auto-approval, action, code hash |
| `ExecutionConfig` | JS execution limits: `max_api_calls`, `timeout_seconds`, `max_loop_iterations` |
| `CodeModeHandler` | Server-side handler trait with tool builder, pre-handle hooks, soft-disable |

## Configuration

`CodeModeConfig` controls the validation pipeline behavior:

```rust
let config = CodeModeConfig {
    enabled: true,
    allow_mutations: false,          // Block mutations by default
    blocked_mutations: HashSet::from(["deleteAll".into()]),
    allowed_mutations: HashSet::new(), // Empty = all non-blocked mutations allowed
    blocked_queries: HashSet::new(),
    allowed_queries: HashSet::new(),
    allow_introspection: false,      // Block schema introspection
    blocked_fields: HashSet::from(["User.ssn".into(), "User.password".into()]),
    max_query_depth: 10,
    max_query_fields: 100,
    token_ttl_seconds: 300,          // 5-minute token expiry
    auto_approve_threshold: Some(RiskLevel::Low), // Auto-approve low-risk queries
    ..CodeModeConfig::enabled()
};
```

### Query and Mutation Authorization

The pipeline enforces config-level authorization checks before policy evaluation:

- **Mutation control:** `allow_mutations` (global toggle), `blocked_mutations` (blocklist), `allowed_mutations` (allowlist). If `allowed_mutations` is non-empty, only listed mutations pass.
- **Query control:** `blocked_queries` (blocklist), `allowed_queries` (allowlist). Same allowlist-takes-precedence semantics as mutations.
- **Policy evaluation:** After config checks pass, `PolicyEvaluator::evaluate_operation()` runs (if configured) for fine-grained authorization.

## Feature Flags

| Feature | Default | What It Adds |
|---------|---------|-------------|
| *(none)* | yes | GraphQL validation via `graphql-parser` |
| `openapi-code-mode` | no | JavaScript/OpenAPI validation via SWC parser |
| `js-runtime` | no | JavaScript AST-based execution in pure Rust (implies `openapi-code-mode`) |
| `sql-code-mode` | no | SQL query validation and parameterization |
| `mcp-code-mode` | no | MCP-to-MCP tool composition (implies `js-runtime`) |
| `cedar` | no | Local Cedar policy evaluation via `cedar-policy 4.9` |

**Dependency chain:** `mcp-code-mode` -> `js-runtime` -> `openapi-code-mode`

## Security Design

See [SECURITY.md](./SECURITY.md) for the full threat model.

**Token security:**
- HMAC-SHA256 binds: code hash + user ID + session ID + server ID + context hash + risk level + expiry
- Token TTL default: 5 minutes
- Code canonicalization prevents whitespace-based bypass
- Any code modification after validation invalidates the token

**Secret handling:**
- `TokenSecret` backed by `secrecy::SecretBox<[u8]>`, zeroed on drop
- Explicitly **does not implement**: `Debug`, `Display`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`
- Minimum 16-byte secret enforced at construction — `HmacTokenGenerator::new` returns `Result<Self, TokenError>` (no panic)
- Access only via `expose_secret()` — framework-internal, never needed by server code

**Policy evaluation:**
- Default-deny: without a configured `PolicyEvaluator`, only basic config checks run
- Policy evaluator stored as `Arc<dyn PolicyEvaluator>` — shared safely across async handlers
- Cedar support via `cedar` feature flag (local evaluation, no network)
- `NoopPolicyEvaluator` for tests only — prominently documented with warnings

## Schema Exposure Architecture

The three-layer schema model controls what the LLM sees:

```text
Full Schema -> Exposure Policy -> Derived Schema -> LLM
              (filter/redact)   (what the LLM sees)
```

- `ExposureMode::Full` — expose everything
- `ExposureMode::ReadOnly` — expose reads, hide mutations
- `ExposureMode::Allowlist` — only specified operations
- `ExposureMode::Custom` — per-operation overrides via `ToolOverride`

## Breaking Changes in v0.1.0

### Constructors now return `Result`

All `ValidationPipeline` constructors and `HmacTokenGenerator::new` return `Result` instead of panicking on invalid input. This catches misconfiguration at startup.

```rust
// Before (v0.0.x):
let pipeline = ValidationPipeline::new(config, secret);

// After (v0.1.0):
let pipeline = ValidationPipeline::new(config, secret)?;
```

### Policy evaluator uses `Arc` (not `Box`)

`with_policy_evaluator` and `set_policy_evaluator` now accept `Arc<dyn PolicyEvaluator>` instead of `Box<dyn PolicyEvaluator>`. This enables shared ownership needed by the derive macro's generated handlers.

```rust
// Before:
pipeline.set_policy_evaluator(Box::new(my_evaluator));

// After:
pipeline.set_policy_evaluator(Arc::new(my_evaluator));
```

### `language` attribute selects validation path

`#[code_mode(language = "...")]` now dispatches to the correct language-specific validation method at compile time, not just tool metadata. Servers using JavaScript, SQL, or MCP can now use `#[derive(CodeMode)]` instead of manual handler structs.

See [CHANGELOG.md](./CHANGELOG.md) for the full list of changes.

## Known Limitations (v0.1.0)

1. **`TokenSecret::new` does not zeroize the source `Vec`.** The bytes are copied into `SecretBox` but the original `Vec` is not zeroed. Use `TokenSecret::from_env()` in production for maximum security.

2. **GraphQL only in default features.** JavaScript/OpenAPI validation requires the `openapi-code-mode` feature flag and pulls in SWC (~25MB compile artifact).

3. **No server-side token revocation.** Tokens are stateless (verified by HMAC). Once issued, a token is valid until it expires. Short TTL (5 min default) mitigates this.

4. **JavaScript validation is sync only.** `validate_javascript_code` is synchronous (no async variant). The derive macro handles this transparently — the generated async handler calls the sync method without `.await`.

5. **SQL and MCP validators are stub.** The `validate_sql_query` and `validate_mcp_composition` methods require their respective feature flags. These validators are being implemented — the derive macro dispatch is ready.

## Crate Dependencies

Minimal in the default feature set:

```
graphql-parser 0.4    — GraphQL parsing (pure Rust, no proc macros)
hmac 0.13 + sha2 0.11 — HMAC-SHA256 token signing
secrecy 0.10          — Secret memory management
zeroize 1.8           — Memory zeroing on drop
chrono 0.4            — Token timestamps
hex 0.4               — Hash encoding
base64 0.22           — Token encoding
serde + serde_json    — Serialization
thiserror             — Error types
async-trait           — Async trait support
```

The `cedar` feature adds `cedar-policy 4.9` (~3MB). The `openapi-code-mode` feature adds SWC.

## Running the Example

```bash
cargo run --example s41_code_mode_graphql --features full
```

This demonstrates the full validate -> approve -> execute round trip, including a rejection path for blocked mutations.

## Feedback Welcome

This is a pre-1.0 API. Key areas where we'd like team input:

- **Derive macro ergonomics** — are the fixed field names (`code_mode_config`, `token_secret`, etc.) workable, or do you need attribute-based field mapping (e.g., `#[code_mode(config = "my_config")]`)?
- **`context_from` pattern** — does returning `ValidationContext` from a method on your server struct work for your auth integration, or do you need an async version?
- **`CodeExecutor` trait surface** — is `execute(code, variables) -> Result<Value, ExecutionError>` sufficient for all language backends, or do specific languages need additional context?
- **`Result`-returning constructors** — are the `TokenError` variants clear enough for your error handling?
- **SQL validation** — what SQL dialects do you need? Should the validator support parameterized queries, prepared statements, or raw SQL only?
- **MCP composition** — what does your composition validation need? Schema compatibility checks, tool existence verification, or just structural validation?
- **Policy evaluation** — any use cases beyond Cedar and AVP?

File issues or discuss in the `#pmcp-sdk` channel.

## License

MIT
