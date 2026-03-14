//! Guided workflow prompt handlers.
//!
//! Each struct implements `PromptHandler` with metadata describing its
//! name, description, and argument schema. Prompts return actionable
//! guidance for common MCP development scenarios.

use async_trait::async_trait;
use pmcp::types::{
    Content, GetPromptResult, PromptArgument, PromptInfo, PromptMessage, Role,
};
use pmcp::RequestHandlerExtra;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Build a single-message assistant prompt result.
fn assistant_result(description: &str, text: String) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage {
            role: Role::Assistant,
            content: Content::Text { text },
        }],
        Some(description.to_string()),
    ))
}

/// Build a prompt argument descriptor.
fn arg(name: &str, description: &str, required: bool) -> PromptArgument {
    PromptArgument {
        name: name.to_string(),
        description: Some(description.to_string()),
        required,
        completion: None,
        arg_type: None,
    }
}

// ---------------------------------------------------------------------------
// 1. QuickstartPrompt
// ---------------------------------------------------------------------------

/// Step-by-step guide to create a first PMCP server.
pub struct QuickstartPrompt;

#[async_trait]
impl pmcp::server::PromptHandler for QuickstartPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        assistant_result(
            "Quickstart guide for building your first PMCP server",
            QUICKSTART_CONTENT.to_string(),
        )
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "quickstart".to_string(),
            description: Some(
                "Step-by-step guide to create your first PMCP MCP server".to_string(),
            ),
            arguments: None,
        })
    }
}

const QUICKSTART_CONTENT: &str = "\
# PMCP Quickstart

## 1. Create a new project

```bash
cargo pmcp init my-server
cd my-server
```

## 2. Add a tool

Open `src/main.rs` and add a typed tool:

```rust
use pmcp::server::TypedSyncTool;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GreetInput { name: String }

struct GreetTool;

impl TypedSyncTool for GreetTool {
    type Input = GreetInput;
    fn metadata(&self) -> pmcp::types::ToolInfo {
        pmcp::types::ToolInfo::new(\"greet\", \"Greet someone\")
    }
    fn call_sync(&self, input: Self::Input, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<pmcp::CallToolResult> {
        Ok(pmcp::CallToolResult::text(format!(\"Hello, {}!\", input.name)))
    }
}
```

## 3. Register and run

```rust
let server = pmcp::Server::builder()
    .name(\"my-server\")
    .version(\"0.1.0\")
    .tool_typed(GreetTool)
    .build()?;
```

## 4. Test

```bash
cargo pmcp test check http://localhost:8080
```";

// ---------------------------------------------------------------------------
// 2. CreateMcpServerPrompt
// ---------------------------------------------------------------------------

/// Guided workspace setup for a new MCP server.
pub struct CreateMcpServerPrompt;

#[async_trait]
impl pmcp::server::PromptHandler for CreateMcpServerPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let name = args
            .get("name")
            .ok_or_else(|| pmcp::Error::validation("Required argument 'name' is missing"))?;
        let template = args
            .get("template")
            .map(String::as_str)
            .unwrap_or("minimal");
        let text = format!(
            "# Create MCP Server: {name}\n\n\
             ## Setup\n\n\
             ```bash\n\
             cargo pmcp init {name} --template {template}\n\
             cd {name}\n\
             ```\n\n\
             ## Project structure\n\n\
             ```\n\
             {name}/\n\
               Cargo.toml\n\
               src/\n\
                 main.rs       # Server entry point\n\
                 tools/        # Tool implementations\n\
                 resources/    # Resource handlers\n\
                 prompts/      # Prompt handlers\n\
             ```\n\n\
             ## Next steps\n\n\
             1. Add tools with `cargo pmcp scaffold tool <name>`\n\
             2. Run with `cargo run`\n\
             3. Test with `cargo pmcp test check http://localhost:8080`\n"
        );
        assistant_result("Workspace setup instructions", text)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "create-mcp-server".to_string(),
            description: Some("Set up a new PMCP MCP server workspace".to_string()),
            arguments: Some(vec![
                arg("name", "Server project name", true),
                arg(
                    "template",
                    "Template: minimal or calculator (default: minimal)",
                    false,
                ),
            ]),
        })
    }
}

// ---------------------------------------------------------------------------
// 3. AddToolPrompt
// ---------------------------------------------------------------------------

/// Guide to adding a new tool to an existing server.
pub struct AddToolPrompt;

