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
use crate::mcp_resource::McpResourceArgs;
use crate::mcp_tool::{McpToolAnnotations, McpToolArgs};
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{FnArg, ImplItem, ImplItemFn, ItemImpl, ReturnType, Type};

use mcp_common::ParamSlot;

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

/// Information collected from a single `#[mcp_resource]`-annotated method.
struct ResourceMethodInfo {
    /// The method identifier.
    method_name: syn::Ident,
    /// The resolved resource name.
    resource_name: String,
    /// Resource description.
    description: String,
    /// URI or URI template.
    uri: String,
    /// MIME type.
    mime_type: String,
    /// Whether the method is async.
    is_async: bool,
    /// URI template variable parameter names (in order).
    uri_param_names: Vec<String>,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
}

/// Expand `#[mcp_server]` attribute macro on an impl block.
///
/// The `args` token stream is currently unused (reserved for future options).
/// The `input` is the parsed `ItemImpl` block containing `#[mcp_tool]` methods.
pub fn expand_mcp_server(_args: TokenStream, mut input: ItemImpl) -> syn::Result<TokenStream> {
    // Collect all annotated methods.
    let tool_methods = collect_tool_methods(&input)?;
    let prompt_methods = collect_prompt_methods(&input)?;
    let resource_methods = collect_resource_methods(&input)?;

    if tool_methods.is_empty() && prompt_methods.is_empty() && resource_methods.is_empty() {
        return Err(syn::Error::new_spanned(
            &input,
            "No methods marked with #[mcp_tool], #[mcp_prompt], or #[mcp_resource] found in impl block",
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
        // State variant is never pushed for #[mcp_server] (rejected at collection time).
        let call_args: Vec<TokenStream> = method_info
            .param_order
            .iter()
            .map(|slot| match slot {
                ParamSlot::Args => quote! { typed_args },
                ParamSlot::Extra => quote! { extra },
                ParamSlot::State => unreachable!("#[mcp_server] uses &self, not State<T>"),
            })
            .collect();

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
        let output_schema_code = generate_method_output_schema(method_info);

        // Generate ToolInfo construction (branching on annotations).
        let tool_info_code = crate::mcp_tool::generate_tool_info_code(
            tool_name,
            description,
            method_info.annotations.as_ref(),
            method_info.ui.as_ref(),
        );

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

        // Generate args deserialization using shared runtime helper.
        let args_deser = if let Some(ref at) = prompt_info.args_type {
            let prompt_name_err = prompt_name;
            quote! {
                let typed_args: #at = pmcp::server::typed_prompt::deserialize_prompt_args(args, #prompt_name_err)?;
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
                ParamSlot::State => unreachable!("#[mcp_server] uses &self, not State<T>"),
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
                    let arguments = pmcp::server::typed_prompt::extract_prompt_arguments_from_schema(&json_schema);
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

    // Generate per-resource handler structs and DynamicResourceProvider impls.
    let mut resource_provider_names = Vec::new();
    for res_info in &resource_methods {
        let handler_name = format_ident!(
            "{}ResourceHandler",
            res_info.method_name.to_string().to_upper_camel_case()
        );
        let method_ident = &res_info.method_name;
        let uri = &res_info.uri;
        let resource_name = &res_info.resource_name;
        let description = &res_info.description;
        let mime_type = &res_info.mime_type;

        // Generate URI parameter extraction.
        let uri_param_extraction: Vec<TokenStream> = res_info
            .uri_param_names
            .iter()
            .map(|name| {
                let ident = format_ident!("{}", name);
                quote! {
                    let #ident = params.get(#name)
                        .ok_or_else(|| pmcp::Error::validation(format!(
                            "Missing URI parameter '{}' for resource '{}'", #name, #uri
                        )))?
                        .clone();
                }
            })
            .collect();

        // Build call arguments in the user's declared parameter order.
        let mut uri_var_idx = 0;
        let call_args: Vec<TokenStream> = res_info
            .param_order
            .iter()
            .map(|slot| match slot {
                ParamSlot::Args => {
                    let ident = format_ident!("{}", res_info.uri_param_names[uri_var_idx]);
                    uri_var_idx += 1;
                    quote! { #ident }
                },
                ParamSlot::Extra => quote! { _context.extra.clone() },
                ParamSlot::State => unreachable!("#[mcp_server] uses &self, not State<T>"),
            })
            .collect();

        // Generate function call (async vs sync).
        let fn_call = if res_info.is_async {
            quote! { let content_str: String = self.server.#method_ident(#(#call_args),*).await?; }
        } else {
            quote! { let content_str: String = self.server.#method_ident(#(#call_args),*)?; }
        };

        let handler_struct = quote! {
            struct #handler_name #handler_impl_params #handler_where {
                server: std::sync::Arc<#server_type>,
            }

            #[pmcp::async_trait]
            impl #handler_impl_params pmcp::server::dynamic_resources::DynamicResourceProvider
                for #handler_name #ty_gen_params #handler_where
            {
                fn templates(&self) -> Vec<pmcp::types::ResourceTemplate> {
                    let mut tmpl = pmcp::types::ResourceTemplate::new(#uri, #resource_name)
                        .with_description(#description);
                    tmpl.mime_type = Some(#mime_type.to_string());
                    vec![tmpl]
                }

                async fn fetch(
                    &self,
                    _uri: &str,
                    params: pmcp::server::dynamic_resources::UriParams,
                    _context: pmcp::server::dynamic_resources::RequestContext,
                ) -> pmcp::Result<pmcp::types::ReadResourceResult> {
                    #(#uri_param_extraction)*
                    #fn_call
                    Ok(pmcp::types::ReadResourceResult::new(
                        vec![pmcp::types::Content::text(content_str)]
                    ))
                }
            }
        };
        handler_structs.push(handler_struct);
        resource_provider_names.push(handler_name);
    }

    // Strip macro attributes from methods in the original impl block.
    strip_mcp_attrs(&mut input);

    // Generate the McpServer trait implementation.
    // If resources are present, build a ResourceCollection with all providers.
    let resource_registration = if !resource_provider_names.is_empty() {
        let provider_adds: Vec<TokenStream> = resource_provider_names
            .iter()
            .map(|name| {
                quote! {
                    .add_dynamic_provider(std::sync::Arc::new(#name { server: shared.clone() }))
                }
            })
            .collect();
        quote! {
            let resource_collection = pmcp::ResourceCollection::new()
                #(#provider_adds)*;
            builder = builder.resources(resource_collection);
        }
    } else {
        quote! {}
    };

    let mcp_server_impl = quote! {
        impl #impl_gen_params pmcp::McpServer for #server_type #where_clause {
            fn register(self, mut builder: pmcp::ServerBuilder) -> pmcp::ServerBuilder {
                let shared = std::sync::Arc::new(self);
                #(#register_lines)*
                #resource_registration
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
                },
                ParamRole::Args(ty) => {
                    if args_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one args parameter",
                        ));
                    }
                    args_type = Some(ty);
                    param_order.push(ParamSlot::Args);
                },
                ParamRole::Extra => {
                    if has_extra {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
                        ));
                    }
                    has_extra = true;
                    param_order.push(ParamSlot::Extra);
                },
                ParamRole::State { .. } => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "#[mcp_server] methods use &self for state access, not State<T>",
                    ));
                },
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

/// Parse `#[mcp_tool(...)]` on an impl-block method into `McpToolArgs`.
///
/// Delegates to `mcp_common::resolve_tool_args` so the rustdoc-fallback
/// and missing-description-error logic matches the standalone parse site.
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        // `#[mcp_tool]` with no parens — fall through to the resolver with
        // an empty token stream. The resolver will consult rustdoc.
        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
        // `#[mcp_tool = "..."]` is an orthogonal syntax error — keep the
        // pre-existing early-return.
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_tool requires parenthesized arguments: #[mcp_tool(description = \"...\")]",
            ));
        },
    };

    let nested_metas =
        crate::mcp_common::resolve_tool_args(tokens, &method.attrs, &method.sig.ident)?;

    McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Generate output schema code from a method's return type.
///
/// Returns `None` (as tokens) for `Result<Value>` or no return type.
/// Returns `Some(schema)` (as tokens) for `Result<TypedStruct>`.
fn generate_method_output_schema(method_info: &ToolMethodInfo) -> TokenStream {
    let Some(ref return_type) = method_info.return_type else {
        return quote! { None };
    };

    if let Some(ok_type) = mcp_common::extract_result_ok_type(return_type) {
        if mcp_common::is_value_type(&ok_type) {
            quote! { None }
        } else {
            mcp_common::generate_output_schema_code(&ok_type)
        }
    } else {
        quote! { None }
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
                },
                ParamRole::Args(ty) => {
                    if args_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one args parameter",
                        ));
                    }
                    args_type = Some(ty);
                    param_order.push(ParamSlot::Args);
                },
                ParamRole::Extra => {
                    if has_extra {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
                        ));
                    }
                    has_extra = true;
                    param_order.push(ParamSlot::Extra);
                },
                ParamRole::State { .. } => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "#[mcp_server] methods use &self for state access, not State<T>",
                    ));
                },
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
fn parse_mcp_prompt_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpPromptArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_prompt requires at least `description = \"...\"` attribute",
            ));
        },
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_prompt requires parenthesized arguments: #[mcp_prompt(description = \"...\")]",
            ));
        },
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

