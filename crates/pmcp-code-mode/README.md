# pmcp-code-mode

Code Mode validation and execution framework for MCP servers built on the PMCP SDK.

Enables LLM-generated code (GraphQL, JavaScript, SQL, MCP compositions) to be **validated, explained, and executed** with HMAC-signed approval tokens that cryptographically bind code to its validation result.

> **Status:** v0.3.0 — multi-language validation with policy enforcement, standard adapters, deploy-time config. The public API is stabilizing; feedback is welcome before the 1.0 contract is locked.

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

For **GraphQL** and **SQL** servers, you implement `CodeExecutor` directly — your executor calls your database or GraphQL backend.

For **JavaScript/OpenAPI**, **SDK**, and **MCP** servers, use the standard adapters instead of implementing `CodeExecutor` manually.

### Standard Adapters (JS/SDK/MCP)

These adapters bridge the low-level execution traits to `CodeExecutor`, eliminating ~75 lines of manual handler boilerplate per server. Each compiles JavaScript code via `PlanCompiler`, executes via `PlanExecutor`, and logs execution metadata automatically.

**`JsCodeExecutor<H>`** — JavaScript + HTTP calls (Pattern B). Requires `js-runtime` feature.

```rust
use pmcp_code_mode::{JsCodeExecutor, ExecutionConfig};

// Your HttpExecutor implementation (e.g., CostExplorerHttpExecutor)
let http = CostExplorerHttpExecutor::new(clients.clone());
let config = ExecutionConfig::default()
    .with_blocked_fields(["password", "ssn"]);
let code_executor = Arc::new(JsCodeExecutor::new(http, config));
// Pass as code_executor field in your #[derive(CodeMode)] struct
```

**`SdkCodeExecutor<S>`** — JavaScript + SDK operations (Pattern C). Requires `js-runtime` feature.

```rust
use pmcp_code_mode::{SdkCodeExecutor, ExecutionConfig};

let sdk = MyCostExplorerSdk::new(credentials);
let config = ExecutionConfig::default();
let code_executor = Arc::new(SdkCodeExecutor::new(sdk, config));
```

**`McpCodeExecutor<M>`** — JavaScript + MCP tool composition (Pattern D). Requires `mcp-code-mode` feature.

```rust
use pmcp_code_mode::{McpCodeExecutor, ExecutionConfig};

let mcp = MyMcpRouter::new(foundation_servers);
let config = ExecutionConfig::default();
let code_executor = Arc::new(McpCodeExecutor::new(mcp, config));
```

