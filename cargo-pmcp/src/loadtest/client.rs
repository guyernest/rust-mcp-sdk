//! MCP-aware HTTP client for load testing.
//!
//! Each virtual user owns one [`McpClient`] instance with its own session.
//! The client performs the full MCP initialize handshake, manages the
//! `mcp-session-id` header, and classifies errors into distinct categories.

use crate::loadtest::error::McpError;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

/// MCP-aware HTTP client for load testing.
///
/// Each virtual user owns one instance with its own session. The client
/// constructs JSON-RPC requests directly using `serde_json::json!` rather
/// than depending on the parent SDK's transport layer.
pub struct McpClient {
    http: Client,
    base_url: String,
    session_id: Option<String>,
    request_timeout: Duration,
    next_request_id: u64,
}

impl McpClient {
    /// Creates a new MCP client.
    ///
    /// Takes a `reqwest::Client` by value (allows shared client via `Clone`),
    /// a base URL for the MCP server, and a per-request timeout duration.
    /// Starts with no session and request ID counter at 1.
    pub fn new(http: Client, base_url: String, timeout: Duration) -> Self {
        Self {
            http,
            base_url,
            session_id: None,
            request_timeout: timeout,
            next_request_id: 1,
        }
    }

    /// Returns the current request ID and increments the counter.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    /// Returns the current session ID, if one has been established.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Builds the JSON-RPC request body for the `initialize` method.
    pub fn build_initialize_body(&mut self) -> Value {
        todo!("GREEN phase")
    }

    /// Builds the JSON-RPC notification body for `notifications/initialized`.
    ///
    /// This is a notification (no `id` field).
    pub fn build_initialized_notification(&self) -> Value {
        todo!("GREEN phase")
    }

    /// Builds the JSON-RPC request body for `tools/call`.
    pub fn build_tool_call_body(&mut self, _tool: &str, _arguments: &Value) -> Value {
        todo!("GREEN phase")
    }

    /// Builds the JSON-RPC request body for `resources/read`.
    pub fn build_resource_read_body(&mut self, _uri: &str) -> Value {
        todo!("GREEN phase")
    }

    /// Builds the JSON-RPC request body for `prompts/get`.
    pub fn build_prompt_get_body(
        &mut self,
        _prompt: &str,
        _arguments: &HashMap<String, String>,
    ) -> Value {
        todo!("GREEN phase")
    }

    /// Extracts the `mcp-session-id` header from response headers and stores it.
    pub fn extract_session_id(&mut self, _headers: &reqwest::header::HeaderMap) {
        todo!("GREEN phase")
    }

    /// Parses a JSON-RPC response body.
    ///
    /// Returns the `result` field on success, or an [`McpError::JsonRpc`] if
    /// the response contains an `error` object.
    pub fn parse_response(body: &[u8]) -> Result<Value, McpError> {
        let _ = body;
        todo!("GREEN phase")
    }

    /// Performs the full MCP initialize handshake.
    ///
    /// Sends the `initialize` request, extracts the session ID from response
    /// headers, then sends the `notifications/initialized` notification.
    pub async fn initialize(&mut self) -> Result<Value, McpError> {
        todo!("GREEN phase")
    }

    /// Sends a `tools/call` request to the MCP server.
    pub async fn call_tool(&mut self, _tool: &str, _arguments: &Value) -> Result<Value, McpError> {
        todo!("GREEN phase")
    }

    /// Sends a `resources/read` request to the MCP server.
    pub async fn read_resource(&mut self, _uri: &str) -> Result<Value, McpError> {
        todo!("GREEN phase")
    }

    /// Sends a `prompts/get` request to the MCP server.
    pub async fn get_prompt(
        &mut self,
        _prompt: &str,
        _arguments: &HashMap<String, String>,
    ) -> Result<Value, McpError> {
        todo!("GREEN phase")
    }

