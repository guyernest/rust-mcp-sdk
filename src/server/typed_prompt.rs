//! Type-safe prompt implementations with automatic argument schema generation.
//!
//! This module provides `TypedPrompt<T, F>` for prompts that accept typed argument
//! structs instead of raw `HashMap<String, String>`. The struct derives its
//! `PromptArgument` list from the `JsonSchema` implementation of `T`.

use crate::types::{GetPromptResult, PromptArgument, PromptInfo};
use crate::Result;
use async_trait::async_trait;
#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::cancellation::RequestHandlerExtra;
use super::PromptHandler;

/// A typed prompt implementation with automatic argument schema generation.
///
/// `TypedPrompt` wraps a handler function that accepts a typed struct `T` instead
/// of raw `HashMap<String, String>` arguments. It automatically derives
/// `PromptArgument` entries from `T`'s `JsonSchema` implementation.
///
/// # String-Only Arguments
///
/// MCP prompt arguments are transmitted as `HashMap<String, String>`. TypedPrompt
/// converts each value to `serde_json::Value::String` before deserializing into `T`.
/// This means struct fields **must** be `String` or `Option<String>` types. Non-string
/// field types (e.g., `i32`, `bool`, `f64`) will fail deserialization because
/// `Value::String("42")` does not coerce to `i32` via `serde_json::from_value`.
///
/// If you need numeric or boolean arguments, accept them as `String` and parse
/// manually in your handler, or use `#[serde(deserialize_with = "...")]` custom
/// deserializers.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::typed_prompt::TypedPrompt;
/// use pmcp::types::GetPromptResult;
/// use serde::Deserialize;
/// use schemars::JsonSchema;
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct ReviewArgs {
///     /// The programming language
///     language: String,
///     /// Code to review
///     code: String,
/// }
///
/// let prompt = TypedPrompt::new("code_review", |args: ReviewArgs, _extra| {
///     Box::pin(async move {
///         Ok(GetPromptResult::new(vec![], Some(format!("Review {} code", args.language))))
///     })
/// })
/// .with_description("Review code for quality issues");
/// ```
pub struct TypedPrompt<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> fmt::Debug for TypedPrompt<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedPrompt")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("arguments", &self.arguments)
            .finish()
    }
}

impl<T, F> TypedPrompt<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    /// Create a new typed prompt with automatic argument schema generation.
    ///
    /// When the `schema-generation` feature is enabled, extracts `PromptArgument`
    /// entries from `T`'s `JsonSchema` implementation. Without the feature,
    /// no argument metadata is generated.
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema,
    {
        let arguments = extract_prompt_arguments::<T>();
        Self {
            name: name.into(),
            description: None,
            arguments,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create a new typed prompt (without schema generation).
    #[cfg(not(feature = "schema-generation"))]
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            arguments: Vec::new(),
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this prompt.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl<T, F> PromptHandler for TypedPrompt<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        let typed_args: T = deserialize_prompt_args(args, &self.name)?;
        (self.handler)(typed_args, extra).await
    }

    fn metadata(&self) -> Option<PromptInfo> {
        let mut info = PromptInfo::new(&self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if !self.arguments.is_empty() {
            info = info.with_arguments(self.arguments.clone());
        }
        Some(info)
    }
}

/// Deserialize MCP prompt arguments from string-only HashMap into a typed struct.
///
/// Converts each value to `serde_json::Value::String` then uses `serde_json::from_value`
/// for deserialization. Used by `TypedPrompt`, `#[mcp_prompt]`, and `#[mcp_server]`.
pub fn deserialize_prompt_args<T: DeserializeOwned>(
    args: HashMap<String, String>,
    prompt_name: &str,
) -> Result<T> {
    let value = serde_json::Value::Object(
        args.into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect(),
    );
    serde_json::from_value(value).map_err(|e| {
        crate::Error::invalid_params(format!(
            "Invalid arguments for prompt '{}': {}",
            prompt_name, e
        ))
    })
}

/// Extract `PromptArgument` entries from a JSON Schema value.
///
/// Walks the schema's `properties` and `required` fields to build
/// argument metadata. Used by both runtime `TypedPrompt` and macro-generated code.
pub fn extract_prompt_arguments_from_schema(json_schema: &serde_json::Value) -> Vec<PromptArgument> {
    let properties = match json_schema.get("properties").and_then(|p| p.as_object()) {
        Some(props) => props,
        None => return Vec::new(),
    };

    let required_fields: Vec<String> = json_schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    properties
        .iter()
        .map(|(name, prop)| {
            let mut arg = PromptArgument::new(name);
            if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
                arg = arg.with_description(desc);
            }
            if required_fields.contains(name) {
                arg = arg.required();
            }
            arg
        })
        .collect()
}