/// Collect information from all `#[mcp_resource(...)]`-annotated methods in the impl block.
fn collect_resource_methods(impl_block: &ItemImpl) -> syn::Result<Vec<ResourceMethodInfo>> {
    let mut methods = Vec::new();

    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        let Some(attr_index) = method
            .attrs
            .iter()
            .position(|a| a.path().is_ident("mcp_resource"))
        else {
            continue;
        };

        let attr = &method.attrs[attr_index];
        let macro_args = parse_mcp_resource_attr(attr, method)?;
        let resource_name = macro_args
            .name
            .unwrap_or_else(|| method.sig.ident.to_string());
        let uri = macro_args.uri;
        let mime_type = macro_args
            .mime_type
            .unwrap_or_else(|| "text/plain".to_string());
        let template_vars = crate::mcp_resource::extract_template_vars(&uri)
            .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e))?;

        let mut has_self = false;
        let mut has_extra = false;
        let mut uri_param_names: Vec<String> = Vec::new();
        let mut param_order: Vec<ParamSlot> = Vec::new();

        for param in &method.sig.inputs {
            let role = mcp_common::classify_param(param)?;
            match role {
                ParamRole::SelfRef => {
                    has_self = true;
                },
                ParamRole::Args(ty) => {
                    // For resources in #[mcp_server], String params map to URI template vars.
                    if mcp_common::type_name_matches(&ty, "String") {
                        if let FnArg::Typed(pat_type) = param {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                let name = pat_ident.ident.to_string();
                                if template_vars.contains(&name) {
                                    uri_param_names.push(name);
                                    param_order.push(ParamSlot::Args);
                                    continue;
                                }
                            }
                        }
                        return Err(syn::Error::new_spanned(
                            param,
                            format!(
                                "Parameter name must match a URI template variable. Available: {:?}",
                                template_vars
                            ),
                        ));
                    } else {
                        return Err(syn::Error::new_spanned(
                            param,
                            "Resource function parameters (URI template variables) must be String type",
                        ));
                    }
                },
                ParamRole::Extra => {
                    if has_extra {
                        return Err(syn::Error::new_spanned(
                            param,
                            "#[mcp_resource] methods can have at most one RequestHandlerExtra parameter",
                        ));
                    }
                    has_extra = true;
                    param_order.push(ParamSlot::Extra);
                },
                ParamRole::State { .. } => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "#[mcp_server] methods use &self for state access, not State<T>",
                    ));
                },
            }
        }

        if !has_self {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "#[mcp_server] methods must take &self as the first parameter",
            ));
        }

        // Validate all URI template variables have matching parameters.
        let uncovered: Vec<&str> = template_vars
            .iter()
            .filter(|v| !uri_param_names.contains(v))
            .map(String::as_str)
            .collect();
        if !uncovered.is_empty() {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                format!(
                    "URI template variables not covered by function parameters: {:?}",
                    uncovered
                ),
            ));
        }

        methods.push(ResourceMethodInfo {
            method_name: method.sig.ident.clone(),
            resource_name,
            description: macro_args.description,
            uri,
            mime_type,
            is_async: method.sig.asyncness.is_some(),
            uri_param_names,
            param_order,
        });
    }

    Ok(methods)
}

/// Parse `#[mcp_resource(...)]` attribute into `McpResourceArgs`.
fn parse_mcp_resource_attr(
    attr: &syn::Attribute,
    method: &ImplItemFn,
) -> syn::Result<McpResourceArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_resource requires `uri = \"...\"` and `description = \"...\"` attributes",
            ));
        },
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_resource requires parenthesized arguments: #[mcp_resource(uri = \"...\", description = \"...\")]",
            ));
        },
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpResourceArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Strip `#[mcp_tool]`, `#[mcp_prompt]`, and `#[mcp_resource]` attributes from all methods.
fn strip_mcp_attrs(input: &mut ItemImpl) {
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("mcp_tool")
                    && !attr.path().is_ident("mcp_prompt")
                    && !attr.path().is_ident("mcp_resource")
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
            },
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
                !method.attrs.iter().any(|a| a.path().is_ident("mcp_prompt")),
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
            },
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
