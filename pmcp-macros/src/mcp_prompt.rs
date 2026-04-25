//! `#[mcp_prompt]` attribute macro expansion.
//!
//! Generates a struct implementing `PromptHandler` from an annotated standalone
//! async or sync function, eliminating boilerplate and providing automatic
//! argument schema generation and state injection.
//!
//! # Generated Code
//!
//! For each annotated function, the macro generates:
//! 1. The original function (preserved unchanged under an internal name)
//! 2. A `{PascalCase(fn_name)}Prompt` struct implementing `PromptHandler`
//! 3. A constructor function `fn fn_name() -> StructName` for ergonomic registration
//!
//! # Example
//!
//! ```rust,ignore
//! #[mcp_prompt(description = "Review code for quality issues")]
//! async fn code_review(args: ReviewArgs) -> Result<GetPromptResult> {
//!     Ok(GetPromptResult::new(
//!         vec![PromptMessage::user(Content::text(format!("Review {} code", args.language)))],
//!         None,
//!     ))
//! }
//!
//! // Register: server_builder.prompt("code_review", code_review())
//! ```

use crate::mcp_common::{self, ParamSlot};
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::ItemFn;

/// Parsed attributes for `#[mcp_prompt(...)]`.
#[derive(Debug, FromMeta)]
pub struct McpPromptArgs {
    /// Prompt description (mandatory).
    pub(crate) description: String,
    /// Override prompt name (defaults to function name).
    #[darling(default)]
    pub(crate) name: Option<String>,
}

/// Parse `#[mcp_prompt(...)]` attribute tokens, rejecting empty args.
fn parse_prompt_attr_args(
    args: TokenStream,
    fn_ident: &syn::Ident,
) -> syn::Result<Vec<darling::ast::NestedMeta>> {
    if args.is_empty() {
        return Err(syn::Error::new_spanned(
            fn_ident,
            "mcp_prompt requires at least `description = \"...\"` attribute",
        ));
    }
    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    Ok(parser
        .parse2(args)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default())
}

/// Bundled parameter classification for a `#[mcp_prompt]` function.
struct PromptFnParams {
    args_type: Option<syn::Type>,
    state_inner_ty: Option<syn::Type>,
    param_order: Vec<ParamSlot>,
    has_extra: bool,
}

/// Push an args-typed parameter slot, rejecting duplicates.
fn push_prompt_args_slot(
    param: &syn::FnArg,
    ty: syn::Type,
    args_type: &mut Option<syn::Type>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if args_type.is_some() {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_prompt functions can have at most one args parameter",
        ));
    }
    *args_type = Some(ty);
    param_order.push(ParamSlot::Args);
    Ok(())
}

/// Push a `State<T>` slot, rejecting duplicates.
fn push_prompt_state_slot(
    param: &syn::FnArg,
    inner_ty: syn::Type,
    state_inner_ty: &mut Option<syn::Type>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if state_inner_ty.is_some() {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_prompt functions can have at most one State<T> parameter",
        ));
    }
    *state_inner_ty = Some(inner_ty);
    param_order.push(ParamSlot::State);
    Ok(())
}

/// Push a `RequestHandlerExtra` slot, rejecting duplicates.
fn push_prompt_extra_slot(
    param: &syn::FnArg,
    has_extra: &mut bool,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if *has_extra {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_prompt functions can have at most one RequestHandlerExtra parameter",
        ));
    }
    *has_extra = true;
    param_order.push(ParamSlot::Extra);
    Ok(())
}

/// Classify every parameter of an `#[mcp_prompt]` function.
fn classify_prompt_fn_params(input: &ItemFn) -> syn::Result<PromptFnParams> {
    let mut args_type: Option<syn::Type> = None;
    let mut state_inner_ty: Option<syn::Type> = None;
    let mut has_extra = false;
    let mut param_order: Vec<ParamSlot> = Vec::new();
    for param in &input.sig.inputs {
        let role = mcp_common::classify_param(param)?;
        match role {
            mcp_common::ParamRole::Args(ty) => {
                push_prompt_args_slot(param, ty, &mut args_type, &mut param_order)?;
            },
            mcp_common::ParamRole::State { inner_ty, .. } => {
                push_prompt_state_slot(param, inner_ty, &mut state_inner_ty, &mut param_order)?;
            },
            mcp_common::ParamRole::Extra => {
                push_prompt_extra_slot(param, &mut has_extra, &mut param_order)?;
            },
            mcp_common::ParamRole::SelfRef => {
                return Err(syn::Error::new_spanned(
                    param,
                    "standalone #[mcp_prompt] functions cannot have &self -- use #[mcp_server] for impl block prompts",
                ));
            },
        }
    }
    Ok(PromptFnParams {
        args_type,
        state_inner_ty,
        param_order,
        has_extra,
    })
}

/// Bundled state codegen for standalone `mcp_prompt`.
struct PromptStateCodegen {
    struct_fields: TokenStream,
    with_state_method: TokenStream,
    constructor_default: TokenStream,
    state_resolution: TokenStream,
}

