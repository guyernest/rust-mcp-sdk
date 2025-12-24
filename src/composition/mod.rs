//! MCP Server Composition
//!
//! This module provides the `FoundationClient` trait and implementations for
//! domain servers to call foundation servers in a deployment-agnostic way.
//!
//! # Overview
//!
//! MCP Server Composition enables domain-tier servers to orchestrate calls to
//! foundation-tier servers. The `FoundationClient` trait provides a unified
//! interface that works across different deployment targets:
//!
//! - **Local development**: HTTP transport to local servers
//! - **pmcp.run deployment**: Lambda direct invocation via `CompositionClient`
//! - **Other platforms**: Custom implementations
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::composition::{FoundationClient, McpFoundationClient, FoundationConfig};
//! use std::sync::Arc;
//!
//! // Load configuration from foundations.toml
//! let config = FoundationConfig::from_file("foundations.toml")?;
//!
//! // Create client for foundation server
//! let client = McpFoundationClient::new(config).await?;
//!
//! // Call a tool on the foundation server
//! let result = client.call_tool(
//!     "calculator",
//!     "add",
//!     &serde_json::json!({"a": 5, "b": 3})
//! ).await?;
//! ```
//!
//! # Configuration
//!
//! Foundation server endpoints are configured in a `foundations.toml` file,
//! typically generated during schema export:
//!
//! ```toml
//! [foundations.calculator]
//! url = "http://localhost:8080"
//!
//! [foundations.database]
//! url = "http://localhost:8081"
//! ```

mod config;
mod error;
mod mcp_client;
mod types;

pub use config::{FoundationConfig, FoundationEndpoint};
pub use error::CompositionError;
pub use mcp_client::McpFoundationClient;
pub use types::{EmbeddedResource, PromptContent, PromptMessage, PromptResult, ResourceContent};

use async_trait::async_trait;
use serde::de::DeserializeOwned;

/// Trait for calling foundation servers from domain servers.
///
/// This trait provides a deployment-agnostic interface for domain servers
/// to call foundation servers. Implementations include:
///
/// - `McpFoundationClient`: Uses the MCP client over HTTP for local/generic deployments
/// - `CompositionClient` (in pmcp-composition): Uses Lambda direct invocation for pmcp.run
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::composition::FoundationClient;
///
/// async fn use_calculator(client: &impl FoundationClient) -> Result<f64, pmcp::composition::CompositionError> {
///     let result = client.call_tool(
///         "calculator",
///         "add",
///         &serde_json::json!({"a": 10, "b": 5})
///     ).await?;
///
///     // Parse the result
///     let value: serde_json::Value = serde_json::from_str(&result)?;
///     Ok(value["result"].as_f64().unwrap_or(0.0))
/// }
/// ```
#[async_trait]
pub trait FoundationClient: Send + Sync {
    /// Call a tool on a foundation server.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server (e.g., "calculator")
    /// * `tool_name` - The name of the tool to call
    /// * `arguments` - Tool arguments as a JSON value
    ///
    /// # Returns
    ///
    /// The tool result as a JSON string, or an error if the call fails.
    async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, CompositionError>;

    /// Call a tool and deserialize the result to a typed value.
    ///
    /// This is a convenience method that calls `call_tool` and deserializes
    /// the result to the specified type.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// #[derive(Deserialize)]
    /// struct ArithmeticResult {
    ///     operation: String,
    ///     result: f64,
    /// }
    ///
    /// let result: ArithmeticResult = client.call_tool_typed(
    ///     "calculator",
    ///     "add",
    ///     &json!({"a": 5, "b": 3})
    /// ).await?;
    /// ```
    async fn call_tool_typed<T: DeserializeOwned>(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<T, CompositionError> {
        let result = self.call_tool(server_id, tool_name, arguments).await?;
        serde_json::from_str(&result).map_err(|e| {
            CompositionError::Deserialization(format!(
                "Failed to deserialize tool result: {}. Raw result: {}",
                e, result
            ))
        })
    }

    /// Read a resource from a foundation server.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server
    /// * `uri` - The URI of the resource to read
    ///
    /// # Returns
    ///
    /// The resource content, or an error if the read fails.
    async fn read_resource(
        &self,
        server_id: &str,
        uri: &str,
    ) -> Result<ResourceContent, CompositionError>;

    /// Get a prompt from a foundation server.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server
    /// * `prompt_name` - The name of the prompt
    /// * `arguments` - Prompt arguments as a JSON value
    ///
    /// # Returns
    ///
    /// The prompt result containing messages, or an error if the call fails.
    async fn get_prompt(
        &self,
        server_id: &str,
        prompt_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<PromptResult, CompositionError>;

    /// Get a prompt with typed arguments.
    ///
    /// This is a convenience method that converts typed arguments to JSON
    /// before calling `get_prompt`.
    async fn get_prompt_typed(
        &self,
        server_id: &str,
        prompt_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<PromptResult, CompositionError> {
        self.get_prompt(server_id, prompt_name, arguments).await
    }

    /// Check if a foundation server is available.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server to check
    ///
    /// # Returns
    ///
    /// `true` if the server is available, `false` otherwise.
    async fn is_available(&self, server_id: &str) -> bool;

    /// Get the list of configured foundation server IDs.
    fn foundation_ids(&self) -> Vec<String>;
}

/// Extract text content from a resource result.
///
/// This is a utility function for extracting text from resource content.
///
/// # Example
///
/// ```rust,ignore
/// let content = client.read_resource("calculator", "calculator://help/guide").await?;
/// let text = pmcp::composition::extract_resource_text(&content)?;
/// println!("Guide: {}", text);
/// ```
pub fn extract_resource_text(content: &ResourceContent) -> Result<String, CompositionError> {
    content.text.clone().ok_or_else(|| {
        CompositionError::InvalidResponse("Resource does not contain text content".to_string())
    })
}
