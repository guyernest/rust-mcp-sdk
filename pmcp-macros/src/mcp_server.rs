//! `#[mcp_server]` attribute macro expansion for impl blocks.
//!
//! Processes an impl block annotated with `#[mcp_server]`, collects all methods
//! marked with `#[mcp_tool(...)]` and `#[mcp_prompt(...)]`, and generates:
//!
//! 1. Per-tool handler structs implementing `ToolHandler` (using `Arc<ServerType>`)
//! 2. Per-prompt handler structs implementing `PromptHandler` (using `Arc<ServerType>`)
//! 3. An `impl McpServer for ServerType` with `register()` for bulk registration
//! 4. The original impl block preserved with `#[mcp_tool]`/`#[mcp_prompt]` attributes stripped
//!
//! # Example
//!
//! ```rust,ignore
//! #[mcp_server]
//! impl MyServer {
//!     #[mcp_tool(description = "Query database")]
//!     async fn query(&self, args: QueryArgs) -> Result<QueryResult> {
//!         self.db.execute(&args.sql).await
//!     }
//!
//!     #[mcp_prompt(description = "Generate a query")]
//!     async fn query_prompt(&self, args: PromptArgs) -> Result<GetPromptResult> {
//!         Ok(GetPromptResult::new(vec![...], None))
//!     }
//! }
//!
//! // Register all tools and prompts at once:
//! let builder = ServerBuilder::new()
//!     .mcp_server(my_server);
//! ```

use crate::mcp_common::{self, ParamRole};
use crate::mcp_prompt::McpPromptArgs;
use crate::mcp_tool::{McpToolAnnotations, McpToolArgs};
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{ImplItem, ImplItemFn, ItemImpl, ReturnType, Type};

/// Parameter role for call-site argument ordering.
#[derive(Debug, Clone, Copy)]
enum ParamSlot {
    Args,
    Extra,
}

/// Information collected from a single `#[mcp_tool]`-annotated method.
struct ToolMethodInfo {
    /// The method identifier (e.g., `query`).
    method_name: syn::Ident,
    /// The resolved tool name (from `name = "..."` or method name).
    tool_name: String,
    /// Tool description (mandatory).
    description: String,
    /// Whether the method is async.
    is_async: bool,
    /// The args type if the method takes a typed input parameter.
    args_type: Option<Type>,
    /// Whether the method takes `RequestHandlerExtra`.
    has_extra: bool,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
    /// The return type of the method (full signature type).
    return_type: Option<Type>,
    /// MCP standard annotations.
    annotations: Option<McpToolAnnotations>,
    /// UI widget resource URI.
    ui: Option<syn::Expr>,
}

/// Information collected from a single `#[mcp_prompt]`-annotated method.
struct PromptMethodInfo {
    /// The method identifier (e.g., `query_builder`).
    method_name: syn::Ident,
    /// The resolved prompt name (from `name = "..."` or method name).
    prompt_name: String,
    /// Prompt description (mandatory).
    description: String,
    /// Whether the method is async.
    is_async: bool,
    /// The args type if the method takes a typed input parameter.
    args_type: Option<Type>,
    /// Whether the method takes `RequestHandlerExtra`.
    has_extra: bool,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
}

