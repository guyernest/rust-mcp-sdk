# pmcp-code-mode

Code Mode validation and execution framework for MCP servers. Enables LLM-generated queries (GraphQL, JavaScript/OpenAPI) to be validated, explained, and executed with HMAC-signed approval tokens binding code to validation results.

## Architecture

```text
describe_schema() -> LLM generates code -> validate_code() -> user approval -> execute_code()
```

The pipeline ensures that code is parsed, security-analyzed, and policy-checked before receiving an approval token. The token cryptographically binds the exact code hash to the validation result, preventing modification after approval.

## Key Concepts

| Concept | Description |
|---------|-------------|
| `ValidationPipeline` | Orchestrates parse, policy check, security analysis, explanation, and token generation |
| `PolicyEvaluator` | Pluggable trait for authorization backends (Cedar, AWS Verified Permissions, custom) |
| `CodeExecutor` | High-level trait for executing validated code against your backend |
| `TokenSecret` | Zeroizing HMAC secret backed by `secrecy::SecretBox<[u8]>` |
| `HmacTokenGenerator` | SHA-256 HMAC token generator binding code hash + context to approval |
| `ApprovalToken` | Signed token containing code hash, user ID, session ID, expiry, and risk level |

## Quick Start

```rust,ignore
use pmcp_code_mode::{
    CodeModeConfig, TokenSecret, ValidationPipeline, ValidationContext,
};

// Create a validation pipeline with a secret key
let config = CodeModeConfig::enabled();
let secret = TokenSecret::new(b"my-secret-key-at-least-32-bytes!".to_vec());
let pipeline = ValidationPipeline::from_token_secret(config, &secret);

// Validate a GraphQL query
let context = ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash");
let result = pipeline.validate_graphql_query("query { users { id name } }", &context)
    .expect("validation failed");

assert!(result.is_valid);
assert!(result.approval_token.is_some());
```

### With `#[derive(CodeMode)]`

Use the companion [`pmcp-code-mode-derive`](https://crates.io/crates/pmcp-code-mode-derive) crate to generate tool registration automatically:

```rust,ignore
use pmcp_code_mode::{CodeModeConfig, TokenSecret, NoopPolicyEvaluator, CodeExecutor};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

#[derive(CodeMode)]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

// Generated: MyServer::register_code_mode_tools(builder) -> ServerBuilder
let builder = pmcp::Server::builder();
let builder = server.register_code_mode_tools(builder);
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| *(none)* | yes | GraphQL validation only (via `graphql-parser`) |
| `openapi-code-mode` | no | JavaScript/OpenAPI validation (adds SWC parser) |
| `js-runtime` | no | JavaScript AST-based execution runtime (implies `openapi-code-mode`) |
| `mcp-code-mode` | no | MCP-to-MCP executor composition (implies `js-runtime`) |
| `cedar` | no | Local Cedar policy evaluation (adds `cedar-policy 4.9`) |

## Security

See [SECURITY.md](./SECURITY.md) for the full threat model.

Key security properties:
- **TokenSecret**: Backed by `secrecy::SecretBox<[u8]>`, zeroed on drop, no Debug/Display/Clone/Serialize/Deserialize
- **HMAC tokens**: SHA-256 code hash + context bound in HMAC-signed token with TTL expiry
- **Default-deny policy**: `PolicyEvaluator` trait enables pluggable authorization

### NoopPolicyEvaluator Warning

The `NoopPolicyEvaluator` is provided for **testing and local development only**. It allows ALL operations without any policy checks. Production servers MUST implement `PolicyEvaluator` with a real authorization backend (e.g., Cedar, AWS Verified Permissions).

## Part of the PMCP SDK

This crate is part of the [PMCP SDK](https://crates.io/crates/pmcp) workspace and is published to [crates.io](https://crates.io/crates/pmcp-code-mode).

## License

MIT
