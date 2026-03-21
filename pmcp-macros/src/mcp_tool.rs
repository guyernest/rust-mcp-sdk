//! `#[mcp_tool]` attribute macro expansion.
//!
//! Generates a struct implementing `ToolHandler` from an annotated standalone
//! async or sync function, eliminating `Box::pin` boilerplate and providing
//! automatic schema generation, state injection, and annotation support.
//!
//! # Generated Code
//!
//! For each annotated function, the macro generates:
//! 1. The original function (preserved unchanged)
//! 2. A `{PascalCase(fn_name)}Tool` struct implementing `ToolHandler`
//! 3. A constructor function `fn fn_name() -> StructName` for ergonomic registration
//!
//! # Example
//!
//! ```rust,ignore
//! #[mcp_tool(description = "Add two numbers")]
//! async fn add(args: AddArgs) -> Result<AddResult> {
//!     Ok(AddResult { sum: args.a + args.b })
//! }
//!
//! // Register: server_builder.tool("add", add())
//! ```

use crate::mcp_common;
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{ItemFn, ReturnType, Type};

/// Parsed attributes for `#[mcp_tool(...)]`.
#[derive(Debug, FromMeta)]
pub struct McpToolArgs {
    /// Tool description (mandatory per D-05).
    description: String,
    /// Override tool name (defaults to function name per D-06).
    #[darling(default)]
    name: Option<String>,
    /// MCP standard annotations (per D-23).
    #[darling(default)]
    annotations: Option<McpToolAnnotations>,
    /// UI widget resource URI (per D-24).
    #[darling(default)]
    ui: Option<syn::Expr>,
}

/// MCP standard tool annotations parsed from macro attributes.
#[derive(Debug, Default, FromMeta)]
pub struct McpToolAnnotations {
    /// If true, the tool does not modify any state.
    #[darling(default)]
    read_only: Option<bool>,
    /// If true, the tool may perform destructive operations.
    #[darling(default)]
    destructive: Option<bool>,
    /// If true, calling the tool multiple times with same args has same effect.
    #[darling(default)]
    idempotent: Option<bool>,
    /// If true, the tool interacts with external systems.
    #[darling(default)]
    open_world: Option<bool>,
}

