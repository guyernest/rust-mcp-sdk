//! `#[mcp_resource]` attribute macro expansion.
//!
//! Generates a struct implementing `DynamicResourceProvider` from an annotated
//! standalone async or sync function, providing automatic URI template matching,
//! parameter extraction, and state injection.
//!
//! # Generated Code
//!
//! For each annotated function, the macro generates:
//! 1. The original function (preserved unchanged under an internal name)
//! 2. A `{PascalCase(fn_name)}Resource` struct implementing `DynamicResourceProvider`
//! 3. A constructor function `fn fn_name() -> StructName` for ergonomic registration
//!
//! # Examples
//!
//! Static resource (no URI template variables):
//! ```rust,ignore
//! #[mcp_resource(uri = "config://settings", description = "App settings")]
//! fn settings() -> Result<String> {
//!     Ok(r#"{"theme": "dark"}"#.to_string())
//! }
//! ```
//!
//! Dynamic resource (with URI template variables):
//! ```rust,ignore
//! #[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
//! async fn read_doc(topic: String) -> Result<String> {
//!     tokio::fs::read_to_string(format!("docs/{topic}.md")).await.map_err(Into::into)
//! }
//! ```

use crate::mcp_common;
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{FnArg, ItemFn, Pat, Type};

use mcp_common::ParamSlot;

/// Parsed attributes for `#[mcp_resource(...)]`.
#[derive(Debug, FromMeta)]
pub struct McpResourceArgs {
    /// URI or URI template (required). Supports RFC 6570 templates like `docs://{topic}`.
    pub(crate) uri: String,
    /// Resource description (required).
    pub(crate) description: String,
    /// Override resource name (defaults to function name).
    #[darling(default)]
    pub(crate) name: Option<String>,
    /// MIME type (defaults to "text/plain").
    #[darling(default)]
    pub(crate) mime_type: Option<String>,
}

/// Extract `{placeholder}` names from a URI template string.
///
/// Returns an error for malformed templates (unclosed braces, empty variables).
pub fn extract_template_vars(uri: &str) -> Result<Vec<String>, String> {
    let open = uri.chars().filter(|&c| c == '{').count();
    let close = uri.chars().filter(|&c| c == '}').count();
    if open != close {
        return Err(format!("Unmatched braces in URI template: {uri}"));
    }
    let mut vars = Vec::new();
    for segment in uri.split('{').skip(1) {
        let name = segment.split('}').next().unwrap_or("");
        if name.is_empty() {
            return Err(format!("Empty template variable `{{}}` in URI: {uri}"));
        }
        vars.push(name.to_string());
    }
    Ok(vars)
}

/// Parse `#[mcp_resource(...)]` attribute tokens into a `Vec<NestedMeta>`,
/// rejecting empty argument lists with a descriptive error.
fn parse_resource_attr_args(
    args: TokenStream,
    fn_ident: &syn::Ident,
) -> syn::Result<Vec<darling::ast::NestedMeta>> {
    if args.is_empty() {
        return Err(syn::Error::new_spanned(
            fn_ident,
            "mcp_resource requires at least `uri = \"...\"` and `description = \"...\"` attributes",
        ));
    }
    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    Ok(parser
        .parse2(args)
        .map_err(|e| {
            syn::Error::new_spanned(
                fn_ident,
                format!("invalid mcp_resource attributes: {e}"),
            )
        })?
        .into_iter()
        .collect())
}

/// Match a `String` arg parameter against URI-template vars; push slot or
/// return a typed error.
fn classify_resource_template_arg(
    param: &FnArg,
    template_vars: &[String],
    uri_param_names: &mut Vec<String>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if let FnArg::Typed(pat_type) = param {
        if let Pat::Ident(pat_ident) = &*pat_type.pat {
            let name = pat_ident.ident.to_string();
            if template_vars.contains(&name) {
                uri_param_names.push(name);
                param_order.push(ParamSlot::Args);
                return Ok(());
            }
        }
    }
    Err(syn::Error::new_spanned(
        param,
        format!(
            "Parameter name must match a URI template variable. Available: {:?}",
            template_vars
        ),
    ))
}

