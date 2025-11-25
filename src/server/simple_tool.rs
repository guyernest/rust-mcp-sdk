//! Simple tool implementations with schema support.

use crate::{types::ToolInfo, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use super::cancellation::RequestHandlerExtra;
use super::ToolHandler;

/// A simple tool implementation with schema support.
pub struct SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    handler: F,
}

impl<F> fmt::Debug for SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimpleTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .finish()
    }
}

impl<F> SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    /// Create a new tool with a name and handler.
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            }),
            handler,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the input schema for this tool.
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }
}

#[async_trait]
impl<F> ToolHandler for SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        (self.handler)(args, extra).await
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            self.name.clone(),
            self.description.clone(),
            self.input_schema.clone(),
        ))
    }
}

/// A simpler tool for synchronous handlers.
pub struct SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    handler: F,
}

impl<F> fmt::Debug for SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .finish()
    }
}

impl<F> SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    /// Create a new synchronous tool with a name and handler.
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            }),
            handler,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the input schema for this tool.
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }
}

#[async_trait]
impl<F> ToolHandler for SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        (self.handler)(args)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            self.name.clone(),
            self.description.clone(),
            self.input_schema.clone(),
        ))
    }
}
