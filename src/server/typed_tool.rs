//! Type-safe tool implementations with automatic schema generation.
//!
//! This module provides the native (non-WASM) typed tool implementations with full
//! input and output typing support. For WASM environments, see `wasm_typed_tool.rs`
//! which provides input typing only due to async constraints.

use crate::types::{ToolAnnotations, ToolInfo};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::cancellation::RequestHandlerExtra;
use super::ToolHandler;

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;

/// A typed tool implementation with automatic schema generation and validation.
pub struct TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> fmt::Debug for TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("annotations", &self.annotations)
            .finish()
    }
}

impl<T, F> TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    /// Create a new typed tool with automatic schema generation.
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema,
    {
        let schema = generate_schema::<T>();
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            ui_resource_uri: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create a new typed tool with a manually provided schema.
    pub fn new_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            ui_resource_uri: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// Annotations provide behavioral hints to AI clients about how this tool
    /// should be used (read-only, destructive, idempotent, etc.).
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use pmcp::types::ToolAnnotations;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct DeleteArgs {
    ///     id: String,
    /// }
    ///
    /// let tool = TypedTool::new("delete_record", |args: DeleteArgs, _extra| {
    ///     Box::pin(async move {
    ///         Ok(serde_json::json!({"deleted": true}))
    ///     })
    /// })
    /// .with_description("Permanently delete a record")
    /// .with_annotations(
    ///     ToolAnnotations::new()
    ///         .with_read_only(false)
    ///         .with_destructive(true)
    ///         .with_idempotent(true)
    /// );
    /// # }
    /// ```
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_read_only(true))`
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    ///
    /// Sets `readOnlyHint: false` and `destructiveHint: true`.
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_idempotent(true))`
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_open_world(true))`
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }

    /// Associate this tool with a UI resource (MCP Apps Extension).
    ///
    /// This sets the `ui/resourceUri` field in the tool's `_meta` field,
    /// allowing MCP hosts to display an interactive UI when this tool is invoked.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct AnalyzeArgs {
    ///     query: String,
    /// }
    ///
    /// let tool = TypedTool::new("analyze_sales", |args: AnalyzeArgs, _extra| {
    ///     Box::pin(async move {
    ///         Ok(serde_json::json!({"result": "data"}))
    ///     })
    /// })
    /// .with_description("Analyze sales data")
    /// .with_ui("ui://charts/sales");  // Associate with UI resource
    /// # }
    /// ```
    pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(ui_resource_uri.into());
        self
    }
}

#[async_trait]
impl<T, F> ToolHandler for TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Deserialize and validate the arguments
        let typed_args: T = serde_json::from_value(args).map_err(|e| {
            crate::Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args, extra).await
    }

    fn metadata(&self) -> Option<ToolInfo> {
        // Build _meta for UI resource if specified
        let meta = self.ui_resource_uri.as_ref().map(|uri| {
            let mut meta = serde_json::Map::new();
            meta.insert("ui".to_string(), serde_json::json!({ "resourceUri": uri }));
            meta
        });

        Some(ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            annotations: self.annotations.clone(),
            _meta: meta,
        })
    }
}

/// A synchronous typed tool implementation with automatic schema generation.
pub struct TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> fmt::Debug for TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedSyncTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("annotations", &self.annotations)
            .finish()
    }
}

impl<T, F> TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    /// Create a new synchronous typed tool with automatic schema generation.
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema,
    {
        let schema = generate_schema::<T>();
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create a new synchronous typed tool with a manually provided schema.
    pub fn new_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// See [`TypedTool::with_annotations`] for detailed documentation.
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }
}

#[async_trait]
impl<T, F> ToolHandler for TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Deserialize and validate the arguments
        let typed_args: T = serde_json::from_value(args).map_err(|e| {
            crate::Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args, extra)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            annotations: self.annotations.clone(),
            _meta: None,
        })
    }
}

/// Generate a JSON schema for a type using schemars.
#[cfg(feature = "schema-generation")]
fn generate_schema<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);

    // Convert the schema to JSON value
    let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    });

    // Normalize the schema by inlining $ref references
    crate::server::schema_utils::normalize_schema(json_schema)
}

/// Extension trait to add type-safe schema generation to `SimpleTool`.
pub trait SimpleToolExt {
    /// Create a `SimpleTool` with schema generated from a type.
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self;
}

