// Allow needless_continue from darling's generated derive code
#![allow(clippy::needless_continue)]

//! Derive macro for Code Mode validation and execution in MCP servers.
//!
//! Provides `#[derive(CodeMode)]` which generates a `register_code_mode_tools`
//! method that registers `validate_code` and `execute_code` tools on a
//! [`pmcp::ServerBuilder`].
//!
//! # Field Name Convention (v0.1.0)
//!
//! The macro identifies required fields by **fixed well-known names**. This is
//! the v0.1.0 contract -- the field names are the API:
//!
//! | Field Name | Required Type | Purpose |
//! |------------|---------------|---------|
//! | `code_mode_config` | `CodeModeConfig` | Validation pipeline configuration |
//! | `token_secret` | `TokenSecret` | HMAC signing secret |
//! | `policy_evaluator` | `Arc<dyn PolicyEvaluator>` or `Arc<P>` | Policy evaluation |
//! | `code_executor` | `Arc<dyn CodeExecutor>` or `Arc<E>` | Code execution |
//!
//! If any required field is missing, the macro emits a **single** compile error
//! listing all absent fields.
//!
//! # Struct-Level Attributes (v0.2.0)
//!
//! | Attribute | Type | Default | Purpose |
//! |-----------|------|---------|---------|
//! | `context_from` | `String` | (none) | Method name returning `ValidationContext` |
//! | `language` | `String` | `"graphql"` | Code language for tool metadata |
//!
//! When `context_from` is specified, `register_code_mode_tools` requires
//! `self: &Arc<Self>` and the generated handler calls `self.parent.{method}(&extra)`
//! to obtain real `ValidationContext` bound to the current user/session.
//!
//! When `context_from` is omitted, `register_code_mode_tools` uses `&self` (no Arc
//! required) with placeholder context values and a `#[deprecated]` warning guiding
//! users toward the production path.
//!
//! # Generated Code
//!
//! The macro generates:
//!
//! 1. A `register_code_mode_tools` method on the struct that takes a
//!    `ServerBuilder` **by value** and returns it (by-value fluent pattern).
//! 2. Two internal handler structs (`ValidateCodeHandler` and
//!    `ExecuteCodeHandler`) that implement `pmcp::ToolHandler`.
//! 3. A `Send + Sync` compile-time assertion (per D-08).
//!
//! # Examples
//!
//! **Production (with `context_from`):**
//! ```rust,ignore
//! #[derive(CodeMode)]
//! #[code_mode(context_from = "get_context", language = "graphql")]
//! struct MyServer {
//!     code_mode_config: CodeModeConfig,
//!     token_secret: TokenSecret,
//!     policy_evaluator: Arc<NoopPolicyEvaluator>,
//!     code_executor: Arc<MyExecutor>,
//! }
//!
//! impl MyServer {
//!     fn get_context(&self, extra: &RequestHandlerExtra) -> ValidationContext {
//!         ValidationContext::new("user-1", "session-1", "schema-v1", "perms-v1")
//!     }
//! }
//!
//! // Generated: MyServer::register_code_mode_tools(self: &Arc<Self>, builder) -> ServerBuilder
//! ```
//!
//! **Testing (without `context_from`, deprecated placeholder path):**
//! ```rust,ignore
//! #[derive(CodeMode)]
//! struct MyServer {
//!     code_mode_config: CodeModeConfig,
//!     token_secret: TokenSecret,
//!     policy_evaluator: Arc<NoopPolicyEvaluator>,
//!     code_executor: Arc<MyExecutor>,
//! }
//!
//! // Generated: #[deprecated] MyServer::register_code_mode_tools(&self, builder) -> ServerBuilder
//! ```

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Required field names for the Code Mode derive macro.
const REQUIRED_FIELDS: &[&str] = &[
    "code_mode_config",
    "token_secret",
    "policy_evaluator",
    "code_executor",
];

/// Parsed attributes from `#[derive(CodeMode)]`.
///
/// Struct-level attributes (v0.2.0):
/// - `context_from`: Optional method name for extracting `ValidationContext`.
///   When specified, the generated `register_code_mode_tools` requires `self: &Arc<Self>`.
/// - `language`: Code language for tool metadata. Defaults to `"graphql"`.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(code_mode))]
struct CodeModeOpts {
    ident: syn::Ident,
    data: darling::ast::Data<(), CodeModeField>,
    /// Optional method name for extracting `ValidationContext` from the struct.
    /// When specified, the generated registration method requires `self: &Arc<Self>`.
    #[darling(default)]
    context_from: Option<String>,
    /// Code language for tool metadata. Defaults to `"graphql"`.
    #[darling(default)]
    language: Option<String>,
}