fn generate_prompt_state_codegen(
    state_inner_ty: Option<&syn::Type>,
    struct_name: &syn::Ident,
    prompt_name: &str,
) -> PromptStateCodegen {
    let Some(inner) = state_inner_ty else {
        return PromptStateCodegen {
            struct_fields: quote! {},
            with_state_method: quote! {},
            constructor_default: quote! { #struct_name {} },
            state_resolution: quote! {},
        };
    };
    let inner_name = quote!(#inner).to_string();
    PromptStateCodegen {
        struct_fields: quote! { state: Option<std::sync::Arc<#inner>>, },
        with_state_method: quote! {
            /// Provide shared state for this prompt.
            ///
            /// Call this at registration time to inject state:
            /// ```rust,ignore
            /// server_builder.prompt("name", prompt_fn().with_state(my_state))
            /// ```
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
                        "State<{}> not provided for prompt '{}' -- call .with_state() during registration",
                        #inner_name, #prompt_name
                    )))?
                    .clone()
            );
        },
    }
}

/// Generate the args deserialization snippet for a prompt handler body.
fn generate_prompt_args_deser(args_type: Option<&syn::Type>, prompt_name: &str) -> TokenStream {
    let Some(at) = args_type else {
        return quote! {};
    };
    quote! {
        let typed_args: #at = pmcp::server::typed_prompt::deserialize_prompt_args(args, #prompt_name)?;
    }
}

/// Generate the `metadata()` body for the prompt handler.
fn generate_prompt_metadata_body(
    args_type: Option<&syn::Type>,
    prompt_name: &str,
    description: &str,
) -> TokenStream {
    let Some(at) = args_type else {
        return quote! {
            fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
                Some(pmcp::types::PromptInfo::new(#prompt_name)
                    .with_description(#description))
            }
        };
    };
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
}

/// Expand `#[mcp_prompt]` attribute macro on a standalone function.
///
/// Refactored in 75-01 Task 1b-C (P1): parse/classify/codegen helpers
/// extracted ([`parse_prompt_attr_args`], [`classify_prompt_fn_params`],
/// [`generate_prompt_state_codegen`], [`generate_prompt_args_deser`],
/// [`generate_prompt_metadata_body`]).
pub fn expand_mcp_prompt(args: TokenStream, input: &ItemFn) -> syn::Result<TokenStream> {
    let nested_metas = parse_prompt_attr_args(args, &input.sig.ident)?;
    let macro_args = McpPromptArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;

    // Extract function info.
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let prompt_name = macro_args.name.unwrap_or_else(|| fn_name_str.clone());
    let is_async = input.sig.asyncness.is_some();
    let struct_name = format_ident!("{}Prompt", fn_name_str.to_upper_camel_case());
    let description = &macro_args.description;

    // Rename the original function to an internal name to avoid conflict
    // with the constructor function that uses the same name.
    let impl_fn_name = format_ident!("__{}_impl", fn_name_str);
    let mut impl_fn = input.clone();
    impl_fn.sig.ident = impl_fn_name.clone();

    let params = classify_prompt_fn_params(input)?;
    let state_codegen =
        generate_prompt_state_codegen(params.state_inner_ty.as_ref(), &struct_name, &prompt_name);

    let args_deser = generate_prompt_args_deser(params.args_type.as_ref(), &prompt_name);
    let struct_fields = state_codegen.struct_fields;
    let with_state_method = state_codegen.with_state_method;
    let constructor_default = state_codegen.constructor_default;
    let state_resolution = state_codegen.state_resolution;

    // Generate extra parameter name in handle() signature.
    let extra_param_name: Ident = if params.has_extra {
        format_ident!("extra")
    } else {
        format_ident!("_extra")
    };

    // Generate function call arguments in correct parameter order.
    let call_args: Vec<TokenStream> = params
        .param_order
        .iter()
        .map(|slot| match slot {
            ParamSlot::Args => quote! { typed_args },
            ParamSlot::State => quote! { state_val },
            ParamSlot::Extra => quote! { #extra_param_name },
        })
        .collect();

    // Generate the function call (async vs sync).
    // Prompts return GetPromptResult directly -- no serialization wrapper.
    let fn_call = if is_async {
        quote! { #impl_fn_name(#(#call_args),*).await }
    } else {
        quote! { #impl_fn_name(#(#call_args),*) }
    };

    let handle_body = quote! {
        #args_deser
        #state_resolution
        #fn_call
    };

    let metadata_body =
        generate_prompt_metadata_body(params.args_type.as_ref(), &prompt_name, description);

    // Assemble everything.
    let expanded = quote! {
        // Emit the original function body under an internal name to avoid
        // collision with the public constructor function.
        #impl_fn

        /// Auto-generated prompt handler for the `#fn_name` MCP prompt.
        ///
        /// **Note:** MCP prompt arguments are string-only. Struct fields should use
        /// `String` or `Option<String>`. Non-string types (`i32`, `bool`, etc.) will
        /// fail deserialization. Use `#[serde(deserialize_with)]` for custom parsing.
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
        impl pmcp::PromptHandler for #struct_name {
            async fn handle(
                &self,
                args: std::collections::HashMap<String, String>,
                #extra_param_name: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<pmcp::types::GetPromptResult> {
                #handle_body
            }

            #metadata_body
        }

        impl #struct_name {
            #with_state_method
        }

        /// Create a new instance of the [`#struct_name`] prompt handler.
        ///
        /// Use this at registration time:
        /// ```rust,ignore
        /// server_builder.prompt("#prompt_name", #fn_name())
        /// ```
        pub fn #fn_name() -> #struct_name {
            #constructor_default
        }
    };

    Ok(expanded)
}
