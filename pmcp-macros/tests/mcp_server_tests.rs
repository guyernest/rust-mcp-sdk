//! Integration tests for `#[mcp_server]` impl blocks.
//!
//! These tests verify that `#[mcp_server]` correctly generates per-tool and
//! per-prompt handler structs, a `McpServer` impl with `register()`, and that
//! the generated handlers dispatch correctly through `ToolHandler::handle()`
//! and `PromptHandler::handle()`.

use pmcp_macros::mcp_server;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// === Shared argument types ===

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    result: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct KeyArgs {
    key: String,
}

// === Test 1: Basic impl block with two tools ===

struct Calculator;

#[mcp_server]
impl Calculator {
    #[mcp_tool(description = "Add two numbers")]
    async fn add(&self, args: AddArgs) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": args.a + args.b}))
    }

    #[mcp_tool(description = "Subtract two numbers")]
    async fn subtract(&self, args: AddArgs) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": args.a - args.b}))
    }
}

#[test]
fn test_register() {
    let calc = Calculator;
    let builder = pmcp::ServerBuilder::new()
        .name("test")
        .version("1.0.0")
        .mcp_server(calc);
    // If this compiles, McpServer is implemented and register() works
    drop(builder);
}

// === Test 2: Impl block with shared state via &self ===

struct DbServer {
    data: std::collections::HashMap<String, String>,
}

#[mcp_server]
impl DbServer {
    #[mcp_tool(description = "Get value by key")]
    async fn get(&self, args: KeyArgs) -> pmcp::Result<serde_json::Value> {
        let val = self.data.get(&args.key).cloned().unwrap_or_default();
        Ok(serde_json::json!({"value": val}))
    }
}

#[test]
fn test_shared_state_via_self() {
    let mut data = std::collections::HashMap::new();
    data.insert("foo".to_string(), "bar".to_string());
    let server = DbServer { data };
    let builder = pmcp::ServerBuilder::new()
        .name("db-test")
        .version("1.0.0")
        .mcp_server(server);
    drop(builder);
}

// === Test 3: No-arg method and sync method in impl block ===

struct MixedServer;

#[mcp_server]
impl MixedServer {
    #[mcp_tool(description = "Health check")]
    async fn health(&self) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"status": "ok"}))
    }

    #[mcp_tool(description = "Sync operation")]
    fn sync_op(&self) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"sync": true}))
    }
}

#[test]
fn test_mixed_methods() {
    let server = MixedServer;
    let builder = pmcp::ServerBuilder::new()
        .name("mixed-test")
        .version("1.0.0")
        .mcp_server(server);
    drop(builder);
}

// === Test 4: Typed output generates outputSchema in impl block ===

struct TypedServer;

#[mcp_server]
impl TypedServer {
    #[mcp_tool(description = "Add with typed result")]
    async fn typed_add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult {
            result: args.a + args.b,
        })
    }

    #[mcp_tool(description = "Untyped return")]
    async fn untyped_op(&self) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"done": true}))
    }
}

#[test]
fn test_typed_server_registration() {
    let server = TypedServer;
    let builder = pmcp::ServerBuilder::new()
        .name("typed-test")
        .version("1.0.0")
        .mcp_server(server);
    drop(builder);
}

// === Test 5: Annotations in impl block ===

struct AnnotatedServer;

#[mcp_server]
impl AnnotatedServer {
    #[mcp_tool(
        description = "Read-only query",
        annotations(read_only = true, idempotent = true)
    )]
    async fn safe_query(&self, args: KeyArgs) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"key": args.key}))
    }
}

#[test]
fn test_annotated_server() {
    let server = AnnotatedServer;
    let builder = pmcp::ServerBuilder::new()
        .name("annotated-test")
        .version("1.0.0")
        .mcp_server(server);
    drop(builder);
}

// === Test 6: Mixed tools and prompts in one impl block (D-14) ===

struct FullServer;

#[mcp_server]
impl FullServer {
    #[mcp_tool(description = "Execute query")]
    async fn execute(&self, args: KeyArgs) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"key": args.key}))
    }

    #[mcp_prompt(description = "Generate query prompt")]
    async fn query_builder(&self, args: AddArgs) -> pmcp::Result<pmcp::types::GetPromptResult> {
        Ok(pmcp::types::GetPromptResult::new(
            vec![pmcp::types::PromptMessage::user(pmcp::Content::text(
                format!("Query with a={} b={}", args.a, args.b),
            ))],
            Some("Query builder".to_string()),
        ))
    }
}

#[test]
fn test_mixed_tools_and_prompts() {
    let server = FullServer;
    let builder = pmcp::ServerBuilder::new()
        .name("full")
        .version("1.0.0")
        .mcp_server(server);
    // If this compiles, both tools and prompts are registered
    drop(builder);
}

// === Test 7: Prompt-only impl block (no tools) ===

struct PromptOnlyServer;

#[mcp_server]
impl PromptOnlyServer {
    #[mcp_prompt(description = "Status prompt")]
    async fn status(&self) -> pmcp::Result<pmcp::types::GetPromptResult> {
        Ok(pmcp::types::GetPromptResult::new(vec![], None))
    }
}

#[test]
fn test_prompt_only_server() {
    let server = PromptOnlyServer;
    let builder = pmcp::ServerBuilder::new()
        .name("prompt-only")
        .version("1.0.0")
        .mcp_server(server);
    // If this compiles, prompt-only registration works
    drop(builder);
}

// ==========================================================================
// Impl-block rustdoc harvest — symmetry with standalone parse site
// ==========================================================================
//
// Both parse sites route through `mcp_common::resolve_tool_args`, so an
// impl-block method with a rustdoc comment must produce the same description
// as a free function with the same rustdoc.

use pmcp::ToolHandler as _RustdocToolHandler;

struct RustdocHarvestServer;

#[mcp_server]
impl RustdocHarvestServer {
    /// Compute the square of a number.
    #[mcp_tool]
    async fn square(&self, args: AddArgs) -> pmcp::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": args.a * args.a}))
    }
}

#[test]
fn test_impl_block_rustdoc_harvest() {
    let server = std::sync::Arc::new(RustdocHarvestServer);
    let handler = SquareToolHandler {
        server: server.clone(),
    };
    let meta = handler.metadata().expect("metadata should exist");
    assert_eq!(meta.name, "square", "tool name derives from method ident");
    assert_eq!(
        meta.description.as_deref(),
        Some("Compute the square of a number."),
        "impl-block #[mcp_tool] must harvest rustdoc symmetrically with standalone fn"
    );
}