#[async_trait]
impl pmcp::server::PromptHandler for AddToolPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let tool_name = args.get("tool_name").ok_or_else(|| {
            pmcp::Error::validation("Required argument 'tool_name' is missing")
        })?;
        let desc = args
            .get("description")
            .cloned()
            .unwrap_or_else(|| format!("A {tool_name} tool"));
        let text = format!(
            "# Add Tool: {tool_name}\n\n\
             ## 1. Create the tool struct\n\n\
             ```rust\n\
             use pmcp::server::TypedTool;\n\
             use serde::Deserialize;\n\n\
             #[derive(Debug, Deserialize)]\n\
             struct {pascal}Input {{\n\
                 // Add input fields here\n\
             }}\n\n\
             struct {pascal}Tool;\n\n\
             #[async_trait::async_trait]\n\
             impl TypedTool for {pascal}Tool {{\n\
                 type Input = {pascal}Input;\n\n\
                 fn metadata(&self) -> pmcp::types::ToolInfo {{\n\
                     pmcp::types::ToolInfo::new(\"{tool_name}\", \"{desc}\")\n\
                 }}\n\n\
                 async fn call(&self, input: Self::Input, _extra: pmcp::RequestHandlerExtra)\n\
                     -> pmcp::Result<pmcp::CallToolResult> {{\n\
                     // Implement tool logic\n\
                     Ok(pmcp::CallToolResult::text(\"result\"))\n\
                 }}\n\
             }}\n\
             ```\n\n\
             ## 2. Register\n\n\
             ```rust\n\
             server_builder.tool_typed({pascal}Tool);\n\
             ```\n\n\
             ## 3. Test\n\n\
             ```bash\n\
             cargo pmcp test check http://localhost:8080\n\
             ```\n",
            pascal = crate::util::to_pascal_case(tool_name),
        );
        assistant_result("Tool creation guide", text)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "add-tool".to_string(),
            description: Some("Add a new tool to an existing PMCP server".to_string()),
            arguments: Some(vec![
                arg("tool_name", "Name for the new tool (snake_case)", true),
                arg("description", "Human-readable tool description", false),
            ]),
        })
    }
}

// ---------------------------------------------------------------------------
// 4. DiagnosePrompt
// ---------------------------------------------------------------------------

/// Diagnostic steps for a running MCP server.
pub struct DiagnosePrompt;

#[async_trait]
impl pmcp::server::PromptHandler for DiagnosePrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let server_url = args.get("server_url").ok_or_else(|| {
            pmcp::Error::validation("Required argument 'server_url' is missing")
        })?;
        let text = format!(
            "# Diagnose Server: {server_url}\n\n\
             ## Step 1: Check connectivity\n\n\
             Use the `test_check` tool to run protocol compliance checks:\n\n\
             ```\n\
             test_check(url: \"{server_url}\")\n\
             ```\n\n\
             ## Step 2: Verify tool listing\n\n\
             The test_check tool will validate:\n\
             - Server responds to `initialize`\n\
             - `tools/list` returns valid tool schemas\n\
             - `resources/list` returns valid resource info\n\
             - `prompts/list` returns valid prompt metadata\n\n\
             ## Step 3: Common issues\n\n\
             - **Connection refused**: Server not running or wrong port\n\
             - **Timeout**: Server is slow to respond, check for blocking operations\n\
             - **Invalid schema**: Tool input schemas must be valid JSON Schema\n\
             - **Missing capabilities**: Ensure server advertises the capabilities it supports\n\n\
             ## Step 4: Generate test scenarios\n\n\
             ```\n\
             test_generate(url: \"{server_url}\")\n\
             ```\n\n\
             This generates test cases based on the server's actual tool/resource listing.\n"
        );
        assistant_result("Server diagnostic steps", text)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "diagnose".to_string(),
            description: Some(
                "Diagnose issues with a running MCP server".to_string(),
            ),
            arguments: Some(vec![arg(
                "server_url",
                "URL of the MCP server to diagnose",
                true,
            )]),
        })
    }
}

// ---------------------------------------------------------------------------
// 5. SetupAuthPrompt
// ---------------------------------------------------------------------------

/// Auth configuration guide.
pub struct SetupAuthPrompt;