use super::simple_tool::SimpleTool;

impl<F> SimpleToolExt for SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self {
        let schema = generate_schema::<T>();
        self.with_schema(schema)
    }
}

/// Extension trait to add type-safe schema generation to `SyncTool`.
pub trait SyncToolExt {
    /// Create a `SyncTool` with schema generated from a type.
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self;
}

use super::simple_tool::SyncTool;

impl<F> SyncToolExt for SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self {
        let schema = generate_schema::<T>();
        self.with_schema(schema)
    }
}

/// A typed tool with both input and output type safety
///
/// This variant provides type safety for both input arguments and return values.
/// While output schemas are not part of the MCP protocol, they're useful for
/// testing, documentation, and API contracts.
pub struct TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    output_schema: Option<Value>,
    annotations: Option<ToolAnnotations>,
    handler: F,
    _phantom: PhantomData<(TIn, TOut)>,
}

impl<TIn, TOut, F> fmt::Debug for TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedToolWithOutput")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("output_schema", &self.output_schema)
            .field("annotations", &self.annotations)
            .finish()
    }
}

impl<TIn, TOut, F> TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    /// Create a new typed tool with automatic input and output schema generation
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        TIn: JsonSchema,
        TOut: JsonSchema,
    {
        let input_schema = generate_schema::<TIn>();
        let output_schema = Some(generate_schema::<TOut>());

        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema,
            annotations: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create with only input schema generation (output schema omitted)
    #[cfg(feature = "schema-generation")]
    pub fn new_input_only(name: impl Into<String>, handler: F) -> Self
    where
        TIn: JsonSchema,
    {
        let input_schema = generate_schema::<TIn>();

        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema: None,
            annotations: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create with manually provided schemas
    pub fn new_with_schemas(
        name: impl Into<String>,
        input_schema: Value,
        output_schema: Option<Value>,
        handler: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema,
            annotations: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// These annotations will be merged with the auto-generated output schema
    /// annotation. User-provided hints (readOnlyHint, destructiveHint, etc.)
    /// will be combined with the output schema annotation.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedToolWithOutput;
    /// use pmcp::types::ToolAnnotations;
    /// use serde::{Deserialize, Serialize};
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct QueryArgs { sql: String }
    ///
    /// #[derive(Debug, Serialize, JsonSchema)]
    /// struct QueryResult { rows: Vec<String> }
    ///
    /// let tool = TypedToolWithOutput::new("query", |args: QueryArgs, _| {
    ///     Box::pin(async move {
    ///         Ok(QueryResult { rows: vec![] })
    ///     })
    /// })
    /// .with_description("Execute SQL query")
    /// .with_annotations(
    ///     ToolAnnotations::new()
    ///         .with_read_only(true)
    ///         .with_idempotent(true)
    /// );
    /// // Tool now has both readOnlyHint and auto-generated pmcp:outputSchema
    /// # }
    /// ```
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }

    /// Get the output schema (if any) for testing/documentation purposes
    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }
}

#[async_trait]
impl<TIn, TOut, F> ToolHandler for TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Parse the arguments to the input type
        let typed_args: TIn = serde_json::from_value(args)
            .map_err(|e| Error::Validation(format!("Invalid arguments: {}", e)))?;

        // Call the handler
        let result = (self.handler)(typed_args, extra).await?;

        // Convert the typed result to JSON
        serde_json::to_value(result)
            .map_err(|e| Error::Internal(format!("Failed to serialize result: {}", e)))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        // Start with user-provided annotations or empty
        let mut annotations = self.annotations.clone().unwrap_or_default();

        // Add output schema annotation if available
        if let Some(schema) = &self.output_schema {
            // Extract type name from schema's title field (set by schemars)
            let type_name = schema
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Output")
                .to_string();

            annotations = annotations.with_output_schema(schema.clone(), type_name);
        }

        // Only include annotations if we have any meaningful content
        let has_annotations = annotations.read_only_hint.is_some()
            || annotations.destructive_hint.is_some()
            || annotations.idempotent_hint.is_some()
            || annotations.open_world_hint.is_some()
            || annotations.output_schema.is_some();

        Some(ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            annotations: if has_annotations {
                Some(annotations)
            } else {
                None
            },
            _meta: None,
        })
    }
}
