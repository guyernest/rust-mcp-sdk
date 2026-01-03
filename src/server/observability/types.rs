//! Core data types for MCP observability.
//!
//! This module defines the fundamental types used for distributed tracing
//! and request metadata collection across MCP servers.
//!
//! # Design Principles
//!
//! - `TraceContext` is for correlation only - contains NO user identity
//! - `RequestMetadata` is for analytics - contains NO user identity
//! - User identity (`user_id`, `tenant_id`) comes from `AuthContext` at logging time
//!
//! This separation ensures:
//! - Single source of truth for user identity (`AuthContext`)
//! - Clean separation between tracing and authorization concerns
//! - Privacy-conscious design (identity not duplicated across contexts)

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

/// Distributed tracing context for request correlation across MCP servers.
///
/// This struct propagates through server composition chains, enabling
/// end-to-end request tracing. The same `trace_id` is shared across all
/// servers in a request chain.
///
/// # Important
///
/// This struct contains NO user identity fields (`user_id`, email, `tenant_id`).
/// User identity is managed by `AuthContext`, which is the single source of
/// truth for authorization. `TraceContext` is purely for observability.
///
/// # Example
///
/// ```rust
/// use pmcp::server::observability::TraceContext;
///
/// // Create root trace at entry point
/// let root = TraceContext::new_root();
///
/// // Create child trace for downstream call
/// let child = root.child();
/// assert_eq!(root.trace_id, child.trace_id);
/// assert_eq!(child.depth, 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceContext {
    /// Unique trace ID (UUID v4, generated at entry point).
    /// Same across all servers in a request chain.
    pub trace_id: String,

    /// Span ID for this specific operation (UUID v4).
    /// Unique per operation within a trace.
    pub span_id: String,

    /// Parent span ID (links to calling server's span).
    /// None for the entry point (proxy or first server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,

    /// Depth in composition chain (0 = entry, 1 = first hop, etc.).
    /// Used for loop detection (reject if depth > `max_depth`).
    pub depth: u32,
}

impl TraceContext {
    /// Create a new root trace context (entry point).
    ///
    /// Use this when starting a new request chain (at proxy or first server).
    pub fn new_root() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            depth: 0,
        }
    }

    /// Create a child context for downstream calls.
    ///
    /// The child inherits the `trace_id` but gets a new `span_id`,
    /// with the current `span_id` becoming the `parent_span_id`.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            depth: self.depth + 1,
        }
    }

    /// Create a trace context with a specific `trace_id`.
    ///
    /// Use this when receiving a trace from an upstream service
    /// (e.g., from HTTP headers or Lambda payload).
    pub fn from_parent(trace_id: String, parent_span_id: Option<String>, depth: u32) -> Self {
        Self {
            trace_id,
            span_id: Uuid::new_v4().to_string(),
            parent_span_id,
            depth,
        }
    }

    /// Get a short version of the `trace_id` (first 8 characters).
    ///
    /// Useful for logging where full UUID is too verbose.
    pub fn short_trace_id(&self) -> &str {
        &self.trace_id[..8.min(self.trace_id.len())]
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new_root()
    }
}

/// Request metadata for observability and analytics.
///
/// This struct captures HOW the request was made (client type, session, etc.),
/// NOT WHO made it. User identity comes from `AuthContext`.
///
/// # Important
///
/// This struct contains NO user identity fields (`user_id`, email, `tenant_id`).
/// User identity is managed by `AuthContext`, which is the single source of
/// truth for authorization.
///
/// # Example
///
/// ```rust
/// use pmcp::server::observability::RequestMetadata;
///
/// let metadata = RequestMetadata::default()
///     .with_client_type("claude-desktop")
///     .with_client_version("1.2.3");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestMetadata {
    /// Client type/name (e.g., "claude-desktop", "cursor", "vscode-mcp").
    /// Source: MCP initialize clientInfo or User-Agent header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_type: Option<String>,

    /// Client version (e.g., "1.2.3").
    /// Source: MCP initialize clientInfo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,

    /// Client IP address (for geo-analytics).
    /// Source: Proxy extraction (pmcp.run) or X-Forwarded-For header.
    /// Privacy: Only captured if explicitly configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,

    /// Session ID for grouping related requests.
    /// Source: MCP session or generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl RequestMetadata {
    /// Set the client type.
    pub fn with_client_type(mut self, client_type: impl Into<String>) -> Self {
        self.client_type = Some(client_type.into());
        self
    }

    /// Set the client version.
    pub fn with_client_version(mut self, version: impl Into<String>) -> Self {
        self.client_version = Some(version.into());
        self
    }

    /// Set the client IP.
    pub fn with_client_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Detailed information about an MCP operation.
///
/// Provides granular tracking of MCP protocol operations including
/// tool calls, resource reads, and prompt invocations.
///
/// # Example
///
/// ```rust
/// use pmcp::server::observability::McpOperationDetails;
/// use serde_json::json;
///
/// let details = McpOperationDetails::from_request(
///     "tools/call",
///     Some(&json!({"name": "get_weather", "arguments": {"city": "NYC"}})),
///     true, // capture arguments hash
/// );
///
/// assert_eq!(details.tool_name, Some("get_weather".to_string()));
/// assert!(details.arguments_hash.is_some());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpOperationDetails {
    /// MCP JSON-RPC method (e.g., "tools/call", "resources/read", "prompts/get").
    pub method: String,

    /// For tools/call: The tool name being invoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// For tools/call: Hash of arguments (for correlation without exposing data).
    /// Only captured if `capture_arguments_hash` is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_hash: Option<String>,

    /// For resources/read: The resource URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<String>,

    /// For prompts/get: The prompt name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_name: Option<String>,

    /// For prompts/get: Hash of prompt arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_arguments_hash: Option<String>,
}