#[cfg(feature = "schema-generation")]
fn extract_prompt_arguments<T: JsonSchema>() -> Vec<PromptArgument> {
    let schema = schemars::schema_for!(T);
    let json_schema = match serde_json::to_value(&schema) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    extract_prompt_arguments_from_schema(&json_schema)
}

#[cfg(all(test, feature = "schema-generation"))]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;
    use crate::types::Content;

    #[derive(Debug, serde::Deserialize, JsonSchema)]
    struct ReviewArgs {
        /// The programming language to review
        language: String,
        /// Optional style guide
        #[serde(default)]
        style: Option<String>,
    }

    #[tokio::test]
    async fn test_typed_prompt_metadata() {
        let prompt = TypedPrompt::new("code_review", |_args: ReviewArgs, _extra| {
            Box::pin(async move {
                Ok(GetPromptResult::new(
                    vec![],
                    Some("Review".to_string()),
                ))
            })
        })
        .with_description("Review code for quality");

        let info = prompt.metadata().unwrap();
        assert_eq!(info.name, "code_review");
        assert_eq!(info.description.as_deref(), Some("Review code for quality"));

        let args = info.arguments.unwrap();
        assert_eq!(args.len(), 2);

        // language should be required
        let lang_arg = args.iter().find(|a| a.name == "language").unwrap();
        assert!(lang_arg.required);
        assert_eq!(
            lang_arg.description.as_deref(),
            Some("The programming language to review")
        );

        // style should be optional
        let style_arg = args.iter().find(|a| a.name == "style").unwrap();
        assert!(!style_arg.required);
    }

    #[tokio::test]
    async fn test_typed_prompt_handle_success() {
        let prompt = TypedPrompt::new("code_review", |args: ReviewArgs, _extra| {
            Box::pin(async move {
                Ok(GetPromptResult::new(
                    vec![crate::types::PromptMessage::user(Content::text(format!(
                        "Review this {} code",
                        args.language
                    )))],
                    None,
                ))
            })
        });

        let mut map = HashMap::new();
        map.insert("language".to_string(), "rust".to_string());
        map.insert("style".to_string(), "clippy".to_string());

        let result = prompt
            .handle(map, RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_typed_prompt_handle_missing_required_field() {
        let prompt = TypedPrompt::new("code_review", |_args: ReviewArgs, _extra| {
            Box::pin(async move {
                Ok(GetPromptResult::new(vec![], None))
            })
        });

        // Missing required "language" field
        let map = HashMap::new();
        let result = prompt.handle(map, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_typed_prompt_debug() {
        let prompt = TypedPrompt::new("test", |_args: ReviewArgs, _extra| {
            Box::pin(async move {
                Ok(GetPromptResult::new(vec![], None))
            })
        })
        .with_description("A test prompt");

        let debug_str = format!("{:?}", prompt);
        assert!(debug_str.contains("TypedPrompt"));
        assert!(debug_str.contains("test"));
    }

    #[derive(Debug, serde::Deserialize, JsonSchema)]
    struct EmptyArgs {}

    #[tokio::test]
    async fn test_typed_prompt_no_arguments() {
        let prompt = TypedPrompt::new("simple", |_args: EmptyArgs, _extra| {
            Box::pin(async move {
                Ok(GetPromptResult::new(vec![], Some("Simple".to_string())))
            })
        });

        let info = prompt.metadata().unwrap();
        assert!(info.arguments.is_none());
    }
}
