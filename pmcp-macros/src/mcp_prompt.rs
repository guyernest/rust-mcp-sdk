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

use crate::mcp_common;
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

/// Expand `#[mcp_prompt]` attribute macro on a standalone function.
pub fn expand_mcp_prompt(args: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
    // Parse macro attributes via darling.
    let nested_metas = if args.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig.ident,
            "mcp_prompt requires at least `description = \"...\"` attribute",
        ));
    } else {
        let parser = syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
        parser
            .parse2(args)
            .map(|p| p.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
    };

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

    // Classify all parameters.
    let mut args_type: Option<syn::Type> = None;
    let mut state_inner_ty: Option<syn::Type> = None;
    let mut has_extra = false;

    // Track parameter order for correct call-site argument passing.
    #[derive(Debug, Clone, Copy)]
    enum ParamSlot {
        Args,
        State,
        Extra,
    }
    let mut param_order: Vec<ParamSlot> = Vec::new();

    for param in &input.sig.inputs {
        let role = mcp_common::classify_param(param)?;
        match role {
            mcp_common::ParamRole::Args(ty) => {
                if args_type.is_some() {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_prompt functions can have at most one args parameter",
                    ));
                }
                args_type = Some(ty);
                param_order.push(ParamSlot::Args);
            }
            mcp_common::ParamRole::State { inner_ty, .. } => {
                if state_inner_ty.is_some() {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_prompt functions can have at most one State<T> parameter",
                    ));
                }
                state_inner_ty = Some(inner_ty);
                param_order.push(ParamSlot::State);
            }
            mcp_common::ParamRole::Extra => {
                if has_extra {
                    return Err(syn::Error::new_spanned(
                        param,
                        "mcp_prompt functions can have at most one RequestHandlerExtra parameter",
                    ));
                }
                has_extra = true;
                param_order.push(ParamSlot::Extra);
            }
            mcp_common::ParamRole::SelfRef => {
                return Err(syn::Error::new_spanned(
                    param,
                    "standalone #[mcp_prompt] functions cannot have &self -- use #[mcp_server] for impl block prompts",
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

    // Generate args deserialization using shared runtime helper.
    let args_deser = if let Some(ref at) = args_type {
        let prompt_name_for_err = &prompt_name;
        quote! {
            let typed_args: #at = pmcp::server::typed_prompt::deserialize_prompt_args(args, #prompt_name_for_err)?;
        }
    } else {
        quote! {}
    };

    // Generate state resolution in handle().
    let state_resolution = if let Some(ref inner) = state_inner_ty {
        let inner_name = quote!(#inner).to_string();
        let prompt_name_for_err = &prompt_name;
        quote! {
            let state_val = pmcp::State(
                self.state.as_ref()
                    .ok_or_else(|| pmcp::Error::internal(format!(
                        "State<{}> not provided for prompt '{}' -- call .with_state() during registration",
                        #inner_name, #prompt_name_for_err
                    )))?
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

    // Generate handle body.
    let handle_body = quote! {
        #args_deser
        #state_resolution
        #fn_call
    };

    // Generate metadata() body.
    let metadata_body = if let Some(ref at) = args_type {
        // Generate argument extraction using shared runtime helper.
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
        // No-args prompt: skip schema extraction entirely.
        quote! {
            fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
                Some(pmcp::types::PromptInfo::new(#prompt_name)
                    .with_description(#description))
            }
        }
    };

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