/// Expand `#[mcp_server]` attribute macro on an impl block.
///
/// The `args` token stream is currently unused (reserved for future options).
/// The `input` is the parsed `ItemImpl` block containing `#[mcp_tool]` methods.
pub fn expand_mcp_server(_args: TokenStream, mut input: ItemImpl) -> syn::Result<TokenStream> {
    // Collect all #[mcp_tool]-annotated methods.
    let tool_methods = collect_tool_methods(&input)?;
    // Collect all #[mcp_prompt]-annotated methods.
    let prompt_methods = collect_prompt_methods(&input)?;

    if tool_methods.is_empty() && prompt_methods.is_empty() {
        return Err(syn::Error::new_spanned(
            &input,
            "No methods marked with #[mcp_tool] or #[mcp_prompt] found in impl block",
        ));
    }

    // Extract the server type from the impl block (clone to avoid borrow conflict
    // with later mutable strip operation).
    let server_type = input.self_ty.clone();

    // Extract generics from the impl block for generic server support (D-25).
    let impl_generics = input.generics.clone();
    // Add Send + Sync + 'static bounds for handler struct generics.
    let handler_generics = mcp_common::add_async_trait_bounds(impl_generics.clone());
    let (impl_gen_params, ty_gen_params, where_clause) = impl_generics.split_for_impl();
    let (handler_impl_params, _handler_ty_params, handler_where) =
        handler_generics.split_for_impl();

    // Generate per-tool handler structs and ToolHandler impls.
    let mut handler_structs = Vec::new();
    let mut register_lines = Vec::new();

    for method_info in &tool_methods {
        let handler_name = format_ident!(
            "{}ToolHandler",
            method_info.method_name.to_string().to_upper_camel_case()
        );
        let method_ident = &method_info.method_name;
        let tool_name = &method_info.tool_name;
        let description = &method_info.description;

        // Generate args deserialization.
        let args_deser = if let Some(ref at) = method_info.args_type {
            let tool_name_err = tool_name;
            quote! {
                let typed_args: #at = serde_json::from_value(args)
                    .map_err(|e| pmcp::Error::invalid_params(
                        format!("Invalid arguments for tool '{}': {}", #tool_name_err, e)
                    ))?;
            }
        } else {
            quote! {}
        };

        // Build call arguments in the user's declared parameter order.
        let call_args: Vec<TokenStream> = method_info.param_order.iter().map(|slot| match slot {
            ParamSlot::Args => quote! { typed_args },
            ParamSlot::Extra => quote! { extra },
        }).collect();

        // Generate function call (async vs sync).
        let fn_call = if method_info.is_async {
            quote! { let result = self.server.#method_ident(#(#call_args),*).await?; }
        } else {
            quote! { let result = self.server.#method_ident(#(#call_args),*)?; }
        };

        // Extra parameter name in handle() signature.
        let extra_param_name = if method_info.has_extra {
            format_ident!("extra")
        } else {
            format_ident!("_extra")
        };

        // Generate input schema code.
        let input_schema_code = if let Some(ref at) = method_info.args_type {
            mcp_common::generate_input_schema_code(at)
        } else {
            mcp_common::generate_empty_schema_code()
        };

        // Generate output schema code.
        let output_schema_code = generate_method_output_schema(method_info)?;

        // Generate ToolInfo construction (branching on annotations).
        let tool_info_code = crate::mcp_tool::generate_tool_info_code(
            tool_name,
            description,
            &method_info.annotations,
            &method_info.ui,
        )?;

        // Generate the handler struct and ToolHandler impl.
        let handler_struct = quote! {
            struct #handler_name #handler_impl_params #handler_where {
                server: std::sync::Arc<#server_type>,
            }

            #[pmcp::async_trait]
            impl #handler_impl_params pmcp::ToolHandler for #handler_name #ty_gen_params #handler_where {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    #extra_param_name: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    #args_deser
                    #fn_call
                    serde_json::to_value(result)
                        .map_err(|e| pmcp::Error::internal(
                            format!("Failed to serialize result: {}", e)
                        ))
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
        };
        handler_structs.push(handler_struct);

        // Generate registration line for register().
        let register_line = quote! {
            builder = builder.tool(#tool_name, #handler_name { server: shared.clone() });
        };
        register_lines.push(register_line);
    }

    // Generate per-prompt handler structs and PromptHandler impls.
    for prompt_info in &prompt_methods {
        let handler_name = format_ident!(
            "{}PromptHandler",
            prompt_info.method_name.to_string().to_upper_camel_case()
        );
        let method_ident = &prompt_info.method_name;
        let prompt_name = &prompt_info.prompt_name;
        let description = &prompt_info.description;

        // Generate args deserialization for prompts.
        // Prompts receive HashMap<String, String>, converted to JSON Value for deserialization.
        let args_deser = if let Some(ref at) = prompt_info.args_type {
            let prompt_name_err = prompt_name;
            quote! {
                let value = serde_json::Value::Object(
                    args.into_iter()
                        .map(|(k, v)| (k, serde_json::Value::String(v)))
                        .collect()
                );
                let typed_args: #at = serde_json::from_value(value)
                    .map_err(|e| pmcp::Error::invalid_params(
                        format!("Invalid arguments for prompt '{}': {}", #prompt_name_err, e)
                    ))?;
            }
        } else {
            quote! {}
        };

        // Build call arguments in the user's declared parameter order.
        let call_args: Vec<TokenStream> = prompt_info
            .param_order
            .iter()
            .map(|slot| match slot {
                ParamSlot::Args => quote! { typed_args },
                ParamSlot::Extra => quote! { extra },
            })
            .collect();

        // Generate function call (async vs sync).
        // Prompts return GetPromptResult directly -- no serialization wrapper.
        let fn_call = if prompt_info.is_async {
            quote! { self.server.#method_ident(#(#call_args),*).await }
        } else {
            quote! { self.server.#method_ident(#(#call_args),*) }
        };

        // Extra parameter name in handle() signature.
        let extra_param_name = if prompt_info.has_extra {
            format_ident!("extra")
        } else {
            format_ident!("_extra")
        };

        // Generate metadata body based on whether prompt has args.
        let metadata_body = if let Some(ref at) = prompt_info.args_type {
            quote! {
                fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
                    let mut info = pmcp::types::PromptInfo::new(#prompt_name)
                        .with_description(#description);

                    let schema = schemars::schema_for!(#at);
                    let json_schema = serde_json::to_value(&schema).unwrap_or_default();
                    let arguments = {
                        let properties = json_schema.get("properties").and_then(|p| p.as_object());
                        let required_fields: Vec<String> = json_schema.get("required")
                            .and_then(|r| r.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        let Some(props) = properties else { return Some(info); };
                        props.iter().map(|(name, prop)| {
                            let mut arg = pmcp::types::PromptArgument::new(name);
                            if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
                                arg = arg.with_description(desc);
                            }
                            if required_fields.contains(name) {
                                arg = arg.required();
                            }
                            arg
                        }).collect::<Vec<_>>()
                    };
                    if !arguments.is_empty() {
                        info = info.with_arguments(arguments);
                    }
                    Some(info)
                }
            }
        } else {
            quote! {
                fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
                    Some(pmcp::types::PromptInfo::new(#prompt_name)
                        .with_description(#description))
                }
            }
        };

        // Generate the handler struct and PromptHandler impl.
        let handler_struct = quote! {
            struct #handler_name #handler_impl_params #handler_where {
                server: std::sync::Arc<#server_type>,
            }

            #[pmcp::async_trait]
            impl #handler_impl_params pmcp::PromptHandler for #handler_name #ty_gen_params #handler_where {
                async fn handle(
                    &self,
                    args: std::collections::HashMap<String, String>,
                    #extra_param_name: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<pmcp::types::GetPromptResult> {
                    #args_deser
                    #fn_call
                }

                #metadata_body
            }
        };
        handler_structs.push(handler_struct);

        // Generate prompt registration line.
        let register_line = quote! {
            builder = builder.prompt(#prompt_name, #handler_name { server: shared.clone() });
        };
        register_lines.push(register_line);
    }

    // Strip #[mcp_tool(...)] and #[mcp_prompt(...)] attributes from methods in the original impl block.
    strip_mcp_attrs(&mut input);

    // Generate the McpServer trait implementation.
    let mcp_server_impl = quote! {
        impl #impl_gen_params pmcp::McpServer for #server_type #where_clause {
            fn register(self, mut builder: pmcp::ServerBuilder) -> pmcp::ServerBuilder {
                let shared = std::sync::Arc::new(self);
                #(#register_lines)*
                builder
            }
        }
    };

    // Assemble output: original impl block + handler structs + McpServer impl.
    let expanded = quote! {
        #input

        #(#handler_structs)*

        #mcp_server_impl
    };

    Ok(expanded)
}

/// Collect information from all `#[mcp_tool(...)]`-annotated methods in the impl block.
fn collect_tool_methods(impl_block: &ItemImpl) -> syn::Result<Vec<ToolMethodInfo>> {
    let mut methods = Vec::new();

    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        // Find #[mcp_tool(...)] attribute.
        let Some(attr_index) = method
            .attrs
            .iter()
            .position(|a| a.path().is_ident("mcp_tool"))
        else {
            continue;
        };

        let attr = &method.attrs[attr_index];

        // Parse the attribute arguments using darling.
        let macro_args = parse_mcp_tool_attr(attr, method)?;
        let tool_name = macro_args
            .name
            .unwrap_or_else(|| method.sig.ident.to_string());

        // Classify parameters.
        let mut args_type: Option<Type> = None;
        let mut has_extra = false;
        let mut has_self = false;
        let mut param_order: Vec<ParamSlot> = Vec::new();

        for param in &method.sig.inputs {
            let role = mcp_common::classify_param(param)?;
            match role {
                ParamRole::SelfRef => {
                    has_self = true;
                }
                ParamRole::Args(ty) => {
                    if args_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one args parameter",
                        ));
                    }
                    args_type = Some(ty);
                    param_order.push(ParamSlot::Args);
                }
                ParamRole::Extra => {
                    if has_extra {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
                        ));
                    }
                    has_extra = true;
                    param_order.push(ParamSlot::Extra);
                }
                ParamRole::State { .. } => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "#[mcp_server] methods use &self for state access, not State<T>",
                    ));
                }
            }
        }

        // Warn if method doesn't have &self (unusual but we handle it).
        if !has_self {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "#[mcp_server] methods must take &self as the first parameter",
            ));
        }

        // Extract return type.
        let return_type = match &method.sig.output {
            ReturnType::Default => None,
            ReturnType::Type(_, ty) => Some(ty.as_ref().clone()),
        };

        methods.push(ToolMethodInfo {
            method_name: method.sig.ident.clone(),
            tool_name,
            description: macro_args.description,
            is_async: method.sig.asyncness.is_some(),
            args_type,
            has_extra,
            param_order,
            return_type,
            annotations: macro_args.annotations,
            ui: macro_args.ui,
        });
    }

    Ok(methods)
}

