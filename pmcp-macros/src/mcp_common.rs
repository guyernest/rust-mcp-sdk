//! Shared codegen utilities for `#[mcp_tool]` and `#[mcp_server]` macros.
//!
//! Provides parameter classification, type detection, and schema generation
//! helpers used by both standalone and impl-block macro expansions.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, GenericParam, Generics, Type, TypePath};

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
        }
    }
}

/// Check if the last segment of a type path matches the given name.
/// Handles qualified paths like `pmcp::State<T>` by checking only the final segment.
pub fn type_name_matches(ty: &Type, name: &str) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        path.segments
            .last()
            .map(|s| s.ident == name)
            .unwrap_or(false)
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

/// Check if a type is `Value` or `serde_json::Value`.
///
/// Per D-15: skip `outputSchema` generation for `Result<Value>` returns.
pub fn is_value_type(ty: &Type) -> bool {
    type_name_matches(ty, "Value")
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
            }
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
            }
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
    fn test_is_value_type_false() {
        let ty: Type = parse_quote!(CalculatorResult);
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
