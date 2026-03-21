//! Procedural macros for PMCP SDK
//!
//! This crate provides attribute macros to reduce boilerplate when implementing
//! MCP servers with tools, prompts, and resources.
//!
//! # Features
//!
//! - `#[tool]` - Define a tool with automatic schema generation
//! - `#[tool_router]` - Collect tools from an impl block
//! - `#[prompt]` - Define a prompt template
//! - `#[resource]` - Define a resource handler
//!
//! # Examples
//!
//! ## Tool Definition
//!
//! ```rust,ignore
//! use pmcp_macros::{tool, tool_router};
//! use serde::{Deserialize, Serialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Debug, Deserialize, JsonSchema)]
//! struct CalculateParams {
//!     a: i32,
//!     b: i32,
//!     operation: String,
//! }
//!
//! #[derive(Debug, Serialize, JsonSchema)]
//! struct CalculateResult {
//!     result: i32,
//! }
//!
//! #[tool_router]
//! impl Calculator {
//!     #[tool(description = "Perform arithmetic operations")]
//!     async fn calculate(&self, params: CalculateParams) -> Result<CalculateResult, String> {
//!         let result = match params.operation.as_str() {
//!             "add" => params.a + params.b,
//!             "subtract" => params.a - params.b,
//!             "multiply" => params.a * params.b,
//!             "divide" => {
//!                 if params.b == 0 {
//!                     return Err("Division by zero".to_string());
//!                 }
//!                 params.a / params.b
//!             }
//!             _ => return Err("Unknown operation".to_string()),
//!         };
//!         Ok(CalculateResult { result })
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, ItemImpl};

mod mcp_common;
mod mcp_server;
mod mcp_tool;
mod tool;
mod tool_router;
#[allow(dead_code)]
mod utils;

/// Defines a tool handler with automatic schema generation.
///
/// # Attributes
///
/// - `name` - Optional tool name (defaults to function name)
/// - `description` - Tool description (required)
/// - `annotations` - Additional metadata for the tool
///
/// # Examples
///
/// ```rust,ignore
/// #[tool(description = "Add two numbers")]
/// async fn add(a: i32, b: i32) -> Result<i32, String> {
///     Ok(a + b)
/// }
/// ```
///
/// With custom name and annotations:
///
/// ```rust,ignore
/// #[tool(
///     name = "math_add",
///     description = "Add two numbers",
///     annotations(category = "math", complexity = "simple")
/// )]
/// async fn add(a: i32, b: i32) -> Result<i32, String> {
///     Ok(a + b)
/// }
/// ```
#[deprecated(since = "0.3.0", note = "Use #[mcp_tool] instead — better DX with State<T> injection, async auto-detection, and mandatory descriptions")]
#[proc_macro_attribute]
pub fn tool(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    tool::expand_tool(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Defines an MCP tool with automatic schema generation and state injection.
///
/// Generates a struct implementing `ToolHandler` from an annotated standalone
/// async or sync function. Eliminates `Box::pin` boilerplate and provides
/// automatic input/output schema generation, `State<T>` injection, and
/// MCP annotation support.
///
/// # Attributes
///
/// - `description` - Tool description (required, enforced at compile time)
/// - `name` - Override tool name (defaults to function name)
/// - `annotations(...)` - MCP standard annotations (`read_only`, `destructive`,
///   `idempotent`, `open_world`)
/// - `ui = "..."` - Widget resource URI for MCP Apps
///
/// # Examples
///
/// ```rust,ignore
/// #[mcp_tool(description = "Add two numbers")]
/// async fn add(args: AddArgs) -> Result<AddResult> {
///     Ok(AddResult { sum: args.a + args.b })
/// }
///
/// // Register: server_builder.tool("add", add())
/// ```
///
/// With state injection:
///
/// ```rust,ignore
/// #[mcp_tool(description = "Query database")]
/// async fn query(args: QueryArgs, db: State<Database>) -> Result<Value> {
///     let rows = db.execute(&args.sql).await?;
///     Ok(json!({ "rows": rows }))
/// }
///
/// // Register: server_builder.tool("query", query().with_state(shared_db))
/// ```
///
/// With annotations:
///
/// ```rust,ignore
/// #[mcp_tool(
///     description = "Delete a record",
///     annotations(destructive = true, idempotent = false),
/// )]
/// async fn delete(args: DeleteArgs) -> Result<Value> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_tool(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_tool::expand_mcp_tool(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Collects `#[mcp_tool]` methods from an impl block and generates tool handlers.
///
/// Processes an impl block to find all methods annotated with `#[mcp_tool(...)]`,
/// generates per-tool `ToolHandler` structs using `Arc<ServerType>` for shared
/// `&self` access, and implements `McpServer` for bulk registration.
///
/// # Examples
///
/// ```rust,ignore
/// #[mcp_server]
/// impl MyServer {
///     #[mcp_tool(description = "Query database")]
///     async fn query(&self, args: QueryArgs) -> Result<QueryResult> {
///         self.db.execute(&args.sql).await
///     }
/// }
///
/// // Register all tools at once:
/// let builder = ServerBuilder::new()
///     .mcp_server(my_server);
/// ```
#[proc_macro_attribute]
pub fn mcp_server(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemImpl);
    mcp_server::expand_mcp_server(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Collects all tool methods from an impl block and generates a router.
///
/// This macro scans an impl block for methods marked with `#[tool]` and
/// automatically generates registration code for them.
///
/// # Examples
///
/// ```rust,ignore
/// #[tool_router]
/// impl MyServer {
///     #[tool(description = "Get current time")]
///     async fn get_time(&self) -> Result<String, Error> {
///         Ok(chrono::Utc::now().to_string())
///     }
///     
///     #[tool(description = "Echo message")]
///     async fn echo(&self, message: String) -> Result<String, Error> {
///         Ok(message)
///     }
/// }
/// ```
///
/// The macro generates:
/// - A `tools()` method returning all tool definitions
/// - A `handle_tool()` method for routing tool calls
/// - Automatic schema generation for parameters
#[proc_macro_attribute]
pub fn tool_router(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemImpl);

    tool_router::expand_tool_router(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Defines a prompt template with typed arguments.
///
/// # Examples
///
/// ```rust,ignore
/// #[prompt(
///     name = "code_review",
///     description = "Review code for quality issues"
/// )]
/// async fn review_code(&self, language: String, code: String) -> Result<String, Error> {
///     Ok(format!("Review this {} code:\n{}", language, code))
/// }
/// ```
#[proc_macro_attribute]
pub fn prompt(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Prompt macro implementation deferred to future release
    input
}

/// Defines a resource handler with URI pattern matching.
///
/// # Examples
///
/// ```rust,ignore
/// #[resource(
///     uri_template = "file:///{path}",
///     mime_type = "text/plain"
/// )]
/// async fn read_file(&self, path: String) -> Result<String, Error> {
///     std::fs::read_to_string(path).map_err(|e| e.into())
/// }
/// ```
#[proc_macro_attribute]
pub fn resource(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Resource macro implementation deferred to future release
    input
}
