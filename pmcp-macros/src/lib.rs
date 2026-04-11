// Crate-level rustdoc is sourced from pmcp-macros/README.md via include_str! so
// that docs.rs and GitHub render from a single authoritative source. Every
// `rust,no_run` code block inside the README is compiled as a doctest under
// `cargo test --doc -p pmcp-macros`, which catches API drift automatically
// (no more silent staleness in the README).
#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, ItemImpl};

mod mcp_common;
mod mcp_prompt;
mod mcp_resource;
mod mcp_server;
mod mcp_tool;
#[allow(dead_code)]
mod utils;

// Phase 66 Plan 01 (POC gate): note on the missing `ReadmeDoctests` struct.
//
// The rustdoc book recommends this pattern for making README doctests
// executable under `cargo test --doc`:
//
//     #[cfg(doctest)]
//     #[doc = include_str!("../README.md")]
//     pub struct ReadmeDoctests;
//
// That pattern does NOT compile in a `proc-macro = true` crate. rustc 1.94
// rejects it with:
//     error: `proc-macro` crate types currently cannot export any items
//     other than functions tagged with `#[proc_macro]`,
//     `#[proc_macro_derive]`, or `#[proc_macro_attribute]`
//
// Discovered during the POC gate for Phase 66. The crate-level
// `#![doc = include_str!("../POC_README.md")]` attribute at the top of this
// file is sufficient on its own: the included file's `rust,no_run` code
// blocks are picked up as doctests attached to the crate root.
//
// This comment is intentionally preserved as a breadcrumb so future
// contributors do not re-introduce the struct expecting it to "make doctests
// work on proc-macro crates". It does not. The crate-level attribute does.

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
    mcp_tool::expand_mcp_tool(args.into(), &input)
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

/// Defines a prompt handler with automatic argument schema generation.
///
/// Generates a struct implementing `PromptHandler` from an annotated standalone
/// async or sync function. Eliminates boilerplate and provides automatic
/// argument schema generation from `JsonSchema` and `State<T>` injection.
///
/// # Attributes
///
/// - `description` - Prompt description (required, enforced at compile time)
/// - `name` - Override prompt name (defaults to function name)
///
/// # Examples
///
/// ```rust,ignore
/// #[mcp_prompt(description = "Review code for quality issues")]
/// async fn code_review(args: ReviewArgs) -> Result<GetPromptResult> {
///     Ok(GetPromptResult::new(
///         vec![PromptMessage::user(Content::text(format!("Review {}", args.language)))],
///         None,
///     ))
/// }
///
/// // Register: server_builder.prompt("code_review", code_review())
/// ```
///
/// With state injection:
///
/// ```rust,ignore
/// #[mcp_prompt(description = "Suggest improvements")]
/// async fn suggest(args: SuggestArgs, db: State<Database>) -> Result<GetPromptResult> {
///     let context = db.get_context(&args.topic).await?;
///     Ok(GetPromptResult::new(vec![PromptMessage::user(Content::text(context))], None))
/// }
///
/// // Register: server_builder.prompt("suggest", suggest().with_state(shared_db))
/// ```
#[proc_macro_attribute]
pub fn mcp_prompt(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_prompt::expand_mcp_prompt(args.into(), &input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Define a resource provider with automatic URI template matching.
///
/// Generates a struct implementing `DynamicResourceProvider` from a function.
/// URI template variables are extracted and passed as `String` parameters.
///
/// # Attributes
///
/// - `uri` (required) — URI or URI template (e.g., `"docs://{topic}"`)
/// - `description` (required) — Human-readable description
/// - `name` (optional) — Override resource name (defaults to function name)
/// - `mime_type` (optional) — MIME type (defaults to `"text/plain"`)
///
/// # Examples
///
/// ```rust,ignore
/// #[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
/// async fn read_doc(topic: String) -> Result<String> {
///     tokio::fs::read_to_string(format!("docs/{topic}.md")).await.map_err(Into::into)
/// }
///
/// // Register via ResourceCollection:
/// // .add_dynamic_provider(Arc::new(read_doc()))
/// ```
#[proc_macro_attribute]
pub fn mcp_resource(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_resource::expand_mcp_resource(args.into(), &input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