/// A single field parsed from the struct.
#[derive(Debug, Clone, darling::FromField)]
#[darling(attributes(code_mode))]
struct CodeModeField {
    ident: Option<syn::Ident>,
}

/// Derive macro that generates `register_code_mode_tools` for Code Mode servers.
///
/// Requires a named struct with four well-known fields:
/// `code_mode_config`, `token_secret`, `policy_evaluator`, `code_executor`.
///
/// See [crate-level documentation](crate) for the full field name convention.
#[proc_macro_derive(CodeMode, attributes(code_mode))]
pub fn code_mode_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_code_mode(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Core expansion logic for `#[derive(CodeMode)]`.
fn expand_code_mode(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let opts = CodeModeOpts::from_derive_input(input)
        .map_err(|e| syn::Error::new_spanned(input, e.to_string()))?;

    let struct_name = &opts.ident;

    // Extract named fields
    let fields = match &opts.data {
        darling::ast::Data::Struct(ref fields) => &fields.fields,
        darling::ast::Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(CodeMode)] can only be applied to structs with named fields",
            ));
        },
    };

    let field_names: Vec<String> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    // Check for missing required fields
    let missing: Vec<&str> = REQUIRED_FIELDS
        .iter()
        .filter(|&&name| !field_names.contains(&name.to_string()))
        .copied()
        .collect();

    if !missing.is_empty() {
        let missing_list = missing.join(", ");
        let all_required = REQUIRED_FIELDS.join(", ");
        let msg = format!(
            "#[derive(CodeMode)] is missing required field(s): `{missing_list}`.\n\
             Required fields: {all_required}"
        );
        return Err(syn::Error::new_spanned(&input.ident, msg));
    }

    // Generate the handler module name to avoid collision (snake_case to suppress warnings)
    let mod_name = syn::Ident::new(
        &format!(
            "__code_mode_impl_{}",
            struct_name.to_string().to_lowercase()
        ),
        struct_name.span(),
    );

    // Extract language (defaults to "graphql" for backward compat)
    let language = opts.language.as_deref().unwrap_or("graphql");
    let language_lit = syn::LitStr::new(language, struct_name.span());

    // Branch code generation based on context_from presence
    let expanded = if let Some(ref method_name) = opts.context_from {
        // --- context_from path: real ValidationContext from user method ---
        let method_ident = syn::Ident::new(method_name, struct_name.span());
        expand_with_context_from(struct_name, &mod_name, &language_lit, &method_ident)
    } else {
        // --- default path: placeholder context with deprecation warning ---
        expand_without_context_from(struct_name, &mod_name, &language_lit)
    };

    Ok(expanded)
}

