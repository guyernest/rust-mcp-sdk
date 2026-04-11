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

// Note on the missing `ReadmeDoctests` struct.
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
// Discovered during the Phase 66 POC gate. The crate-level
// `#![doc = include_str!("../README.md")]` attribute at the top of this file
// is sufficient on its own: the included file's `rust,no_run` code blocks
// are picked up as doctests attached to the crate root.
//
// This comment is intentionally preserved as a breadcrumb so future
// contributors do not re-introduce the struct expecting it to "make doctests
// work on proc-macro crates". It does not. The crate-level attribute does.

/// Defines an MCP tool with automatic schema generation and state injection.
///
/// Generates a struct implementing `ToolHandler` from an annotated standalone
/// async or sync function. Eliminates `Box::pin` boilerplate and provides
/// automatic input/output schema generation, `State<T>` injection, and MCP
/// annotation support.
///
/// # Attributes
///
/// - `description = "..."` — required. Human-readable description enforced at
///   compile time.
/// - `name = "..."` — optional. Overrides the tool name (defaults to the
///   function name).
/// - `annotations(...)` — optional. MCP standard annotations: `read_only`,
///   `destructive`, `idempotent`, `open_world`.
/// - `ui = "..."` — optional. Widget resource URI for MCP Apps integrations.
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::mcp_tool;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct AddArgs { a: f64, b: f64 }
///
/// #[derive(Debug, Serialize, JsonSchema)]
/// struct AddResult { sum: f64 }
///
/// #[mcp_tool(description = "Add two numbers")]
/// async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
///     Ok(AddResult { sum: args.a + args.b })
/// }
///
/// // Register with: ServerBuilder::new().tool("add", add())
/// ```
///
/// See `examples/s23_mcp_tool_macro.rs` for a complete runnable demo covering
/// `State<T>` injection, sync tools, annotations, and impl-block registration.
#[proc_macro_attribute]
pub fn mcp_tool(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_tool::expand_mcp_tool(args.into(), &input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Collects `#[mcp_tool]` and `#[mcp_prompt]` methods from an impl block and
/// generates handlers plus bulk registration.
///
/// Processes an impl block to find all methods annotated with `#[mcp_tool(...)]`
/// or `#[mcp_prompt(...)]`, generates per-method handler structs sharing an
/// `Arc<ServerType>` for `&self` access, and emits `impl McpServer for ServerType`
/// so `ServerBuilder::mcp_server(...)` can register everything at once.
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::{mcp_server, mcp_tool};
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// struct Calculator;
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct AddArgs { a: f64, b: f64 }
///
/// #[derive(Debug, Serialize, JsonSchema)]
/// struct AddResult { sum: f64 }
///
/// #[mcp_server]
/// impl Calculator {
///     #[mcp_tool(description = "Add two numbers")]
///     async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
///         Ok(AddResult { sum: args.a + args.b })
///     }
/// }
///
/// // Register with: ServerBuilder::new().mcp_server(Calculator)
/// ```
///
/// See `examples/s23_mcp_tool_macro.rs` for a complete runnable demo.
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
/// async or sync function. Eliminates the
/// `HashMap::get("x").ok_or()?.parse()?` boilerplate of hand-rolled prompt
/// handlers, derives argument schemas from `JsonSchema`, and supports
/// `State<T>` injection.
///
/// # Attributes
///
/// - `description = "..."` — required. Human-readable description enforced at
///   compile time.
/// - `name = "..."` — optional. Overrides the prompt name (defaults to the
///   function name).
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::mcp_prompt;
/// use pmcp::types::{Content, GetPromptResult, PromptMessage};
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct ReviewArgs {
///     /// The programming language to review
///     language: String,
/// }
///
/// #[mcp_prompt(description = "Review code for quality issues")]
/// async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
///     Ok(GetPromptResult::new(
///         vec![PromptMessage::user(Content::text(format!(
///             "Review this {} code", args.language,
///         )))],
///         None,
///     ))
/// }
///
/// // Register with: ServerBuilder::new().prompt("code_review", code_review())
/// ```
///
/// See `examples/s24_mcp_prompt_macro.rs` for a complete runnable demo
/// covering `State<T>` injection and impl-block registration.
#[proc_macro_attribute]
pub fn mcp_prompt(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_prompt::expand_mcp_prompt(args.into(), &input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Defines a resource provider with automatic URI template matching.
///
/// Generates a struct implementing `DynamicResourceProvider` from an annotated
/// standalone async or sync function. Every `{variable_name}` placeholder inside
/// the `uri` template is extracted at request time and passed to the function as
/// a `String` parameter of the same name — no hand-rolled URI parsing required.
///
/// # Attributes
///
/// - `uri = "..."` — required. URI or URI template (for example
///   `"docs://{topic}"`).
/// - `description = "..."` — required. Human-readable description.
/// - `name = "..."` — optional. Overrides the resource name (defaults to the
///   function name).
/// - `mime_type = "..."` — optional. Defaults to `"text/plain"`.
///
/// # Example
///
/// ```rust,no_run
/// // Note: direct import until the `mcp_resource` re-export gap is closed.
/// use pmcp_macros::mcp_resource;
///
/// #[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
/// async fn read_doc(topic: String) -> pmcp::Result<String> {
///     Ok(format!("# {topic}\n\nDocumentation content for `{topic}`."))
/// }
///
/// // Register with:
/// // ResourceCollection::new().add_dynamic_provider(Arc::new(read_doc()))
/// ```
///
/// See `examples/s23_mcp_tool_macro.rs` and `examples/s24_mcp_prompt_macro.rs`
/// for the related `#[mcp_tool]` / `#[mcp_prompt]` patterns; a complete runnable
/// resource demo is tracked for a future phase.
#[proc_macro_attribute]
pub fn mcp_resource(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    mcp_resource::expand_mcp_resource(args.into(), &input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
