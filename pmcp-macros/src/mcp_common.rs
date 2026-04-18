//! Shared codegen utilities for `#[mcp_tool]` and `#[mcp_server]` macros.
//!
//! Provides parameter classification, type detection, and schema generation
//! helpers used by both standalone and impl-block macro expansions.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{FnArg, GenericParam, Generics, Type, TypePath};

/// Parameter slot for call-site argument ordering.
///
/// Used by `#[mcp_tool]`, `#[mcp_prompt]`, and `#[mcp_server]` to track
/// the user's declared parameter order and generate correct call-site code.
#[derive(Debug, Clone, Copy)]
pub enum ParamSlot {
    Args,
    State,
    Extra,
}

/// Role of a function parameter in an `#[mcp_tool]` function.
#[derive(Debug)]
pub enum ParamRole {
    /// First struct implementing `JsonSchema` + `Deserialize` = tool input args.
    Args(Type),
    /// `State<T>` = shared state injection.
    State {
        /// The inner type `T` from `State<T>`.
        inner_ty: Type,
    },
    /// `RequestHandlerExtra` = cancellation/progress/auth context.
    Extra,
    /// `&self` in impl block = server instance.
    SelfRef,
}

/// Classify a function parameter by its type.
///
/// Determines whether a parameter is:
/// - `State<T>` (shared state injection)
/// - `RequestHandlerExtra` (cancellation/progress/auth)
/// - `&self` (impl block receiver)
/// - Any other type (tool input args)
pub fn classify_param(param: &FnArg) -> syn::Result<ParamRole> {
    match param {
        FnArg::Receiver(_) => Ok(ParamRole::SelfRef),
        FnArg::Typed(pat_type) => {
            let ty = &*pat_type.ty;
            if type_name_matches(ty, "State") {
                let inner = extract_state_inner(ty)?;
                Ok(ParamRole::State { inner_ty: inner })
            } else if type_name_matches(ty, "RequestHandlerExtra") {
                Ok(ParamRole::Extra)
            } else {
                Ok(ParamRole::Args(ty.clone()))
            }
        },
    }
}

/// Check if the last segment of a type path matches the given name.
/// Handles qualified paths like `pmcp::State<T>` by checking only the final segment.
pub fn type_name_matches(ty: &Type, name: &str) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        path.segments.last().is_some_and(|s| s.ident == name)
    } else {
        false
    }
}

/// Extract inner `T` from `State<T>`.
pub fn extract_state_inner(ty: &Type) -> syn::Result<Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                    return Ok(inner.clone());
                }
            }
        }
    }
    Err(syn::Error::new_spanned(
        ty,
        "Expected State<T> with a type parameter",
    ))
}

/// Check if a type is `serde_json::Value` or a common alias.
///
/// Per D-15: skip `outputSchema` generation for `Result<Value>` returns.
/// Matches: `Value`, `JsonValue`, `serde_json::Value`.
///
/// This is a best-effort heuristic — proc macros cannot resolve type aliases,
/// so a user-defined `struct Value` would be a false positive.
pub fn is_value_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        let segments = &path.segments;
        match segments.len() {
            1 => {
                let ident = &segments[0].ident;
                ident == "Value" || ident == "JsonValue"
            },
            2 => segments[0].ident == "serde_json" && segments[1].ident == "Value",
            _ => false,
        }
    } else {
        false
    }
}

/// Extract the Ok type from `Result<T>` or `Result<T, E>`.
pub fn extract_result_ok_type(ty: &Type) -> Option<Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(ok_type)) = args.args.first() {
                        return Some(ok_type.clone());
                    }
                }
            }
        }
    }
    None
}

/// Generate input schema code for a given args type.
///
/// Uses `schemars::schema_for!` and `normalize_schema`.
pub fn generate_input_schema_code(args_type: &Type) -> TokenStream {
    quote! {
        {
            let schema = schemars::schema_for!(#args_type);
            let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
                serde_json::json!({"type": "object", "properties": {}})
            });
            pmcp::server::schema_utils::normalize_schema(json_schema)
        }
    }
}