impl McpOperationDetails {
    /// Create operation details for a tool call.
    pub fn tool_call(tool_name: impl Into<String>) -> Self {
        Self {
            method: "tools/call".to_string(),
            tool_name: Some(tool_name.into()),
            ..Default::default()
        }
    }

    /// Create operation details for a resource read.
    pub fn resource_read(uri: impl Into<String>) -> Self {
        Self {
            method: "resources/read".to_string(),
            resource_uri: Some(uri.into()),
            ..Default::default()
        }
    }

    /// Create operation details for a prompt get.
    pub fn prompt_get(name: impl Into<String>) -> Self {
        Self {
            method: "prompts/get".to_string(),
            prompt_name: Some(name.into()),
            ..Default::default()
        }
    }

    /// Extract operation details from MCP method and params.
    ///
    /// # Arguments
    ///
    /// * `method` - The MCP JSON-RPC method name
    /// * `params` - Optional JSON params from the request
    /// * `capture_hash` - Whether to capture argument hashes
    pub fn from_request(
        method: &str,
        params: Option<&serde_json::Value>,
        capture_hash: bool,
    ) -> Self {
        let mut details = Self {
            method: method.to_string(),
            ..Default::default()
        };

        if let Some(params) = params {
            match method {
                "tools/call" => {
                    details.tool_name = params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if capture_hash {
                        details.arguments_hash = params.get("arguments").map(hash_value);
                    }
                },
                "resources/read" => {
                    details.resource_uri =
                        params.get("uri").and_then(|v| v.as_str()).map(String::from);
                },
                "prompts/get" => {
                    details.prompt_name = params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if capture_hash {
                        details.prompt_arguments_hash = params.get("arguments").map(hash_value);
                    }
                },
                _ => {},
            }
        }

        details
    }

    /// Get the primary operation name for metrics dimensions.
    ///
    /// Returns the tool name, prompt name, or resource URI depending on
    /// the operation type.
    pub fn operation_name(&self) -> Option<&str> {
        self.tool_name
            .as_deref()
            .or(self.prompt_name.as_deref())
            .or(self.resource_uri.as_deref())
    }

    /// Add an arguments hash to the details.
    pub fn with_arguments_hash(mut self, hash: impl Into<String>) -> Self {
        self.arguments_hash = Some(hash.into());
        self
    }
}

/// Hash a JSON value for correlation without exposing the actual data.
///
/// This creates a stable hash that can be used to correlate requests
/// with the same arguments without logging sensitive data.
pub fn hash_value(value: &serde_json::Value) -> String {
    let mut hasher = DefaultHasher::new();
    // Use canonical JSON representation for stable hashing
    // Note: json_str IS used for hashing below
    #[allow(clippy::collection_is_never_read)]
    let json_str = serde_json::to_string(value).unwrap_or_default();
    json_str.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_trace_context_new_root() {
        let ctx = TraceContext::new_root();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());
        assert_eq!(ctx.depth, 0);
    }

    #[test]
    fn test_trace_context_child() {
        let parent = TraceContext::new_root();
        let child = parent.child();

        assert_eq!(parent.trace_id, child.trace_id);
        assert_ne!(parent.span_id, child.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_eq!(child.depth, 1);
    }

    #[test]
    fn test_trace_context_chain() {
        let root = TraceContext::new_root();
        let child1 = root.child();
        let child2 = child1.child();

        assert_eq!(root.trace_id, child2.trace_id);
        assert_eq!(child2.depth, 2);
        assert_eq!(child2.parent_span_id, Some(child1.span_id));
    }

    #[test]
    fn test_request_metadata_builder() {
        let metadata = RequestMetadata::default()
            .with_client_type("claude-desktop")
            .with_client_version("1.2.3")
            .with_session_id("session-123");

        assert_eq!(metadata.client_type, Some("claude-desktop".to_string()));
        assert_eq!(metadata.client_version, Some("1.2.3".to_string()));
        assert_eq!(metadata.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_operation_details_tool_call() {
        let params = json!({
            "name": "get_weather",
            "arguments": {"city": "NYC"}
        });

        let details = McpOperationDetails::from_request("tools/call", Some(&params), true);

        assert_eq!(details.method, "tools/call");
        assert_eq!(details.tool_name, Some("get_weather".to_string()));
        assert!(details.arguments_hash.is_some());
        assert_eq!(details.operation_name(), Some("get_weather"));
    }

    #[test]
    fn test_operation_details_resource_read() {
        let params = json!({
            "uri": "file:///path/to/resource"
        });

        let details = McpOperationDetails::from_request("resources/read", Some(&params), false);

        assert_eq!(details.method, "resources/read");
        assert_eq!(
            details.resource_uri,
            Some("file:///path/to/resource".to_string())
        );
        assert_eq!(details.operation_name(), Some("file:///path/to/resource"));
    }

    #[test]
    fn test_hash_value_deterministic() {
        let value = json!({"city": "NYC", "country": "USA"});

        let hash1 = hash_value(&value);
        let hash2 = hash_value(&value);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16); // 64-bit hash as hex
    }

    #[test]
    fn test_hash_value_different_values() {
        let value1 = json!({"city": "NYC"});
        let value2 = json!({"city": "LA"});

        assert_ne!(hash_value(&value1), hash_value(&value2));
    }

    #[test]
    fn test_trace_context_serialization() {
        let ctx = TraceContext::new_root();
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: TraceContext = serde_json::from_str(&json).unwrap();

        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn test_short_trace_id() {
        let ctx = TraceContext::new_root();
        let short = ctx.short_trace_id();
        assert_eq!(short.len(), 8);
        assert!(ctx.trace_id.starts_with(short));
    }
}