/// Parse `#[mcp_tool(...)]` attribute into `McpToolArgs`.
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_tool requires at least `description = \"...\"` attribute",
            ));
        }
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_tool requires parenthesized arguments: #[mcp_tool(description = \"...\")]",
            ));
        }
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Generate output schema code from a method's return type.
///
/// Returns `None` (as tokens) for `Result<Value>` or no return type.
/// Returns `Some(schema)` (as tokens) for `Result<TypedStruct>`.
fn generate_method_output_schema(method_info: &ToolMethodInfo) -> syn::Result<TokenStream> {
    let Some(ref return_type) = method_info.return_type else {
        return Ok(quote! { None });
    };

    if let Some(ok_type) = mcp_common::extract_result_ok_type(return_type) {
        if mcp_common::is_value_type(&ok_type) {
            Ok(quote! { None })
        } else {
            Ok(mcp_common::generate_output_schema_code(&ok_type))
        }
    } else {
        Ok(quote! { None })
    }
}

/// Collect information from all `#[mcp_prompt(...)]`-annotated methods in the impl block.
fn collect_prompt_methods(impl_block: &ItemImpl) -> syn::Result<Vec<PromptMethodInfo>> {
    let mut methods = Vec::new();

    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        // Find #[mcp_prompt(...)] attribute.
        let Some(attr_index) = method
            .attrs
            .iter()
            .position(|a| a.path().is_ident("mcp_prompt"))
        else {
            continue;
        };

        let attr = &method.attrs[attr_index];

        // Parse the attribute arguments using darling.
        let macro_args = parse_mcp_prompt_attr(attr, method)?;
        let prompt_name = macro_args
            .name
            .unwrap_or_else(|| method.sig.ident.to_string());

        // Classify parameters.
        let mut args_type: Option<Type> = None;
        let mut has_extra = false;
        let mut has_self = false;
        let mut param_order: Vec<ParamSlot> = Vec::new();

        for param in &method.sig.inputs {
            let role = mcp_common::classify_param(param)?;
            match role {
                ParamRole::SelfRef => {
                    has_self = true;
                }
                ParamRole::Args(ty) => {
                    if args_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one args parameter",
                        ));
                    }
                    args_type = Some(ty);
                    param_order.push(ParamSlot::Args);
                }
                ParamRole::Extra => {
                    if has_extra {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
                        ));
                    }
                    has_extra = true;
                    param_order.push(ParamSlot::Extra);
                }
                ParamRole::State { .. } => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "#[mcp_server] methods use &self for state access, not State<T>",
                    ));
                }
            }
        }

        // Require &self (same as tool methods).
        if !has_self {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "#[mcp_server] methods must take &self as the first parameter",
            ));
        }

        methods.push(PromptMethodInfo {
            method_name: method.sig.ident.clone(),
            prompt_name,
            description: macro_args.description,
            is_async: method.sig.asyncness.is_some(),
            args_type,
            has_extra,
            param_order,
        });
    }

    Ok(methods)
}