/// Process a single `#[mcp_resource]` function parameter, dispatching by role.
#[allow(clippy::too_many_arguments)]
fn process_resource_fn_param(
    param: &FnArg,
    template_vars: &[String],
    state_inner_ty: &mut Option<Type>,
    has_extra: &mut bool,
    uri_param_names: &mut Vec<String>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    let role = mcp_common::classify_param(param)?;
    match role {
        mcp_common::ParamRole::Args(ty) => {
            if !mcp_common::type_name_matches(&ty, "String") {
                return Err(syn::Error::new_spanned(
                    param,
                    "Resource function parameters (URI template variables) must be String type",
                ));
            }
            classify_resource_template_arg(param, template_vars, uri_param_names, param_order)
        },
        mcp_common::ParamRole::State { inner_ty, .. } => {
            if state_inner_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    param,
                    "mcp_resource functions can have at most one State<T> parameter",
                ));
            }
            *state_inner_ty = Some(inner_ty);
            param_order.push(ParamSlot::State);
            Ok(())
        },
        mcp_common::ParamRole::Extra => {
            if *has_extra {
                return Err(syn::Error::new_spanned(
                    param,
                    "mcp_resource functions can have at most one RequestHandlerExtra parameter",
                ));
            }
            *has_extra = true;
            param_order.push(ParamSlot::Extra);
            Ok(())
        },
        mcp_common::ParamRole::SelfRef => Err(syn::Error::new_spanned(
            param,
            "standalone #[mcp_resource] functions cannot have &self — use #[mcp_server] for impl block resources",
        )),
    }
}

/// Bundled parameter classification for a `#[mcp_resource]` function.
struct ResourceFnParams {
    state_inner_ty: Option<Type>,
    uri_param_names: Vec<String>,
    param_order: Vec<ParamSlot>,
}

/// Classify every parameter of an `#[mcp_resource]` function.
fn classify_resource_fn_params(
    input: &ItemFn,
    template_vars: &[String],
) -> syn::Result<ResourceFnParams> {
    let mut state_inner_ty: Option<Type> = None;
    let mut has_extra = false;
    let mut uri_param_names: Vec<String> = Vec::new();
    let mut param_order: Vec<ParamSlot> = Vec::new();
    for param in &input.sig.inputs {
        process_resource_fn_param(
            param,
            template_vars,
            &mut state_inner_ty,
            &mut has_extra,
            &mut uri_param_names,
            &mut param_order,
        )?;
    }
    Ok(ResourceFnParams {
        state_inner_ty,
        uri_param_names,
        param_order,
    })
}

/// Verify all URI template variables are covered by the function parameters.
fn require_resource_template_var_coverage(
    fn_ident: &syn::Ident,
    template_vars: &[String],
    uri_param_names: &[String],
) -> syn::Result<()> {
    let uncovered: Vec<&str> = template_vars
        .iter()
        .filter(|v| !uri_param_names.contains(v))
        .map(String::as_str)
        .collect();
    if uncovered.is_empty() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        fn_ident,
        format!(
            "URI template variables not covered by function parameters: {:?}",
            uncovered
        ),
    ))
}

/// Generate the optional `state` struct field, `with_state()` method,
/// constructor body, and runtime state-resolution snippet for a resource
/// that may take `State<T>`.
struct StateCodegen {
    struct_fields: TokenStream,
    with_state_method: TokenStream,
    constructor_default: TokenStream,
    state_resolution: TokenStream,
}

