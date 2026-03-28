//! MCP HTTP Proxy
//!
//! Handles communication with the target MCP server via HTTP.
//! Uses session-once initialization with double-checked locking
//! to avoid re-initializing the MCP session on every request.

use anyhow::Result;
use parking_lot::RwLock as SyncRwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    id: u64,
}

/// JSON-RPC notification (no `id` field)
#[derive(Debug, Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: String,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: Option<u64>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: i32,
    message: String,
    #[allow(dead_code)]
    #[serde(default)]
    data: Option<Value>,
}

/// Persistent session information from MCP initialize handshake
struct SessionInfo {
    /// Session ID from `Mcp-Session-Id` response header (if server provides one)
    session_id: Option<String>,
    /// Server capabilities and info from initialize response
    #[allow(dead_code)]
    server_info: Value,
}

/// Tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<Value>,
}

/// Tool call result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Content item in tool response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

/// Resource information from `resources/list`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<Value>,
}

/// Content item within a resource read response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContentItem {
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<Value>,
}

/// Result of reading a resource via `resources/read`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadResult {
    pub contents: Vec<ResourceContentItem>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<Value>,
}

/// MCP HTTP Proxy with session-once initialization
///
/// The proxy initializes the MCP session exactly once on the first
/// request and reuses it for all subsequent calls. The session can
/// be reset via `reset_session()` for reconnect scenarios.
/// Error type for MCP requests that preserves upstream HTTP status for auth failures.
#[derive(Debug)]
pub enum McpRequestError {
    /// Upstream returned 401 or 403 -- caller should propagate the status code.
    AuthRequired(u16, String),
    /// Any other error (network, non-auth HTTP error, JSON-RPC error, etc.).
    Other(anyhow::Error),
}

impl std::fmt::Display for McpRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthRequired(status, body) => write!(f, "Auth required (HTTP {}): {}", status, body),
            Self::Other(e) => write!(f, "{}", e),
        }
    }
}

pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session: RwLock<Option<SessionInfo>>,
    auth_header: SyncRwLock<Option<String>>,
}

impl McpProxy {
    /// Create a new MCP proxy targeting the given base URL.
    pub fn new(base_url: &str) -> Self {
        Self::new_with_auth(base_url, None)
    }