/// Expand `#[mcp_tool]` attribute macro on a standalone function.
pub fn expand_mcp_tool(args: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
    // Parse macro attributes via darling.
    let nested_metas = if args.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig.ident,
            "mcp_tool requires at least `description = \"...\"` attribute",
        ));
    } else {
        let parser = syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
        parser
            .parse2(args)
            .map(|p| p.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
    };

    let macro_args = McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;

    // Extract function info.
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let tool_name = macro_args.name.unwrap_or_else(|| fn_name_str.clone());
    let is_async = input.sig.asyncness.is_some();
    let struct_name = format_ident!("{}Tool", fn_name_str.to_upper_camel_case());
    let description = &macro_args.description;

    // Classify all parameters.
    let mut args_type: Option<Type> = None;
    let mut state_inner_ty: Option<Type> = None;
    let mut has_extra = false;

    // Track parameter order for correct call-site argument passing.
    // Each entry is one of: "args", "state", "extra".
    let mut param_order: Vec<&str> = Vec::new();

    for param in &input.sig.inputs {
        let role = mcp_common::classify_param(param)?;
        match role {
            mcp_common::ParamRole::Args(ty) => {
                if args_type.is_some() {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_tool functions can have at most one args parameter",
                    ));
                }
                args_type = Some(ty);
                param_order.push("args");
            }
            mcp_common::ParamRole::State { inner_ty, .. } => {
                if state_inner_ty.is_some() {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_tool functions can have at most one State<T> parameter",
                    ));
                }
                state_inner_ty = Some(inner_ty);
                param_order.push("state");
            }
            mcp_common::ParamRole::Extra => {
                if has_extra {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_tool functions can have at most one RequestHandlerExtra parameter",
                    ));
                }
                has_extra = true;
                param_order.push("extra");
            }
            mcp_common::ParamRole::SelfRef => {
                return Err(syn::Error::new_spanned(
                    param,
                    "standalone #[mcp_tool] functions cannot have &self — use #[mcp_server] for impl block tools",
                ));
            }
        }
    }

    // Generate struct fields.
    let struct_fields = if state_inner_ty.is_some() {
        let inner = state_inner_ty.as_ref().unwrap();
        quote! { state: Option<std::sync::Arc<#inner>>, }
    } else {
        quote! {}
    };

    // Generate with_state method (only if state param present).
    let with_state_method = if let Some(ref inner) = state_inner_ty {
        quote! {
            /// Provide shared state for this tool.
            ///
            /// Call this at registration time to inject state:
            /// ```rust,ignore
            /// server_builder.tool("name", tool_fn().with_state(my_state))
            /// ```
            pub fn with_state(mut self, state: impl Into<std::sync::Arc<#inner>>) -> Self {
                self.state = Some(state.into());
                self
            }
        }
    } else {
        quote! {}
    };

    // Generate constructor default.
    let constructor_default = if state_inner_ty.is_some() {
        quote! { #struct_name { state: None } }
    } else {
        quote! { #struct_name {} }
    };

    // Generate args deserialization in handle().
    let args_deser = if let Some(ref at) = args_type {
        let tool_name_for_err = &tool_name;
        quote! {
            let typed_args: #at = serde_json::from_value(args)
                .map_err(|e| pmcp::Error::invalid_params(
                    format!("Invalid arguments for tool '{}': {}", #tool_name_for_err, e)
                ))?;
        }
    } else {
        quote! {}
    };

    // Generate state resolution in handle().
    let state_resolution = if let Some(ref inner) = state_inner_ty {
        let inner_name = quote!(#inner).to_string();
        let tool_name_for_err = &tool_name;
        quote! {
            let state_val = pmcp::State(
                self.state.as_ref()
                    .unwrap_or_else(|| panic!(
                        "State<{}> not provided for tool '{}' -- call .with_state() during registration",
                        #inner_name, #tool_name_for_err
                    ))
                    .clone()
            );
        }
    } else {
        quote! {}
    };

    // Generate extra parameter name in handle() signature.
    let extra_param_name: Ident = if has_extra {
        format_ident!("extra")
    } else {
        format_ident!("_extra")
    };

    // Generate function call arguments in correct parameter order.
    let call_args: Vec<TokenStream> = param_order
        .iter()
        .map(|role| match *role {
            "args" => quote! { typed_args },
            "state" => quote! { state_val },
            "extra" => quote! { #extra_param_name },
            _ => unreachable!(),
        })
        .collect();

    // Generate the function call (async vs sync).
    let fn_call = if is_async {
        quote! { let result = #fn_name(#(#call_args),*).await?; }
    } else {
        quote! { let result = #fn_name(#(#call_args),*)?; }
    };

    // Generate result serialization.
    let result_serialize = quote! {
        serde_json::to_value(result)
            .map_err(|e| pmcp::Error::internal(format!("Failed to serialize result: {}", e)))
    };

    // Generate handle body (wrapped in async for sync functions too since ToolHandler is async).
    let handle_body = quote! {
        #args_deser
        #state_resolution
        #fn_call
        #result_serialize
    };

    // Generate input schema code for metadata().
    let input_schema_code = if let Some(ref at) = args_type {
        mcp_common::generate_input_schema_code(at)
    } else {
        mcp_common::generate_empty_schema_code()
    };

    // Generate output schema code for metadata().
    // Extract return type and check if it's Result<T> where T is not Value.
    let output_schema_code = extract_output_schema_code(&input)?;

    // Generate ToolInfo construction (branching on annotations presence).
    let tool_info_code =
        generate_tool_info_code(&tool_name, description, &macro_args.annotations, &macro_args.ui)?;

    // Assemble everything.
    let expanded = quote! {
        // Preserve the original function unchanged.
        #input

        /// Auto-generated tool handler for the `#fn_name` MCP tool.
        #[derive(Clone)]
        pub struct #struct_name {
            #struct_fields
        }

        // Manual Debug impl to avoid requiring T: Debug on state.
        impl std::fmt::Debug for #struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!(#struct_name)).finish()
            }
        }

        #[pmcp::async_trait]
        impl pmcp::ToolHandler for #struct_name {
            async fn handle(
                &self,
                args: serde_json::Value,
                #extra_param_name: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<serde_json::Value> {
                #handle_body
            }

            fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                let input_schema = #input_schema_code;
                let output_schema: Option<serde_json::Value> = #output_schema_code;
                let mut info = #tool_info_code;
                if let Some(schema) = output_schema {
                    info = info.with_output_schema(schema);
                }
                Some(info)
            }
        }

        impl #struct_name {
            #with_state_method
        }

        /// Create a new instance of the [`#struct_name`] tool handler.
        ///
        /// Use this at registration time:
        /// ```rust,ignore
        /// server_builder.tool("#tool_name", #fn_name())
        /// ```
        pub fn #fn_name() -> #struct_name {
            #constructor_default
        }
    };

    Ok(expanded)
}