#[async_trait]
impl pmcp::server::PromptHandler for SetupAuthPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let auth_type = args
            .get("auth_type")
            .map(String::as_str)
            .unwrap_or("oauth");
        let text = match auth_type {
            "api-key" => AUTH_API_KEY_CONTENT.to_string(),
            "jwt" => AUTH_JWT_CONTENT.to_string(),
            _ => AUTH_OAUTH_CONTENT.to_string(),
        };
        assistant_result("Authentication setup guide", text)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "setup-auth".to_string(),
            description: Some("Configure authentication for your MCP server".to_string()),
            arguments: Some(vec![arg(
                "auth_type",
                "Auth type: oauth, api-key, or jwt (default: oauth)",
                false,
            )]),
        })
    }
}

const AUTH_OAUTH_CONTENT: &str = "\
# OAuth 2.0 Setup

## 1. Add OAuth middleware

```rust
use pmcp::server::auth::{OAuthMiddleware, OAuthConfig};

let config = OAuthConfig {
    issuer_url: \"https://auth.example.com\".into(),
    audience: Some(\"my-api\".into()),
    ..Default::default()
};

server_builder.middleware(OAuthMiddleware::new(config));
```

## 2. Configure your OAuth provider

Set up a client application in your OAuth provider (Auth0, Okta, etc.)
and note the issuer URL and audience.

## 3. Test with a token

```bash
curl -H 'Authorization: Bearer <token>' http://localhost:8080/mcp
```";

const AUTH_API_KEY_CONTENT: &str = "\
# API Key Setup

## 1. Add API key middleware

```rust
use pmcp::server::auth::ApiKeyMiddleware;

let middleware = ApiKeyMiddleware::new(\"X-API-Key\", vec![
    \"sk-your-key-here\".into(),
]);
server_builder.middleware(middleware);
```

## 2. Store keys securely

```bash
cargo pmcp secret set API_KEY=sk-your-key-here
```

## 3. Test

```bash
curl -H 'X-API-Key: sk-your-key-here' http://localhost:8080/mcp
```";

const AUTH_JWT_CONTENT: &str = "\
# JWT Setup

## 1. Add JWT middleware

```rust
use pmcp::server::auth::JwtMiddleware;

let jwt = JwtMiddleware::builder()
    .issuer(\"https://auth.example.com\")
    .audience(\"my-api\")
    .jwks_url(\"https://auth.example.com/.well-known/jwks.json\")
    .build()?;
server_builder.middleware(jwt);
```

## 2. Token validation

The middleware validates:
- Token signature (via JWKS)
- Issuer claim
- Audience claim
- Expiration

## 3. Test

```bash
curl -H 'Authorization: Bearer <jwt-token>' http://localhost:8080/mcp
```";

// ---------------------------------------------------------------------------
// 6. DebugProtocolErrorPrompt
// ---------------------------------------------------------------------------

/// Protocol error debugging steps.
pub struct DebugProtocolErrorPrompt;

#[async_trait]
impl pmcp::server::PromptHandler for DebugProtocolErrorPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let error_msg = args
            .get("error_message")
            .cloned()
            .unwrap_or_else(|| "(no error message provided)".into());
        let text = format!(
            "# Debug Protocol Error\n\n\
             **Error:** {error_msg}\n\n\
             ## Common Causes\n\n\
             ### JSON-RPC Errors\n\
             - **-32700 Parse Error**: Invalid JSON in request body\n\
             - **-32600 Invalid Request**: Missing `jsonrpc`, `method`, or `id` fields\n\
             - **-32601 Method Not Found**: Calling an unregistered method\n\
             - **-32602 Invalid Params**: Tool input doesn't match schema\n\
             - **-32603 Internal Error**: Server threw an unhandled exception\n\n\
             ## Debugging Steps\n\n\
             1. **Enable verbose logging**: Set `RUST_LOG=debug` on the server\n\
             2. **Capture the raw request**: Use `cargo pmcp connect <url>` for interactive inspection\n\
             3. **Validate schemas**: Run `cargo pmcp schema export <url>` to check tool schemas\n\
             4. **Run compliance tests**: Use `cargo pmcp test check <url>` for protocol validation\n\n\
             ## Common Fixes\n\n\
             - Ensure all tool input types derive `Deserialize`\n\
             - Check that `required` fields in JSON Schema match Rust struct fields\n\
             - Verify the server advertises correct capabilities in `initialize` response\n\
             - Check content-type headers (`application/json` for JSON-RPC)\n"
        );
        assistant_result("Protocol error debugging guide", text)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "debug-protocol-error".to_string(),
            description: Some("Debug MCP protocol errors".to_string()),
            arguments: Some(vec![arg(
                "error_message",
                "The error message to diagnose",
                false,
            )]),
        })
    }
}