    /// Create a new MCP proxy with an optional authorization header.
    ///
    /// The auth header (e.g., `"Bearer <token>"`) is included on every
    /// outbound request to the upstream MCP server.
    pub fn new_with_auth(base_url: &str, auth_header: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
            session: RwLock::new(None),
            auth_header: SyncRwLock::new(auth_header),
        }
    }

    /// Update the authorization header at runtime (e.g., after browser OAuth flow completes).
    pub fn set_auth_header(&self, header: Option<String>) {
        *self.auth_header.write() = header;
    }

    /// Check whether an authorization header is currently configured.
    pub fn has_auth_header(&self) -> bool {
        self.auth_header.read().is_some()
    }

    /// Build a base request builder with common headers and optional auth.
    fn base_post(&self) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .post(&self.base_url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");
        if let Some(ref auth) = *self.auth_header.read() {
            builder = builder.header("Authorization", auth.clone());
        }
        builder
    }

    /// Get the next request ID
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Ensure the MCP session is initialized (double-checked locking).
    ///
    /// Fast path: read lock checks if session exists.
    /// Slow path: write lock re-checks, then performs initialize handshake.
    async fn ensure_initialized(&self) -> Result<()> {
        // Fast path: session already initialized
        {
            let guard = self.session.read().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        // Slow path: acquire write lock and re-check
        let mut guard = self.session.write().await;
        if guard.is_some() {
            return Ok(());
        }

        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": false },
                "sampling": {}
            },
            "clientInfo": {
                "name": "mcp-preview",
                "version": "0.1.0"
            }
        });

        // Send initialize request, capturing response headers for session ID
        let request_body = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "initialize".to_string(),
            params: Some(params),
            id: self.next_id(),
        };

        let response = self
            .base_post()
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("MCP server returned {}: {}", status, text);
        }

        // Capture Mcp-Session-Id header if present
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let rpc_response: JsonRpcResponse = response.json().await?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("MCP initialize error: {}", error.message);
        }

        let server_info = rpc_response.result.unwrap_or(Value::Null);

        // Store session before sending notification
        *guard = Some(SessionInfo {
            session_id,
            server_info,
        });

        // Drop the write lock before sending notification to avoid holding it
        // during the network call
        drop(guard);

        // Send notifications/initialized (fire-and-forget per MCP protocol)
        let _ = self.send_notification("notifications/initialized").await;

        Ok(())
    }

    /// Send a JSON-RPC request to the MCP server.
    ///
    /// If a session ID is available, it is forwarded via the
    /// `Mcp-Session-Id` request header.
    ///
    /// Returns `McpRequestError::AuthRequired` for 401/403 responses so callers
    /// can propagate the upstream status to the browser (instead of wrapping as 502).
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, McpRequestError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: self.next_id(),
        };

        let mut req_builder = self.base_post().json(&request);

        // Forward session ID header if we have one
        {
            let guard = self.session.read().await;
            if let Some(ref session) = *guard {
                if let Some(ref sid) = session.session_id {
                    req_builder = req_builder.header("Mcp-Session-Id", sid);
                }
            }
        }

        let response = req_builder.send().await.map_err(|e| McpRequestError::Other(e.into()))?;

        let status = response.status();

        // Propagate 401/403 directly so the browser can detect auth failures
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            let text = response.text().await.unwrap_or_default();
            return Err(McpRequestError::AuthRequired(status.as_u16(), text));
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(McpRequestError::Other(anyhow::anyhow!("MCP server returned {}: {}", status, text)));
        }

        let rpc_response: JsonRpcResponse = response.json().await.map_err(|e| McpRequestError::Other(e.into()))?;

        if let Some(error) = rpc_response.error {
            return Err(McpRequestError::Other(anyhow::anyhow!("MCP error: {}", error.message)));
        }

        Ok(rpc_response.result.unwrap_or(Value::Null))
    }

    /// Send a JSON-RPC notification (no `id` field, fire-and-forget).
    ///
    /// Notifications do not expect a response from the server.
    async fn send_notification(&self, method: &str) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
        };

        let mut req_builder = self.base_post().json(&notification);

        // Forward session ID header if we have one
        {
            let guard = self.session.read().await;
            if let Some(ref session) = *guard {
                if let Some(ref sid) = session.session_id {
                    req_builder = req_builder.header("Mcp-Session-Id", sid);
                }
            }
        }

        let _ = req_builder.send().await;
        Ok(())
    }

    /// Reset the session, allowing re-initialization on the next call.
    ///
    /// Used by the reconnect endpoint to force a fresh handshake
    /// without restarting the preview server.
    pub async fn reset_session(&self) {
        let mut guard = self.session.write().await;
        *guard = None;
    }

    /// Check whether the MCP session is currently initialized.
    pub async fn is_connected(&self) -> bool {
        let guard = self.session.read().await;
        guard.is_some()
    }

    /// List available tools from the MCP server.
    ///
    /// Ensures the session is initialized before sending the request.
    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>, McpRequestError> {
        self.ensure_initialized().await.map_err(McpRequestError::Other)?;

        let result = self.send_request("tools/list", None).await?;

        let tools: Vec<ToolInfo> =
            serde_json::from_value(result.get("tools").cloned().unwrap_or(Value::Array(vec![])))
                .map_err(|e| McpRequestError::Other(e.into()))?;

        Ok(tools)
    }

    /// Call a tool on the MCP server.
    ///
    /// Ensures the session is initialized before sending the request.
    /// Returns `McpRequestError::AuthRequired` for upstream 401/403.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult, McpRequestError> {
        self.ensure_initialized().await.map_err(McpRequestError::Other)?;

        let params = json!({
            "name": name,
            "arguments": arguments
        });

        let result = self.send_request("tools/call", Some(params)).await;

        match result {
            Ok(value) => {
                let content: Vec<ContentItem> = serde_json::from_value(
                    value
                        .get("content")
                        .cloned()
                        .unwrap_or(Value::Array(vec![])),
                )
                .unwrap_or_default();

                let meta = value.get("_meta").cloned();
                let structured_content = value.get("structuredContent").cloned();

                Ok(ToolCallResult {
                    success: true,
                    content: Some(content),
                    error: None,
                    structured_content,
                    meta,
                })
            },
            Err(McpRequestError::AuthRequired(status, body)) => {
                Err(McpRequestError::AuthRequired(status, body))
            },
            Err(McpRequestError::Other(e)) => Ok(ToolCallResult {
                success: false,
                content: None,
                error: Some(e.to_string()),
                structured_content: None,
                meta: None,
            }),
        }
    }

    /// List all resources exposed by the MCP server.
    ///
    /// Ensures the session is initialized before sending the request.
    /// Returns the full unfiltered list; callers (e.g., API handlers)
    /// are responsible for filtering to UI-only resources.
    pub async fn list_resources(&self) -> Result<Vec<ResourceInfo>, McpRequestError> {
        self.ensure_initialized().await.map_err(McpRequestError::Other)?;

        let result = self.send_request("resources/list", None).await?;

        let resources: Vec<ResourceInfo> = serde_json::from_value(
            result
                .get("resources")
                .cloned()
                .unwrap_or(Value::Array(vec![])),
        )
        .map_err(|e| McpRequestError::Other(e.into()))?;

        Ok(resources)
    }

    /// Read the content of a resource by URI.
    ///
    /// Ensures the session is initialized before sending the request.
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, McpRequestError> {
        self.ensure_initialized().await.map_err(McpRequestError::Other)?;

        let params = json!({ "uri": uri });
        let result = self.send_request("resources/read", Some(params)).await?;

        let contents: Vec<ResourceContentItem> = serde_json::from_value(
            result
                .get("contents")
                .cloned()
                .unwrap_or(Value::Array(vec![])),
        )
        .map_err(|e| McpRequestError::Other(e.into()))?;

        let meta = result.get("_meta").cloned();

        Ok(ResourceReadResult { contents, meta })
    }
}