/// Generate code when `context_from` is specified.
///
/// The registration method requires `self: &Arc<Self>` and the validate handler
/// calls `self.parent.{method_ident}(&extra)` for real `ValidationContext`.
fn expand_with_context_from(
    struct_name: &syn::Ident,
    mod_name: &syn::Ident,
    language_lit: &syn::LitStr,
    method_ident: &syn::Ident,
) -> proc_macro2::TokenStream {
    quote! {
        // Send + Sync compile-time assertion (D-08)
        const _: fn() = || {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<#struct_name>();
        };

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use super::*;
            use std::sync::Arc;
            // Import TokenGenerator trait to bring verify/verify_code into scope
            use pmcp_code_mode::TokenGenerator as _;

            /// Internal state for the `validate_code` tool handler.
            pub(super) struct ValidateCodeHandler {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) config: pmcp_code_mode::CodeModeConfig,
                pub(super) parent: Arc<#struct_name>,
            }

            #[pmcp_code_mode::async_trait]
            impl pmcp::ToolHandler for ValidateCodeHandler {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ValidateCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();
                    let dry_run = input.dry_run.unwrap_or(false);

                    // Real ValidationContext from user-defined method
                    let context = self.parent.#method_ident(&extra);

                    let result = self.pipeline.validate_graphql_query_async(code, &context)
                        .await
                        .map_err(|e| pmcp::Error::Internal(format!("Validation error: {}", e)))?;

                    let response = pmcp_code_mode::ValidationResponse::success(
                        result.explanation.clone(),
                        result.risk_level,
                        if dry_run {
                            String::new()
                        } else {
                            result.approval_token.clone().unwrap_or_default()
                        },
                        result.metadata.clone(),
                    )
                    .with_warnings(result.warnings.clone())
                    .with_auto_approved(self.config.should_auto_approve(result.risk_level));

                    let (json, _is_error) = response.to_json_response();
                    Ok(json)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    let tools = pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_tools();
                    tools.into_iter().find(|t| t.name == "validate_code")
                }
            }

            /// Internal state for the `execute_code` tool handler.
            pub(super) struct ExecuteCodeHandler<E: pmcp_code_mode::CodeExecutor + 'static> {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) executor: Arc<E>,
            }

            #[pmcp_code_mode::async_trait]
            impl<E: pmcp_code_mode::CodeExecutor + 'static> pmcp::ToolHandler for ExecuteCodeHandler<E> {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ExecuteCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();

                    // Verify the approval token
                    let token_gen = self.pipeline.token_generator();
                    let token = pmcp_code_mode::ApprovalToken::decode(&input.approval_token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Invalid approval token: {}", e),
                        ))?;

                    // Verify token signature and expiry
                    token_gen.verify(&token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Token verification failed: {}", e),
                        ))?;

                    // Verify code matches the token's code hash
                    token_gen.verify_code(code, &token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Code verification failed: {}", e),
                        ))?;

                    // Execute the validated code
                    let result = self.executor.execute(code, input.variables.as_ref()).await
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Execution error: {}", e),
                        ))?;

                    Ok(result)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    let tools = pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_tools();
                    tools.into_iter().find(|t| t.name == "execute_code")
                }
            }
        }

        impl #struct_name {
            /// Register Code Mode tools (`validate_code` + `execute_code`) on the builder.
            ///
            /// Uses the `context_from` method to extract real `ValidationContext` from
            /// each request. Requires `self: &Arc<Self>` to share the server reference
            /// with the generated handler.
            ///
            /// # Example
            ///
            /// ```rust,ignore
            /// let server = Arc::new(my_server);
            /// let builder = server.register_code_mode_tools(Server::builder());
            /// ```
            pub fn register_code_mode_tools(
                self: &std::sync::Arc<Self>,
                builder: pmcp::ServerBuilder,
            ) -> pmcp::ServerBuilder {
                let pipeline = std::sync::Arc::new(
                    pmcp_code_mode::ValidationPipeline::from_token_secret_with_policy(
                        self.code_mode_config.clone(),
                        &self.token_secret,
                        std::sync::Arc::clone(&self.policy_evaluator) as std::sync::Arc<dyn pmcp_code_mode::PolicyEvaluator>,
                    )
                );

                let validate_handler = #mod_name::ValidateCodeHandler {
                    pipeline: std::sync::Arc::clone(&pipeline),
                    config: self.code_mode_config.clone(),
                    parent: std::sync::Arc::clone(self),
                };

                let execute_handler = #mod_name::ExecuteCodeHandler {
                    pipeline,
                    executor: std::sync::Arc::clone(&self.code_executor),
                };

                builder
                    .tool("validate_code", validate_handler)
                    .tool("execute_code", execute_handler)
            }
        }
    }
}

