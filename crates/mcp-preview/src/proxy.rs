//! MCP HTTP Proxy
//!
//! Handles communication with the target MCP server via HTTP.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    id: u64,
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

/// Tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Option<Value>,
}

/// Tool call result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

/// MCP HTTP Proxy
pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
}

impl McpProxy {
    /// Create a new MCP proxy
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }

    /// Get the next request ID
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request to the MCP server
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: self.next_id(),
        };

        let url = format!("{}/mcp", self.base_url);

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("MCP server returned {}: {}", status, text);
        }

        let rpc_response: JsonRpcResponse = response.json().await?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("MCP error: {}", error.message);
        }

        Ok(rpc_response.result.unwrap_or(Value::Null))
    }

    /// Initialize the MCP session
    pub async fn initialize(&self) -> Result<Value> {
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

        self.send_request("initialize", Some(params)).await
    }

    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        // First, ensure we're initialized
        let _ = self.initialize().await;

        let result = self.send_request("tools/list", None).await?;

        let tools: Vec<ToolInfo> =
            serde_json::from_value(result.get("tools").cloned().unwrap_or(Value::Array(vec![])))?;

        Ok(tools)
    }

    /// Call a tool
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        let params = json!({
            "name": name,
            "arguments": arguments
        });

        let result = self.send_request("tools/call", Some(params)).await;

        match result {
            Ok(value) => {
                // Parse the result
                let content: Vec<ContentItem> = serde_json::from_value(
                    value
                        .get("content")
                        .cloned()
                        .unwrap_or(Value::Array(vec![])),
                )
                .unwrap_or_default();

                let meta = value.get("_meta").cloned();

                Ok(ToolCallResult {
                    success: true,
                    content: Some(content),
                    error: None,
                    meta,
                })
            },
            Err(e) => Ok(ToolCallResult {
                success: false,
                content: None,
                error: Some(e.to_string()),
                meta: None,
            }),
        }
    }
}