// ---------------------------------------------------------------------------
// 7. MigratePrompt
// ---------------------------------------------------------------------------

/// Migration guide from TypeScript SDK to PMCP.
pub struct MigratePrompt;

#[async_trait]
impl pmcp::server::PromptHandler for MigratePrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        assistant_result(
            "Migration guide from TypeScript SDK to PMCP",
            MIGRATE_CONTENT.to_string(),
        )
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "migrate".to_string(),
            description: Some(
                "Guide for migrating from TypeScript MCP SDK to PMCP (Rust)".to_string(),
            ),
            arguments: None,
        })
    }
}

const MIGRATE_CONTENT: &str = "\
# Migrate from TypeScript SDK to PMCP

## Concept Mapping

| TypeScript SDK         | PMCP (Rust)                  |
|------------------------|------------------------------|
| `server.tool()`        | `TypedTool` / `TypedSyncTool`|
| `server.resource()`    | `ResourceHandler` trait      |
| `server.prompt()`      | `PromptHandler` trait        |
| `zod` schemas          | `serde::Deserialize` structs |
| `McpError`             | `pmcp::Error` variants       |
| `StdioServerTransport` | `StdioTransport`             |
| `SSEServerTransport`   | `StreamableHttpServer`       |

## Migration Steps

### 1. Create Rust project
```bash
cargo pmcp init my-server
```

### 2. Convert tool definitions
TypeScript:
```typescript
server.tool('greet', { name: z.string() }, async ({ name }) => ({
    content: [{ type: 'text', text: `Hello ${name}` }]
}));
```

Rust:
```rust
#[derive(Deserialize)]
struct GreetInput { name: String }

struct GreetTool;
impl TypedSyncTool for GreetTool {
    type Input = GreetInput;
    fn metadata(&self) -> ToolInfo { ToolInfo::new(\"greet\", \"Greet\") }
    fn call_sync(&self, input: Self::Input, _: RequestHandlerExtra)
        -> pmcp::Result<CallToolResult> {
        Ok(CallToolResult::text(format!(\"Hello {}\", input.name)))
    }
}
```

### 3. Key differences
- Rust uses `serde` for schema generation (no zod equivalent needed)
- Error handling uses `Result<T, pmcp::Error>` instead of thrown exceptions
- Server building uses a builder pattern: `Server::builder().tool_typed(T).build()`
- HTTP transport uses `StreamableHttpServer` (not SSE)";

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::server::PromptHandler;

    #[test]
    fn all_prompts_have_metadata() {
        let quickstart = QuickstartPrompt;
        assert!(quickstart.metadata().is_some());
        assert_eq!(quickstart.metadata().unwrap().name, "quickstart");

        let create = CreateMcpServerPrompt;
        assert!(create.metadata().is_some());
        assert_eq!(create.metadata().unwrap().name, "create-mcp-server");

        let add_tool = AddToolPrompt;
        assert!(add_tool.metadata().is_some());
        assert_eq!(add_tool.metadata().unwrap().name, "add-tool");

        let diagnose = DiagnosePrompt;
        assert!(diagnose.metadata().is_some());
        assert_eq!(diagnose.metadata().unwrap().name, "diagnose");

        let auth = SetupAuthPrompt;
        assert!(auth.metadata().is_some());
        assert_eq!(auth.metadata().unwrap().name, "setup-auth");

        let debug = DebugProtocolErrorPrompt;
        assert!(debug.metadata().is_some());
        assert_eq!(debug.metadata().unwrap().name, "debug-protocol-error");

        let migrate = MigratePrompt;
        assert!(migrate.metadata().is_some());
        assert_eq!(migrate.metadata().unwrap().name, "migrate");
    }

    #[test]
    fn required_args_flagged_correctly() {
        let create = CreateMcpServerPrompt;
        let meta = create.metadata().unwrap();
        let args = meta.arguments.unwrap();
        assert!(args.iter().find(|a| a.name == "name").unwrap().required);
        assert!(
            !args
                .iter()
                .find(|a| a.name == "template")
                .unwrap()
                .required
        );
    }
}