/// Parse `#[mcp_prompt(...)]` attribute into `McpPromptArgs`.
fn parse_mcp_prompt_attr(
    attr: &syn::Attribute,
    method: &ImplItemFn,
) -> syn::Result<McpPromptArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_prompt requires at least `description = \"...\"` attribute",
            ));
        }
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_prompt requires parenthesized arguments: #[mcp_prompt(description = \"...\")]",
            ));
        }
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpPromptArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Strip `#[mcp_tool(...)]` and `#[mcp_prompt(...)]` attributes from all methods.
///
/// After collecting tool and prompt info, the attributes are no longer needed and
/// would cause compilation errors if left in place (since `mcp_tool`/`mcp_prompt`
/// are proc macro attributes that expect standalone functions, not methods in an
/// already-processed block).
fn strip_mcp_attrs(input: &mut ItemImpl) {
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("mcp_tool") && !attr.path().is_ident("mcp_prompt")
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_collect_tool_methods_finds_annotated() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query data")]
                async fn query(&self, args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }

                fn helper(&self) {}

                #[mcp_tool(description = "Clear cache")]
                async fn clear_cache(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method_name, "query");
        assert_eq!(methods[0].tool_name, "query");
        assert_eq!(methods[0].description, "Query data");
        assert!(methods[0].is_async);
        assert!(methods[0].args_type.is_some());
        assert!(!methods[0].has_extra);

        assert_eq!(methods[1].method_name, "clear_cache");
        assert_eq!(methods[1].tool_name, "clear_cache");
        assert!(!methods[1].has_extra);
        assert!(methods[1].args_type.is_none());
    }

    #[test]
    fn test_collect_tool_methods_empty_errors() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                fn helper(&self) {}
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert!(methods.is_empty());
    }

    #[test]
    fn test_collect_tool_methods_with_extra() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Export data")]
                async fn export(&self, args: ExportArgs, extra: RequestHandlerExtra) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 1);
        assert!(methods[0].has_extra);
        assert!(methods[0].args_type.is_some());
    }

    #[test]
    fn test_collect_tool_methods_state_rejected() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Bad tool")]
                async fn bad(&self, db: State<Database>) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let result = collect_tool_methods(&impl_block);
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("use &self for state access"),
                    "Expected State<T> rejection error, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Expected error for State<T> in #[mcp_server] method"),
        }
    }

    #[test]
    fn test_collect_tool_methods_custom_name() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query data", name = "custom_query")]
                async fn query(&self, args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods[0].tool_name, "custom_query");
    }

    #[test]
    fn test_strip_mcp_attrs() {
        let mut impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query")]
                #[doc = "A query method"]
                async fn query(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }

                #[mcp_prompt(description = "Prompt")]
                async fn my_prompt(&self) -> Result<GetPromptResult> {
                    unimplemented!()
                }
            }
        };

        strip_mcp_attrs(&mut impl_block);

        // The #[mcp_tool] attr should be removed, but #[doc] preserved.
        if let ImplItem::Fn(method) = &impl_block.items[0] {
            assert!(
                !method.attrs.iter().any(|a| a.path().is_ident("mcp_tool")),
                "mcp_tool attribute should be stripped"
            );
            assert!(
                method.attrs.iter().any(|a| a.path().is_ident("doc")),
                "doc attribute should be preserved"
            );
        } else {
            panic!("Expected ImplItem::Fn");
        }

        // The #[mcp_prompt] attr should also be removed.
        if let ImplItem::Fn(method) = &impl_block.items[1] {
            assert!(
                !method
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("mcp_prompt")),
                "mcp_prompt attribute should be stripped"
            );
        } else {
            panic!("Expected ImplItem::Fn");
        }
    }

    #[test]
    fn test_collect_tool_methods_no_self_errors() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "No self")]
                async fn no_self(args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let result = collect_tool_methods(&impl_block);
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("must take &self"),
                    "Expected &self requirement error, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Expected error for method without &self"),
        }
    }

    #[test]
    fn test_collect_tool_methods_sync() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Get version")]
                fn version(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 1);
        assert!(!methods[0].is_async);
    }

    #[test]
    fn test_collect_prompt_methods_finds_annotated() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_prompt(description = "Generate query")]
                async fn query_builder(&self, args: QueryArgs) -> Result<GetPromptResult> {
                    unimplemented!()
                }

                fn helper(&self) {}

                #[mcp_prompt(description = "System status")]
                async fn status(&self) -> Result<GetPromptResult> {
                    unimplemented!()
                }
            }
        };

        let methods = collect_prompt_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method_name, "query_builder");
        assert_eq!(methods[0].prompt_name, "query_builder");
        assert_eq!(methods[0].description, "Generate query");
        assert!(methods[0].is_async);
        assert!(methods[0].args_type.is_some());
        assert!(!methods[0].has_extra);

        assert_eq!(methods[1].method_name, "status");
        assert_eq!(methods[1].prompt_name, "status");
        assert_eq!(methods[1].description, "System status");
        assert!(methods[1].args_type.is_none());
    }
}