/// Generate code when `context_from` is NOT specified (backward-compatible path).
///
/// The registration method uses `&self` (no `Arc` needed) and the validate handler
/// uses placeholder `ValidationContext` values with a `#[deprecated]` warning guiding
/// users toward the `context_from` attribute for production use.
fn expand_without_context_from(
    struct_name: &syn::Ident,
    mod_name: &syn::Ident,
    language_lit: &syn::LitStr,
) -> proc_macro2::TokenStream {
    quote! {
        // Send + Sync compile-time assertion (D-08)
        const _: fn() = || {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<#struct_name>();
        };

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use super::*;
            use std::sync::Arc;
            // Import TokenGenerator trait to bring verify/verify_code into scope
            use pmcp_code_mode::TokenGenerator as _;

            /// Internal state for the `validate_code` tool handler.
            pub(super) struct ValidateCodeHandler {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) config: pmcp_code_mode::CodeModeConfig,
            }

            #[pmcp_code_mode::async_trait]
            impl pmcp::ToolHandler for ValidateCodeHandler {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ValidateCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();
                    let dry_run = input.dry_run.unwrap_or(false);

                    // WARNING: These are PLACEHOLDER values. The validation context
                    // uses static strings, so approval tokens are NOT bound to a
                    // specific user, session, or schema version. An attacker who
                    // obtains a valid token can replay it across different users and
                    // sessions until it expires.
                    //
                    // Use `#[code_mode(context_from = "method_name")]` for production.
                    let context = pmcp_code_mode::ValidationContext::new(
                        "anonymous",
                        "session",
                        "schema",
                        "perms",
                    );

                    let result = self.pipeline.validate_graphql_query_async(code, &context)
                        .await
                        .map_err(|e| pmcp::Error::Internal(format!("Validation error: {}", e)))?;

                    let response = pmcp_code_mode::ValidationResponse::success(
                        result.explanation.clone(),
                        result.risk_level,
                        if dry_run {
                            String::new()
                        } else {
                            result.approval_token.clone().unwrap_or_default()
                        },
                        result.metadata.clone(),
                    )
                    .with_warnings(result.warnings.clone())
                    .with_auto_approved(self.config.should_auto_approve(result.risk_level));

                    let (json, _is_error) = response.to_json_response();
                    Ok(json)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    let tools = pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_tools();
                    tools.into_iter().find(|t| t.name == "validate_code")
                }
            }

            /// Internal state for the `execute_code` tool handler.
            pub(super) struct ExecuteCodeHandler<E: pmcp_code_mode::CodeExecutor + 'static> {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) executor: Arc<E>,
            }

            #[pmcp_code_mode::async_trait]
            impl<E: pmcp_code_mode::CodeExecutor + 'static> pmcp::ToolHandler for ExecuteCodeHandler<E> {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ExecuteCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();

                    // Verify the approval token
                    let token_gen = self.pipeline.token_generator();
                    let token = pmcp_code_mode::ApprovalToken::decode(&input.approval_token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Invalid approval token: {}", e),
                        ))?;

                    // Verify token signature and expiry
                    token_gen.verify(&token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Token verification failed: {}", e),
                        ))?;

                    // Verify code matches the token's code hash
                    token_gen.verify_code(code, &token)
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Code verification failed: {}", e),
                        ))?;

                    // Execute the validated code
                    let result = self.executor.execute(code, input.variables.as_ref()).await
                        .map_err(|e| pmcp::Error::Internal(
                            format!("Execution error: {}", e),
                        ))?;

                    Ok(result)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    let tools = pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_tools();
                    tools.into_iter().find(|t| t.name == "execute_code")
                }
            }
        }

        impl #struct_name {
            /// Register Code Mode tools (`validate_code` + `execute_code`) on the builder.
            ///
            /// **Deprecated:** Uses placeholder `ValidationContext` values. Use
            /// `#[code_mode(context_from = "method_name")]` for production to bind
            /// approval tokens to real user identity and session.
            ///
            /// # Example
            ///
            /// ```rust,ignore
            /// #[allow(deprecated)]
            /// let builder = server.register_code_mode_tools(Server::builder());
            /// ```
            #[deprecated(note = "Use #[code_mode(context_from = \"method_name\")] for production. This uses placeholder ValidationContext.")]
            pub fn register_code_mode_tools(
                &self,
                builder: pmcp::ServerBuilder,
            ) -> pmcp::ServerBuilder {
                let pipeline = std::sync::Arc::new(
                    pmcp_code_mode::ValidationPipeline::from_token_secret_with_policy(
                        self.code_mode_config.clone(),
                        &self.token_secret,
                        std::sync::Arc::clone(&self.policy_evaluator) as std::sync::Arc<dyn pmcp_code_mode::PolicyEvaluator>,
                    )
                );

                let validate_handler = #mod_name::ValidateCodeHandler {
                    pipeline: std::sync::Arc::clone(&pipeline),
                    config: self.code_mode_config.clone(),
                };

                let execute_handler = #mod_name::ExecuteCodeHandler {
                    pipeline,
                    executor: std::sync::Arc::clone(&self.code_executor),
                };

                builder
                    .tool("validate_code", validate_handler)
                    .tool("execute_code", execute_handler)
            }
        }
    }
}