fn generate_resource_state_codegen(
    state_inner_ty: Option<&Type>,
    struct_name: &syn::Ident,
    resource_name: &str,
) -> StateCodegen {
    let Some(inner) = state_inner_ty else {
        return StateCodegen {
            struct_fields: quote! {},
            with_state_method: quote! {},
            constructor_default: quote! { #struct_name {} },
            state_resolution: quote! {},
        };
    };
    let inner_name = quote!(#inner).to_string();
    StateCodegen {
        struct_fields: quote! { state: Option<std::sync::Arc<#inner>>, },
        with_state_method: quote! {
            /// Provide shared state for this resource.
            pub fn with_state(mut self, state: impl Into<std::sync::Arc<#inner>>) -> Self {
                self.state = Some(state.into());
                self
            }
        },
        constructor_default: quote! { #struct_name { state: None } },
        state_resolution: quote! {
            let state_val = pmcp::State(
                self.state.as_ref()
                    .ok_or_else(|| pmcp::Error::internal(format!(
                        "State<{}> not provided for resource '{}' -- call .with_state() during registration",
                        #inner_name, #resource_name
                    )))?
                    .clone()
            );
        },
    }
}

/// Generate URI-parameter extraction code for the `fetch()` body.
fn generate_resource_uri_param_extractions(
    uri_param_names: &[String],
    uri: &str,
) -> Vec<TokenStream> {
    uri_param_names
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
        .collect()
}

/// Generate the call-site arg tokens for the standalone-function fetch impl.
fn generate_resource_call_args(
    param_order: &[ParamSlot],
    uri_param_names: &[String],
) -> Vec<TokenStream> {
    let mut uri_var_idx = 0;
    param_order
        .iter()
        .map(|slot| match slot {
            ParamSlot::Args => {
                let ident = format_ident!("{}", uri_param_names[uri_var_idx]);
                uri_var_idx += 1;
                quote! { #ident }
            },
            ParamSlot::State => quote! { state_val },
            ParamSlot::Extra => quote! { _context.extra.clone() },
        })
        .collect()
}

/// Expand `#[mcp_resource]` attribute macro on a standalone function.
///
/// Refactored in 75-01 Task 1b-B (P1): per-section codegen extracted into
/// [`parse_resource_attr_args`], [`classify_resource_fn_params`],
/// [`require_resource_template_var_coverage`],
/// [`generate_resource_state_codegen`],
/// [`generate_resource_uri_param_extractions`],
/// [`generate_resource_call_args`].
pub fn expand_mcp_resource(args: TokenStream, input: &ItemFn) -> syn::Result<TokenStream> {
    let nested_metas = parse_resource_attr_args(args, &input.sig.ident)?;
    let macro_args = McpResourceArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;

    // Extract function info.
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let resource_name = macro_args.name.unwrap_or_else(|| fn_name_str.clone());
    let is_async = input.sig.asyncness.is_some();
    let struct_name = format_ident!("{}Resource", fn_name_str.to_upper_camel_case());
    let description = &macro_args.description;
    let uri = &macro_args.uri;
    let mime_type = macro_args.mime_type.as_deref().unwrap_or("text/plain");

    // Detect URI template variables.
    let template_vars =
        extract_template_vars(uri).map_err(|e| syn::Error::new_spanned(&input.sig.ident, e))?;

    // Rename the original function to an internal name.
    let impl_fn_name = format_ident!("__{}_impl", fn_name_str);
    let mut impl_fn = input.clone();
    impl_fn.sig.ident = impl_fn_name.clone();

    let params = classify_resource_fn_params(input, &template_vars)?;
    require_resource_template_var_coverage(
        &input.sig.ident,
        &template_vars,
        &params.uri_param_names,
    )?;

    let state_codegen = generate_resource_state_codegen(
        params.state_inner_ty.as_ref(),
        &struct_name,
        &resource_name,
    );

    let uri_param_extraction_code =
        generate_resource_uri_param_extractions(&params.uri_param_names, uri);
    let call_args = generate_resource_call_args(&params.param_order, &params.uri_param_names);

    let fn_call = if is_async {
        quote! { let content_str: String = #impl_fn_name(#(#call_args),*).await?; }
    } else {
        quote! { let content_str: String = #impl_fn_name(#(#call_args),*)?; }
    };

    let struct_fields = state_codegen.struct_fields;
    let with_state_method = state_codegen.with_state_method;
    let constructor_default = state_codegen.constructor_default;
    let state_resolution = state_codegen.state_resolution;

    let struct_doc =
        format!("Auto-generated resource provider for the `{fn_name_str}` MCP resource.");
    let ctor_doc = format!(
        "Create a new instance of the [`{}`] resource provider.",
        struct_name
    );
    let expanded = quote! {
        // Emit the original function body under an internal name.
        #impl_fn

        #[doc = #struct_doc]
        #[derive(Clone)]
        pub struct #struct_name {
            #struct_fields
        }

        impl std::fmt::Debug for #struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!(#struct_name)).finish()
            }
        }

        #[pmcp::async_trait]
        impl pmcp::server::dynamic_resources::DynamicResourceProvider for #struct_name {
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
                #state_resolution
                #(#uri_param_extraction_code)*
                #fn_call
                Ok(pmcp::types::ReadResourceResult::new(
                    vec![pmcp::types::Content::text(content_str)]
                ))
            }
        }

        impl #struct_name {
            #with_state_method
        }

        #[doc = #ctor_doc]
        pub fn #fn_name() -> #struct_name {
            #constructor_default
        }
    };

    Ok(expanded)
}