All three adapters:
- Create a fresh `PlanCompiler` + `PlanExecutor` per call (cheap — your `HttpExecutor`/`SdkExecutor`/`McpExecutor` holds `Arc`'d state)
- Forward `variables` into the execution plan as `args` (available in JS code as the `args` variable)
- Log `api_calls` count and `execution_time_ms` via `tracing::debug!`

### End-to-End: Cost Coach with Derive Macro

Before (manual handlers, ~75 lines):
```rust,ignore
struct ValidateState { pipeline: Arc<ValidationPipeline>, config: CodeModeConfig }
struct ExecuteState { pipeline: Arc<ValidationPipeline>, http: CostExplorerHttpExecutor, config: ExecutionConfig }
// ... implement ToolHandler for both, wire manually ...
```

After (derive macro + adapter, 8 lines):
```rust,ignore
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "javascript")]
struct CostCoachServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<JsCodeExecutor<CostExplorerHttpExecutor>>,
}

let http = CostExplorerHttpExecutor::new(clients.clone());
let code_executor = Arc::new(JsCodeExecutor::new(http, ExecutionConfig::default()));
let server = Arc::new(CostCoachServer { /* ... */ });
let builder = server.register_code_mode_tools(pmcp::Server::builder())?;
```

## Key Types

| Type | What It Does |
|------|-------------|
| `ValidationPipeline` | Orchestrates: parse -> policy check -> security analysis -> explanation -> token |
| `CodeModeConfig` | Controls what's allowed: mutations, introspection, blocked fields, max depth, TTL |
| `CodeLanguage` | Enum of supported languages: `GraphQL`, `JavaScript`, `Sql`, `Mcp` |
| `PolicyEvaluator` | Trait for pluggable authorization (Cedar, AWS Verified Permissions, custom) |
| `CodeExecutor` | Trait for executing validated code against your backend |
| `JsCodeExecutor<H>` | Standard adapter: `HttpExecutor` -> `CodeExecutor` (JS+HTTP, `js-runtime` feature) |
| `SdkCodeExecutor<S>` | Standard adapter: `SdkExecutor` -> `CodeExecutor` (JS+SDK, `js-runtime` feature) |
| `McpCodeExecutor<M>` | Standard adapter: `McpExecutor` -> `CodeExecutor` (JS+MCP, `mcp-code-mode` feature) |
| `ExecutionConfig` | JS execution limits: `max_api_calls`, `timeout_seconds`, `max_loop_iterations`, blocked fields |
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

## Deployment Configuration (`config.toml`)

When deploying with `cargo pmcp deploy`, the server's `config.toml` is automatically included in the deploy ZIP. The pmcp.run platform extracts operation metadata from this file to populate the Code Mode policy page in the admin UI — administrators can then enable/disable individual operations by category.

### config.toml Schema

The `[[code_mode.operations]]` section declares available operations with canonical IDs and categories. When present, these feed into the `OperationRegistry` which:

1. **Maps raw API paths to plain-name IDs** in Cedar entity `calledOperations` (e.g., `getCostAnomalies` instead of `POST:/getCostAnomalies`)
2. **Overrides HTTP method-based action routing** with the declared `category` (e.g., a POST endpoint declared as `category = "read"` routes to the `Read` Cedar action, not `Write`)

Each entry has four fields:

| Field | Required | Purpose |
|-------|----------|---------|
| `id` | yes | Canonical operation name — appears in Cedar `calledOperations` and admin UI |
| `category` | yes | Action routing: `"read"`, `"write"`, `"delete"`, `"admin"` |
| `description` | no | Human-readable label for admin UI and LLM context |
| `path` | no | Raw API path to match against `api.post('/...')` calls (exact match) |

**OpenAPI server:**

```toml
[server]
name = "cost-coach"
type = "openapi-api"

[code_mode]
enabled = true
server_id = "cost-coach"
openapi_allow_writes = true    # Required when operations use POST for reads

[[code_mode.operations]]
id = "getCostAndUsage"
category = "read"
description = "Historical cost and usage data"
path = "/getCostAndUsage"

[[code_mode.operations]]
id = "getCostAnomalies"
category = "read"
description = "Cost anomalies detected by AWS"
path = "/getCostAnomalies"

[[code_mode.operations]]
id = "deleteBudget"
category = "delete"
description = "Delete a budget"
path = "/deleteBudget"
```

**Loading config from TOML** (recommended for external servers):

```rust
const CONFIG_TOML: &str = include_str!("../../config.toml");

let config = CodeModeConfig::from_toml(CONFIG_TOML)
    .expect("Invalid code_mode section in config.toml");
```

`from_toml` parses the `[code_mode]` section including all `[[code_mode.operations]]` entries. Other sections (`[server]`, `[[tools]]`, etc.) are ignored. This is preferable to manual `CodeModeConfig` construction because operations are automatically populated.

**GraphQL server:**

```toml
[server]
name = "open-images"
type = "graphql-api"

[code_mode]
allow_mutations = false

[[code_mode.operations]]
id = "searchImages"
category = "read"
description = "Search the image catalog"

[[code_mode.operations]]
id = "createCollection"
category = "write"
description = "Create a new image collection"

[[code_mode.operations]]
id = "deleteImage"
category = "delete"
description = "Permanently delete an image"
```

**SQL server:**

```toml
[server]
name = "analytics"
type = "sql"

[code_mode]
allow_writes = true
allow_deletes = false
blocked_tables = ["audit_log", "credentials"]

[database]
[[database.tables]]
name = "orders"
description = "Customer order history"

[[database.tables]]
name = "products"
description = "Product catalog"
```

**MCP composition server:**

```toml
[server]
name = "orchestrator"
type = "mcp-api"

[[code_mode.operations]]
id = "analyze_costs"
category = "read"
description = "Multi-step cost analysis workflow"

[[code_mode.operations]]
id = "provision_resources"
category = "admin"
description = "Provision cloud resources"
```

### Action Routing and Categorization

The Cedar action (`Read`/`Write`/`Delete`/`Admin`) sent to AVP determines which policies apply. The SDK resolves the action through a two-tier system:

**Tier 1: Operation category (from `[[code_mode.operations]]`)**

When the `OperationRegistry` has an entry for the API path with a declared `category`, that category determines the action — regardless of the HTTP method:

| `category` | Cedar Action | Example |
|-----------|-------------|---------|
| `"read"` | `CodeMode::Action::"Read"` | `api.post('/getCostAnomalies')` with `category = "read"` → Read |
| `"write"` | `CodeMode::Action::"Write"` | `api.post('/createBudget')` with `category = "write"` → Write |
| `"delete"` | `CodeMode::Action::"Delete"` | `api.post('/deleteBudget')` with `category = "delete"` → Delete |
| `"admin"` | `CodeMode::Action::"Write"` | Admin operations route to Write action |

This is critical for APIs that use POST for read operations (AWS SDK, GraphQL, Cost Coach). Without the category override, all POST calls would route to the Write action and miss Read Blocklist policies.

**Tier 2: HTTP method fallback (when no registry entry matches)**

When a path has no `[[code_mode.operations]]` entry, the SDK falls back to HTTP method classification:

| HTTP Method | Cedar Action |
|-------------|-------------|
| GET, HEAD, OPTIONS | `Read` |
| POST, PUT, PATCH | `Write` |
| DELETE | `Delete` |

**Script-level action** is determined by aggregating all API calls in the script:

| Script Classification | Condition | `action()` |
|-----------------------|-----------|-----------|
| `read_only` | Only read calls | `Read` |
| `write_only` | Only write/delete calls, no deletes | `Write` |
| `write_only` + deletes | Has delete calls | `Delete` |
| `mixed` | Both read and write/delete calls | `Write` |
| `empty` | No API calls | `Read` |

### Config File Resolution

`cargo pmcp deploy` finds the config file using this resolution order:

1. `config.toml` in the server crate root
2. Single `.toml` file in `instances/` directory

The same file the server embeds via `include_str!()` in `main.rs`.

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
- **Both GraphQL and JavaScript** validation call their respective policy evaluation methods (`evaluate_operation` / `evaluate_script`) — fail-closed on policy errors
- Cedar support via `cedar` feature flag (local evaluation, no network)
- AVP (AWS Verified Permissions) support via external evaluator — policies configured in pmcp.run admin UI
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

## Breaking Changes in v0.3.0

### JavaScript derive macro now calls async validation with policy enforcement

`#[derive(CodeMode)]` with `language = "javascript"` now calls `validate_javascript_code_async` instead of the sync `validate_javascript_code`. This means:

- **Cedar policies are now enforced** for JavaScript servers using the derive macro
- **AVP policies are now enforced** when deployed with `POLICY_STORE_ID` on pmcp.run
- Policy evaluation failures are **fail-closed** (same as GraphQL) — a policy backend outage blocks requests rather than silently allowing them

If your JavaScript server was relying on the absence of policy enforcement (e.g., using a custom `PolicyEvaluator` that only implemented `evaluate_operation` but not `evaluate_script`), the default `evaluate_script` implementation **denies all scripts**. Override `evaluate_script` in your evaluator to allow scripts, or use `NoopPolicyEvaluator` for testing.

### Standard adapters added

`JsCodeExecutor`, `SdkCodeExecutor`, and `McpCodeExecutor` are new. They don't break existing code, but if you were manually implementing `CodeExecutor` for JS plan execution, you can now replace ~75 lines of boilerplate with:

```rust
let code_executor = Arc::new(JsCodeExecutor::new(http_client, ExecutionConfig::default()));
```

See [CHANGELOG.md](./CHANGELOG.md) for the full list of changes.

## Known Limitations (v0.1.0)

1. **`TokenSecret::new` does not zeroize the source `Vec`.** The bytes are copied into `SecretBox` but the original `Vec` is not zeroed. Use `TokenSecret::from_env()` in production for maximum security.

2. **GraphQL only in default features.** JavaScript/OpenAPI validation requires the `openapi-code-mode` feature flag and pulls in SWC (~25MB compile artifact).

3. **No server-side token revocation.** Tokens are stateless (verified by HMAC). Once issued, a token is valid until it expires. Short TTL (5 min default) mitigates this.

4. **SQL and MCP validators are stub.** The `validate_sql_query` and `validate_mcp_composition` methods require their respective feature flags. These validators are being implemented — the derive macro dispatch is ready.

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

- **Standard adapters** — does `JsCodeExecutor`/`SdkCodeExecutor`/`McpCodeExecutor` cover your execution pattern, or do you need a different adapter shape?
- **Variables forwarding** — the adapters pass `variables` as the `args` variable in JS plans. Does your server need a different variable binding strategy?
- **Derive macro ergonomics** — are the fixed field names (`code_mode_config`, `token_secret`, etc.) workable, or do you need attribute-based field mapping?
- **`context_from` pattern** — does returning `ValidationContext` from a sync method work for your auth integration, or do you need an async version?
- **SQL validation** — what SQL dialects do you need? Parameterized queries, prepared statements, or raw SQL only?
- **MCP composition** — what should `validate_mcp_composition` check? Schema compatibility, tool existence, or structural validation?
- **Policy evaluation** — any use cases beyond Cedar and AVP?

File issues or discuss in the `#pmcp-sdk` channel.

## License

MIT