/// Generate output schema code for a typed output (not `Value`).
pub fn generate_output_schema_code(output_type: &Type) -> TokenStream {
    quote! {
        {
            let schema = schemars::schema_for!(#output_type);
            Some(serde_json::to_value(&schema).unwrap_or_else(|_| {
                serde_json::json!({"type": "object"})
            }))
        }
    }
}

/// Generate output schema tokens from a return type.
///
/// If return type is `Result<T>` where T is not `Value`, generates schema code.
/// Otherwise returns `quote! { None }`.
pub fn output_schema_tokens(return_type: Option<&Type>) -> TokenStream {
    let Some(rt) = return_type else {
        return quote! { None };
    };
    if let Some(ok_type) = extract_result_ok_type(rt) {
        if is_value_type(&ok_type) {
            quote! { None }
        } else {
            generate_output_schema_code(&ok_type)
        }
    } else {
        quote! { None }
    }
}

/// Generate empty schema for no-args tools.
pub fn generate_empty_schema_code() -> TokenStream {
    quote! {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
}

/// Add Send + Sync + 'static bounds to all type parameters.
/// Used by `#[mcp_server]` to ensure handler structs are thread-safe.
pub fn add_async_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(syn::parse_quote!(Send));
            type_param.bounds.push(syn::parse_quote!(Sync));
            type_param.bounds.push(syn::parse_quote!('static));
        }
    }
    generics
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_classify_param_state() {
        let param: FnArg = parse_quote!(db: State<Database>);
        let role = classify_param(&param).unwrap();
        match role {
            ParamRole::State { inner_ty, .. } => {
                let inner_str = quote!(#inner_ty).to_string();
                assert_eq!(inner_str, "Database");
            },
            _ => panic!("Expected ParamRole::State, got {:?}", role),
        }
    }

    #[test]
    fn test_classify_param_extra() {
        let param: FnArg = parse_quote!(extra: RequestHandlerExtra);
        let role = classify_param(&param).unwrap();
        assert!(matches!(role, ParamRole::Extra));
    }

    #[test]
    fn test_classify_param_args() {
        let param: FnArg = parse_quote!(args: CalculatorArgs);
        let role = classify_param(&param).unwrap();
        match role {
            ParamRole::Args(ty) => {
                let ty_str = quote!(#ty).to_string();
                assert_eq!(ty_str, "CalculatorArgs");
            },
            _ => panic!("Expected ParamRole::Args, got {:?}", role),
        }
    }

    #[test]
    fn test_classify_param_self_ref() {
        let param: FnArg = parse_quote!(&self);
        let role = classify_param(&param).unwrap();
        assert!(matches!(role, ParamRole::SelfRef));
    }

    #[test]
    fn test_is_value_type_true() {
        let ty: Type = parse_quote!(Value);
        assert!(is_value_type(&ty));
    }

    #[test]
    fn test_is_value_type_json_value_alias() {
        let ty: Type = parse_quote!(JsonValue);
        assert!(
            is_value_type(&ty),
            "should recognize JsonValue alias for serde_json::Value"
        );
    }

    #[test]
    fn test_is_value_type_fully_qualified() {
        let ty: Type = parse_quote!(serde_json::Value);
        assert!(
            is_value_type(&ty),
            "should recognize fully qualified serde_json::Value"
        );
    }

    #[test]
    fn test_is_value_type_false() {
        let ty: Type = parse_quote!(CalculatorResult);
        assert!(!is_value_type(&ty));
    }

    #[test]
    fn test_is_value_type_false_other_value() {
        // A user-defined type named MyValue should NOT match
        let ty: Type = parse_quote!(MyValue);
        assert!(!is_value_type(&ty));
    }

    #[test]
    fn test_extract_result_ok_type() {
        let ty: Type = parse_quote!(Result<String, Error>);
        let ok_type = extract_result_ok_type(&ty).unwrap();
        let ok_str = quote!(#ok_type).to_string();
        assert_eq!(ok_str, "String");
    }

    #[test]
    fn test_extract_result_ok_type_single_param() {
        let ty: Type = parse_quote!(Result<String>);
        let ok_type = extract_result_ok_type(&ty).unwrap();
        let ok_str = quote!(#ok_type).to_string();
        assert_eq!(ok_str, "String");
    }

    #[test]
    fn test_extract_result_ok_type_not_result() {
        let ty: Type = parse_quote!(String);
        assert!(extract_result_ok_type(&ty).is_none());
    }

    #[test]
    fn test_extract_state_inner() {
        let ty: Type = parse_quote!(State<Database>);
        let inner = extract_state_inner(&ty).unwrap();
        let inner_str = quote!(#inner).to_string();
        assert_eq!(inner_str, "Database");
    }

    #[test]
    fn test_extract_state_inner_error_no_params() {
        let ty: Type = parse_quote!(State);
        assert!(extract_state_inner(&ty).is_err());
    }

    #[test]
    fn test_type_name_matches_state_qualified() {
        let ty: Type = parse_quote!(pmcp::State<Database>);
        assert!(type_name_matches(&ty, "State"));
    }

    #[test]
    fn test_type_name_matches_extra_qualified() {
        let ty: Type = parse_quote!(pmcp::RequestHandlerExtra);
        assert!(type_name_matches(&ty, "RequestHandlerExtra"));
    }
}

/// Error message used when `#[mcp_tool]` has neither a `description = "..."`
/// attribute nor a rustdoc comment. Kept as a `pub(crate) const` so both
/// parse sites emit byte-identical diagnostics.
pub(crate) const MCP_TOOL_MISSING_DESCRIPTION_ERROR: &str =
    "mcp_tool requires either a `description = \"...\"` attribute or a rustdoc comment on the function";

/// Build a synthetic `description = "..."` nested-meta from a plain string.
///
/// Uses `syn::LitStr::new` + `parse_quote!` rather than string formatting
/// to guarantee correct handling of embedded quotes, backslashes, and
/// arbitrary UTF-8.
///
/// Visibility is `pub(crate)` for testability only — production call sites
/// MUST route through `resolve_tool_args`, not call this helper directly.
pub(crate) fn build_description_meta(desc: &str) -> darling::ast::NestedMeta {
    let lit = syn::LitStr::new(desc, proc_macro2::Span::call_site());
    let meta: syn::Meta = syn::parse_quote! { description = #lit };
    darling::ast::NestedMeta::Meta(meta)
}

/// True iff `metas` already contains a `description = ...` NameValue entry.
/// Note: empty string `description = ""` counts as present — explicit empty
/// wins silently over rustdoc.
pub(crate) fn has_description_meta(metas: &[darling::ast::NestedMeta]) -> bool {
    metas.iter().any(|m| {
        matches!(
            m,
            darling::ast::NestedMeta::Meta(syn::Meta::NameValue(nv))
                if nv.path.is_ident("description")
        )
    })
}

/// Shared resolver — the ONE place where rustdoc fallback logic lives.
///
/// Both `#[mcp_tool]` parse sites (`mcp_tool.rs::expand_mcp_tool` and
/// `mcp_server.rs::parse_mcp_tool_attr`) call this function. Do NOT inline
/// any of its steps at the call sites — doing so reintroduces the drift
/// risk resolved by this function's existence.
///
/// # Arguments
/// - `args_tokens`: the `(...)` inside `#[mcp_tool(...)]`. Pass an empty
///   `TokenStream` for `#[mcp_tool]` with no parens.
/// - `item_attrs`: the annotated item's attributes (function-level for the
///   standalone site, method-level for the impl-block site) — the rustdoc
///   harvest source.
/// - `error_span_ident`: the identifier used for the `Span` of the
///   missing-description compile error (typically `&sig.ident`).
///
/// # Returns
/// A `Vec<NestedMeta>` guaranteed to contain a `description = ...` entry,
/// ready to feed into `McpToolArgs::from_list`. Or `Err` with the canonical
/// missing-description error if neither source supplied a description.
pub(crate) fn resolve_tool_args(
    args_tokens: proc_macro2::TokenStream,
    item_attrs: &[syn::Attribute],
    error_span_ident: &syn::Ident,
) -> syn::Result<Vec<darling::ast::NestedMeta>> {
    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let mut nested_metas: Vec<darling::ast::NestedMeta> = parser
        .parse2(args_tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut has_desc = has_description_meta(&nested_metas);
    if !has_desc {
        if let Some(doc_desc) = pmcp_macros_support::rustdoc::extract_doc_description(item_attrs) {
            nested_metas.push(build_description_meta(&doc_desc));
            has_desc = true;
        }
    }

    if !has_desc {
        return Err(syn::Error::new_spanned(
            error_span_ident,
            MCP_TOOL_MISSING_DESCRIPTION_ERROR,
        ));
    }

    Ok(nested_metas)
}

#[cfg(test)]
mod rustdoc_fallback_tests {
    use super::{
        build_description_meta, has_description_meta, resolve_tool_args,
        MCP_TOOL_MISSING_DESCRIPTION_ERROR,
    };
    use quote::ToTokens;

    fn sample_ident() -> syn::Ident {
        syn::Ident::new("sample", proc_macro2::Span::call_site())
    }

    fn doc_attrs(lines: &[&str]) -> Vec<syn::Attribute> {
        lines
            .iter()
            .map(|line| {
                let lit = syn::LitStr::new(line, proc_macro2::Span::call_site());
                syn::parse_quote! { #[doc = #lit] }
            })
            .collect()
    }

    fn description_value(metas: &[darling::ast::NestedMeta]) -> Option<String> {
        metas.iter().find_map(|m| {
            if let darling::ast::NestedMeta::Meta(syn::Meta::NameValue(nv)) = m {
                if nv.path.is_ident("description") {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        return Some(s.value());
                    }
                }
            }
            None
        })
    }

    #[test]
    fn resolve_empty_args_with_rustdoc_synthesizes() {
        let attrs = doc_attrs(&[" Hello from rustdoc."]);
        let ident = sample_ident();
        let metas = resolve_tool_args(proc_macro2::TokenStream::new(), &attrs, &ident)
            .expect("should synthesize from rustdoc");
        assert_eq!(
            description_value(&metas).as_deref(),
            Some("Hello from rustdoc.")
        );
    }

    #[test]
    fn resolve_with_description_ignores_rustdoc() {
        let attrs = doc_attrs(&[" IGNORED"]);
        let ident = sample_ident();
        let args: proc_macro2::TokenStream = syn::parse_str(r#"description = "WINS""#).unwrap();
        let metas = resolve_tool_args(args.to_token_stream(), &attrs, &ident)
            .expect("attribute should win");
        assert_eq!(description_value(&metas).as_deref(), Some("WINS"));
    }

    #[test]
    fn resolve_neither_present_errors_with_canonical_wording() {
        let ident = sample_ident();
        let err = resolve_tool_args(proc_macro2::TokenStream::new(), &[], &ident)
            .expect_err("should error");
        let msg = err.to_string();
        assert!(
            msg.contains("mcp_tool requires either"),
            "error should contain canonical prefix, got: {msg}"
        );
        assert!(
            msg.contains("or a rustdoc comment on the function"),
            "error should mention rustdoc fallback, got: {msg}"
        );
    }

    #[test]
    fn resolve_empty_string_description_with_rustdoc_keeps_empty() {
        // MEDIUM-3 semantic: `description = ""` is PRESENT, so rustdoc is
        // NOT consulted and the empty string passes through.
        let attrs = doc_attrs(&[" IGNORED rustdoc."]);
        let ident = sample_ident();
        let args: proc_macro2::TokenStream = syn::parse_str(r#"description = """#).unwrap();
        let metas = resolve_tool_args(args.to_token_stream(), &attrs, &ident)
            .expect("empty string is valid input");
        assert_eq!(description_value(&metas).as_deref(), Some(""));
    }

    #[test]
    fn resolve_empty_string_description_no_rustdoc_keeps_empty() {
        // Same as above but NO rustdoc. Empty string still counts as
        // present — no error, no synthesis.
        let ident = sample_ident();
        let args: proc_macro2::TokenStream = syn::parse_str(r#"description = """#).unwrap();
        let metas = resolve_tool_args(args.to_token_stream(), &[], &ident)
            .expect("empty string is valid input");
        assert_eq!(description_value(&metas).as_deref(), Some(""));
    }

    #[test]
    fn mcp_tool_missing_error_wording_exact() {
        assert_eq!(
            MCP_TOOL_MISSING_DESCRIPTION_ERROR,
            "mcp_tool requires either a `description = \"...\"` attribute or a rustdoc comment on the function"
        );
    }

    #[test]
    fn has_description_meta_true_when_present() {
        let meta = build_description_meta("hello");
        assert!(has_description_meta(&[meta]));
    }

    #[test]
    fn has_description_meta_false_when_absent() {
        let metas: Vec<darling::ast::NestedMeta> = Vec::new();
        assert!(!has_description_meta(&metas));
    }

    #[test]
    fn build_description_meta_roundtrips_embedded_quotes() {
        let meta = build_description_meta("he said \"hi\"");
        assert!(has_description_meta(&[meta]));
    }

    #[test]
    fn resolve_skips_include_str_doc_and_errors_if_no_other_source() {
        // `#[doc = include_str!("...")]` is Expr::Macro, not Expr::Lit(Str).
        // The support-crate helper skips it silently. With no other
        // description source, the resolver errors with the canonical wording.
        let attr: syn::Attribute = syn::parse_quote! { #[doc = include_str!("nonexistent.md")] };
        let ident = sample_ident();
        let err = resolve_tool_args(proc_macro2::TokenStream::new(), &[attr], &ident)
            .expect_err("include_str!() doc must not satisfy description");
        assert!(
            err.to_string().contains("mcp_tool requires either"),
            "include_str!-only should trigger canonical error"
        );
    }

    #[test]
    fn resolve_skips_cfg_attr_doc_and_errors_if_no_other_source() {
        // `#[cfg_attr(doc, doc = "...")]` is a `#[cfg_attr(...)]` attribute
        // whose path is `cfg_attr`, NOT `doc`. The harvest helper only
        // inspects attrs whose path is `doc`, so this is silently skipped.
        let attr: syn::Attribute = syn::parse_quote! { #[cfg_attr(doc, doc = "conditional doc")] };
        let ident = sample_ident();
        let err = resolve_tool_args(proc_macro2::TokenStream::new(), &[attr], &ident)
            .expect_err("cfg_attr-gated doc must not satisfy description");
        assert!(
            err.to_string().contains("mcp_tool requires either"),
            "cfg_attr-only should trigger canonical error"
        );
    }
}