    /// Sends an HTTP POST request with the given JSON-RPC body.
    ///
    /// Attaches the session ID header if present. Applies per-request timeout.
    /// Returns response headers and body bytes.
    ///
    /// **Timing boundary:** The caller captures `Instant::now()` before calling
    /// this method. The returned bytes represent the raw response -- JSON parsing
    /// happens after timing measurement is complete.
    async fn send_request(
        &mut self,
        _body: &Value,
    ) -> Result<(reqwest::header::HeaderMap, Vec<u8>), McpError> {
        todo!("GREEN phase")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_client() -> McpClient {
        McpClient::new(
            Client::new(),
            "http://localhost:3000".to_string(),
            Duration::from_secs(5),
        )
    }

    #[test]
    fn test_new_client_has_no_session() {
        let client = make_client();
        assert!(client.session_id().is_none());
    }

    #[test]
    fn test_next_id_increments() {
        let mut client = make_client();
        assert_eq!(client.next_id(), 1);
        assert_eq!(client.next_id(), 2);
        assert_eq!(client.next_id(), 3);
    }

    #[test]
    fn test_build_initialize_request_body() {
        let mut client = make_client();
        let body = client.build_initialize_body();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "initialize");
        assert_eq!(body["params"]["protocolVersion"], "2025-06-18");
        assert_eq!(
            body["params"]["clientInfo"]["name"],
            "cargo-pmcp-loadtest"
        );
        assert_eq!(
            body["params"]["clientInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_initialized_notification_body() {
        let client = make_client();
        let body = client.build_initialized_notification();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "notifications/initialized");
        assert!(body.get("id").is_none() || body["id"].is_null());
    }

    #[test]
    fn test_build_tool_call_body() {
        let mut client = make_client();
        let args = json!({"expression": "2+2"});
        let body = client.build_tool_call_body("calculate", &args);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "tools/call");
        assert_eq!(body["params"]["name"], "calculate");
        assert_eq!(body["params"]["arguments"]["expression"], "2+2");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_resource_read_body() {
        let mut client = make_client();
        let body = client.build_resource_read_body("file:///data.json");
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "resources/read");
        assert_eq!(body["params"]["uri"], "file:///data.json");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_prompt_get_body() {
        let mut client = make_client();
        let mut arguments = HashMap::new();
        arguments.insert("text".to_string(), "hello".to_string());
        let body = client.build_prompt_get_body("summarize", &arguments);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "prompts/get");
        assert_eq!(body["params"]["name"], "summarize");
        assert_eq!(body["params"]["arguments"]["text"], "hello");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_parse_session_id_from_headers() {
        let mut client = make_client();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("mcp-session-id", "test-session-123".parse().unwrap());
        client.extract_session_id(&headers);
        assert_eq!(client.session_id(), Some("test-session-123"));
    }

    #[test]
    fn test_parse_jsonrpc_error_response() {
        let body = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let result = McpClient::parse_response(body);
        match result {
            Err(McpError::JsonRpc { code, message }) => {
                assert_eq!(code, -32601);
                assert_eq!(message, "Method not found");
            }
            other => panic!("Expected McpError::JsonRpc, got: {:?}", other),
        }
    }

    #[test]
    fn test_parse_jsonrpc_success_response() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let result = McpClient::parse_response(body).expect("should parse ok");
        assert!(result.get("capabilities").is_some());
    }

    #[test]
    fn test_classify_reqwest_timeout() {
        // Test error_category on a manually constructed McpError::Timeout
        let err = McpError::Timeout;
        assert_eq!(err.error_category(), "timeout");
    }

    #[test]
    fn test_error_category_returns_correct_strings() {
        assert_eq!(
            McpError::JsonRpc {
                code: -32600,
                message: "Bad".to_string()
            }
            .error_category(),
            "jsonrpc"
        );
        assert_eq!(
            McpError::Http {
                status: 500,
                body: "err".to_string()
            }
            .error_category(),
            "http"
        );
        assert_eq!(McpError::Timeout.error_category(), "timeout");
        assert_eq!(
            McpError::Connection {
                message: "err".to_string()
            }
            .error_category(),
            "connection"
        );
    }

    #[tokio::test]
    async fn test_timeout_fires_on_slow_server() {
        use tokio::net::TcpListener;

        // Start a local TCP listener that accepts but never responds
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a task that accepts connections but sleeps forever
        tokio::spawn(async move {
            loop {
                let (_socket, _) = listener.accept().await.unwrap();
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        let mut client = McpClient::new(
            Client::new(),
            format!("http://{}", addr),
            Duration::from_millis(200),
        );

        let start = std::time::Instant::now();
        let result = client.initialize().await;
        let elapsed = start.elapsed();

        // Must complete within 2 seconds
        assert!(
            elapsed < Duration::from_secs(2),
            "Timeout test took {:?}, expected < 2s",
            elapsed
        );

        // Must be an error (either Timeout or Connection is acceptable)
        assert!(result.is_err(), "Expected error from slow server");
        let err = result.unwrap_err();
        let cat = err.error_category();
        assert!(
            cat == "timeout" || cat == "connection",
            "Expected timeout or connection error, got: {}",
            cat
        );
    }
}