/// Extract output schema code from the function's return type.
///
/// Returns `None` (as tokens) if return type is `Result<Value>` or not a Result.
/// Returns `Some(schema)` (as tokens) if return type is `Result<TypedStruct>`.
fn extract_output_schema_code(input: &ItemFn) -> syn::Result<TokenStream> {
    let return_type = match &input.sig.output {
        ReturnType::Default => return Ok(quote! { None }),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };

    // Try to extract Ok type from Result<T> or Result<T, E>.
    if let Some(ok_type) = mcp_common::extract_result_ok_type(return_type) {
        // Per D-15: skip outputSchema for Result<Value>.
        if mcp_common::is_value_type(&ok_type) {
            Ok(quote! { None })
        } else {
            Ok(mcp_common::generate_output_schema_code(&ok_type))
        }
    } else {
        Ok(quote! { None })
    }
}

/// Generate `ToolInfo` construction code, branching on annotations presence.
///
/// Per the plan: ToolInfo has NO `set_annotations()` method, so we must branch:
/// - With annotations: `ToolInfo::with_annotations(...)`
/// - Without annotations: `ToolInfo::new(...)`
fn generate_tool_info_code(
    tool_name: &str,
    description: &str,
    annotations: &Option<McpToolAnnotations>,
    ui: &Option<syn::Expr>,
) -> syn::Result<TokenStream> {
    let base_info = match annotations {
        Some(ann) => {
            // Build annotations chain.
            let mut chain_parts = Vec::new();
            chain_parts.push(quote! { pmcp::types::ToolAnnotations::new() });

            if let Some(read_only) = ann.read_only {
                chain_parts.push(quote! { .with_read_only(#read_only) });
            }
            if let Some(destructive) = ann.destructive {
                chain_parts.push(quote! { .with_destructive(#destructive) });
            }
            if let Some(idempotent) = ann.idempotent {
                chain_parts.push(quote! { .with_idempotent(#idempotent) });
            }
            if let Some(open_world) = ann.open_world {
                chain_parts.push(quote! { .with_open_world(#open_world) });
            }

            quote! {
                {
                    let annotations = #(#chain_parts)*;
                    pmcp::types::ToolInfo::with_annotations(
                        #tool_name,
                        Some(#description.to_string()),
                        input_schema,
                        annotations,
                    )
                }
            }
        }
        None => {
            quote! {
                pmcp::types::ToolInfo::new(
                    #tool_name,
                    Some(#description.to_string()),
                    input_schema,
                )
            }
        }
    };

    // If ui attribute is present, set _meta for widget attachment.
    if let Some(ui_expr) = ui {
        Ok(quote! {
            {
                let mut info = #base_info;
                info._meta = Some(pmcp::types::ui::ToolUIMetadata::build_meta_map(
                    &#ui_expr.to_string()
                ));
                info
            }
        })
    } else {
        Ok(base_info)
    }
}
