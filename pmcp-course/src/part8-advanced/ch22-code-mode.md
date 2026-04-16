# Code Mode: Validated LLM Code Execution

Code Mode is the PMCP SDK's framework for safely executing LLM-generated code against your backend — with validation, policy enforcement, and cryptographic approval tokens.

## Learning Objectives

After this chapter, you will be able to:

1. Add Code Mode to any MCP server using `#[derive(CodeMode)]`
2. Configure operation policies for your target system (GraphQL, JS/OpenAPI, SQL, MCP)
3. Declare operations in `config.toml` for platform-level policy management
4. Choose the right executor adapter for your backend pattern

## Why Code Mode?

When an LLM generates a GraphQL query, JavaScript plan, or SQL statement, three questions arise:

1. **Is it safe?** — Does it access sensitive data? Does it modify production state?
2. **Is it authorized?** — Does this user have permission for this operation?
3. **Is it the same code the user approved?** — Can we prove tamper-free execution?

Code Mode answers all three with a pipeline: **parse -> policy check -> explain -> HMAC sign -> user approves -> verify token -> execute**.

## Adding Code Mode to Your Server

### Step 1: Add Dependencies

```toml
[dependencies]
pmcp = "2.3.0"
pmcp-code-mode = "0.2.0"
pmcp-code-mode-derive = "0.1.0"
```

### Step 2: Derive and Configure

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
        ValidationContext::new("user-123", "session-456", "schema-v1", "perms-v1")
    }
}
```

The `language` attribute selects the validation path at compile time:

| Language | Value | Feature |
|----------|-------|---------|
| GraphQL | `"graphql"` | *(default)* |
| JavaScript/OpenAPI | `"javascript"` | `openapi-code-mode` |
| SQL | `"sql"` | `sql-code-mode` |
| MCP composition | `"mcp"` | `mcp-code-mode` |

### Step 3: Choose Your Executor

**Direct implementation** (GraphQL, SQL):

```rust,ignore
#[async_trait]
impl CodeExecutor for MyGraphQLExecutor {
    async fn execute(&self, code: &str, variables: Option<&Value>) -> Result<Value, ExecutionError> {
        let result = self.pool.execute_graphql(code, variables).await?;
        Ok(serde_json::to_value(result)?)
    }
}
```

**Standard adapters** (JavaScript, SDK, MCP):

```rust,ignore
// JS + HTTP (e.g., Cost Coach calling REST APIs)
let executor = Arc::new(JsCodeExecutor::new(http_client, ExecutionConfig::default()));

// JS + AWS SDK (e.g., direct Cost Explorer SDK calls)
let executor = Arc::new(SdkCodeExecutor::new(sdk_client, ExecutionConfig::default()));

// MCP tool composition (routing to other MCP servers)
let executor = Arc::new(McpCodeExecutor::new(mcp_router, ExecutionConfig::default()));
```

### Step 4: Register and Build

```rust,ignore
let server = Arc::new(MyServer { /* ... */ });
let builder = server.register_code_mode_tools(pmcp::Server::builder())?;
// builder now has validate_code + execute_code tools
```

## Declaring Operations in config.toml

The `config.toml` file declares what operations your server supports. When deployed via `cargo pmcp deploy`, this file is included in the deploy ZIP and extracted by the pmcp.run platform to populate the Code Mode policy page.

### OpenAPI Example

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

### GraphQL Example

```toml
[server]
name = "open-images"
type = "graphql-api"

[code_mode]
allow_writes = false

[[code_mode.operations]]
name = "searchImages"
operation_type = "query"

[[code_mode.operations]]
name = "deleteImage"
operation_type = "mutation"
destructive_hint = true
```

### SQL Example

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

### Categorization

Operations are sorted into **read**, **write**, **delete**, and **admin** categories based on:

- **OpenAPI:** HTTP method (GET=read, POST/PUT=write, DELETE=delete)
- **GraphQL:** operation_type (query=read, mutation=write/delete)
- **SQL:** statement type + `allow_writes`/`allow_deletes` settings
- **MCP-API:** tool annotations + name pattern matching

The `operation_category` field overrides automatic categorization when you need explicit control.

## Policy Enforcement

### Runtime (CodeModeConfig)

`CodeModeConfig` controls what the validation pipeline allows at runtime — blocklists, allowlists, depth limits, field restrictions:

```rust,ignore
let config = CodeModeConfig {
    enabled: true,
    allow_mutations: false,
    blocked_fields: HashSet::from(["User.ssn".into()]),
    max_query_depth: 10,
    token_ttl_seconds: 300,
    ..CodeModeConfig::enabled()
};
```

### Platform (config.toml)

`config.toml` declares operations for platform-level management. Administrators can enable/disable individual operations in the pmcp.run admin UI without redeploying the server.

### Authorization (PolicyEvaluator)

For fine-grained per-user authorization, implement `PolicyEvaluator` with Cedar or AWS Verified Permissions:

```rust,ignore
// Cedar: local policy evaluation (no network)
let evaluator = Arc::new(CedarPolicyEvaluator::new(policy_set));

// Custom: your authorization backend
let evaluator = Arc::new(MyAuthzBackend::new(authz_client));
```

## Key Takeaways

1. `#[derive(CodeMode)]` + `language` attribute = zero-boilerplate Code Mode for any language
2. `config.toml` declares operations for platform-level policy management
3. Standard adapters (`JsCodeExecutor`, `SdkCodeExecutor`, `McpCodeExecutor`) bridge execution traits
4. HMAC tokens cryptographically bind validated code to user, session, and schema
5. Three layers of policy: runtime (`CodeModeConfig`), platform (`config.toml`), authorization (`PolicyEvaluator`)
