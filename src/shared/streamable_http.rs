use crate::error::{Error, Result, TransportError};
use crate::shared::http_constants::{
    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, LAST_EVENT_ID, MCP_PROTOCOL_VERSION,
    MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use crate::shared::sse_parser::SseParser;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::{Client, RequestBuilder, Response};
use std::fmt::Debug;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use url::Url;

/// Options for sending messages over streamable HTTP transport.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::streamable_http::SendOptions;
///
/// // Default options for a simple message
/// let opts = SendOptions::default();
/// assert!(opts.related_request_id.is_none());
/// assert!(opts.resumption_token.is_none());
///
/// // Options with request correlation
/// let opts = SendOptions {
///     related_request_id: Some("req-123".to_string()),
///     resumption_token: None,
/// };
///
/// // Options for resuming after disconnection
/// let opts = SendOptions {
///     related_request_id: None,
///     resumption_token: Some("event-456".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Related request ID for associating responses
    pub related_request_id: Option<String>,
    /// Resumption token for continuing interrupted streams
    pub resumption_token: Option<String>,
}

/// Configuration for the `StreamableHttpTransport`.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::streamable_http::StreamableHttpTransportConfig;
/// use url::Url;
///
/// // Minimal configuration for stateless operation
/// let config = StreamableHttpTransportConfig {
///     url: Url::parse("http://localhost:8080").unwrap(),
///     extra_headers: vec![],
///     auth_provider: None,
///     session_id: None,
///     enable_json_response: false,
///     on_resumption_token: None,
///     http_middleware_chain: None,
/// };
///
/// // Configuration with session for stateful operation
/// let config = StreamableHttpTransportConfig {
///     url: Url::parse("http://localhost:8080").unwrap(),
///     extra_headers: vec![
///         ("X-API-Key".to_string(), "secret".to_string()),
///     ],
///     auth_provider: None,
///     session_id: Some("session-123".to_string()),
///     enable_json_response: false,
///     on_resumption_token: None,
///     http_middleware_chain: None,
/// };
///
/// // Configuration for simple request/response (no streaming)
/// let config = StreamableHttpTransportConfig {
///     url: Url::parse("http://localhost:8080").unwrap(),
///     extra_headers: vec![],
///     auth_provider: None,
///     session_id: None,
///     enable_json_response: true,  // JSON instead of SSE
///     on_resumption_token: None,
///     http_middleware_chain: None,
/// };
/// ```
#[derive(Clone)]
pub struct StreamableHttpTransportConfig {
    /// The HTTP endpoint URL
    pub url: Url,
    /// Additional headers to include in requests
    pub extra_headers: Vec<(String, String)>,
    /// Optional authentication provider
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    /// Optional session ID (for stateful operation)
    pub session_id: Option<String>,
    /// Enable JSON responses instead of SSE (for simple request/response)
    pub enable_json_response: bool,
    /// Callback when resumption token is received
    pub on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// HTTP middleware chain for request/response transformation
    pub http_middleware_chain: Option<Arc<crate::client::http_middleware::HttpMiddlewareChain>>,
}

impl Debug for StreamableHttpTransportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransportConfig")
            .field("url", &self.url)
            .field("extra_headers", &self.extra_headers)
            .field("auth_provider", &self.auth_provider.is_some())
            .field("session_id", &self.session_id)
            .field("enable_json_response", &self.enable_json_response)
            .field("on_resumption_token", &self.on_resumption_token.is_some())
            .field("http_middleware_chain", &self.http_middleware_chain.is_some())
            .finish()
    }
}

/// A streamable HTTP transport for MCP.
///
/// This transport supports both stateless and stateful operation modes:
/// - Stateless: No session tracking, each request is independent (suitable for Lambda)
/// - Stateful: Optional session ID tracking for persistent sessions
///
/// The transport can handle both JSON responses and SSE streams based on server response.
#[derive(Clone)]
pub struct StreamableHttpTransport {
    config: Arc<RwLock<StreamableHttpTransportConfig>>,
    client: Client,
    /// Channel for receiving messages from SSE streams or responses
    receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<TransportMessage>>>,
    /// Sender for messages
    sender: mpsc::UnboundedSender<TransportMessage>,
    /// Protocol version negotiated with server
    protocol_version: Arc<RwLock<Option<String>>>,
    /// Abort controller for SSE streams
    abort_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Last event ID for resumability
    last_event_id: Arc<RwLock<Option<String>>>,
}

impl Debug for StreamableHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransport")
            .field("config", &self.config)
            .field("protocol_version", &self.protocol_version)
            .field("last_event_id", &self.last_event_id)
            .finish()
    }
}

impl StreamableHttpTransport {
    /// Creates a new `StreamableHttpTransport`.
    pub fn new(config: StreamableHttpTransportConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            config: Arc::new(RwLock::new(config)),
            client: Client::new(),
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            sender,
            protocol_version: Arc::new(RwLock::new(None)),
            abort_handle: Arc::new(RwLock::new(None)),
            last_event_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the current session ID
    pub fn session_id(&self) -> Option<String> {
        self.config.read().session_id.clone()
    }

    /// Set the session ID (useful for resuming sessions)
    pub fn set_session_id(&self, session_id: Option<String>) {
        self.config.write().session_id = session_id;
    }

    /// Get the protocol version
    pub fn protocol_version(&self) -> Option<String> {
        self.protocol_version.read().clone()
    }

    /// Set the protocol version (called after initialization)
    pub fn set_protocol_version(&self, version: Option<String>) {
        *self.protocol_version.write() = version;
    }

    /// Get the last event ID (for resumability)
    pub fn last_event_id(&self) -> Option<String> {
        self.last_event_id.read().clone()
    }

    /// Start a GET SSE stream
    pub async fn start_sse(&self, resumption_token: Option<String>) -> Result<()> {
        // Abort any existing SSE stream
        let handle = self.abort_handle.write().take();
        if let Some(handle) = handle {
            handle.abort();
        }

        let url = self.config.read().url.clone();
        let mut builder = self.build_request(reqwest::Method::GET, url).await?;

        builder = builder.header(ACCEPT, TEXT_EVENT_STREAM);

        // Add Last-Event-ID for resumability
        if let Some(token) = resumption_token {
            builder = builder.header(LAST_EVENT_ID, token);
        }

        let response = builder.send().await;

        // Handle 405 (SSE not supported) gracefully
        if let Ok(resp) = &response {
            if resp.status().as_u16() == 405 {
                // Server doesn't support GET SSE, which is OK
                return Ok(());
            }

            if !resp.status().is_success() {
                return Err(Error::Transport(TransportError::Request(format!(
                    "SSE request failed with status: {}",
                    resp.status()
                ))));
            }
        } else if let Err(e) = response {
            return Err(Error::Transport(TransportError::Request(e.to_string())));
        }

        let response = response.unwrap();
        self.process_response_headers(&response);

        // Start streaming task
        let sender = self.sender.clone();
        let on_resumption = self.config.read().on_resumption_token.clone();
        let last_event_id = self.last_event_id.clone();

        let handle = tokio::spawn(async move {
            let mut sse_parser = SseParser::new();
            let body = response.text().await.unwrap_or_default();

            // For now, parse the whole body - later we can stream with hyper Body
            let events = sse_parser.feed(&body);
            for event in events {
                // Update last event ID and notify callback
                if let Some(id) = &event.id {
                    *last_event_id.write() = Some(id.clone());
                    if let Some(callback) = &on_resumption {
                        callback(id.clone());
                    }
                }

                // Only process "message" events or no event type
                if event.event.as_deref() == Some("message") || event.event.is_none() {
                    // Use JSON-RPC compatibility layer
                    if let Ok(msg) =
                        crate::shared::StdioTransport::parse_message(event.data.as_bytes())
                    {
                        let _ = sender.send(msg);
                    }
                }
            }
        });

        *self.abort_handle.write() = Some(handle);
        Ok(())
    }

    async fn build_request(&self, method: reqwest::Method, url: Url) -> Result<RequestBuilder> {
        let mut builder = self.client.request(method, url);

        // Extract config data we need
        let (extra_headers, auth_provider, session_id) = {
            let config = self.config.read();
            (
                config.extra_headers.clone(),
                config.auth_provider.clone(),
                config.session_id.clone(),
            )
        };

        // Add extra headers from config
        for (key, value) in &extra_headers {
            builder = builder.header(key, value);
        }

        // Add auth header if provider is present
        if let Some(auth_provider) = auth_provider {
            let token = auth_provider.get_access_token().await?;
            builder = builder.bearer_auth(token);
        }

        // Add session ID header if we have one
        if let Some(session_id) = session_id {
            builder = builder.header(MCP_SESSION_ID, session_id);
        }

        // Add protocol version header if we have one
        if let Some(protocol_version) = self.protocol_version.read().as_ref() {
            builder = builder.header(MCP_PROTOCOL_VERSION, protocol_version);
        }

        Ok(builder)
    }

    /// Apply HTTP middleware to a request before sending.
    ///
    /// This extracts the request details, runs middleware, and reconstructs the request.
    async fn apply_request_middleware(
        &self,
        method: &str,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<(std::collections::HashMap<String, String>, Vec<u8>)> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpRequest};

        let middleware_chain = self.config.read().http_middleware_chain.clone();
        if let Some(chain) = middleware_chain {
            // Create HttpRequest from reqwest components
            let mut http_req = HttpRequest::new(method.to_string(), url.to_string(), body);

            // Copy headers
            for (key, value) in headers.iter() {
                if let Ok(value_str) = value.to_str() {
                    http_req.add_header(key.to_string(), value_str.to_string());
                }
            }

            // Create context
            let context = HttpMiddlewareContext::new(url.to_string(), method.to_string());

            // Check if auth was set by transport
            if headers.contains_key("Authorization") {
                context.set_metadata("auth_already_set".to_string(), "true".to_string());
            }

            // Run middleware chain
            if let Err(e) = chain.process_request(&mut http_req, &context).await {
                // Call error handlers
                chain.handle_transport_error(&e, &context).await;
                return Err(e);
            }

            // Return modified headers and body
            Ok((http_req.headers, http_req.body))
        } else {
            // No middleware - return original
            let header_map: std::collections::HashMap<String, String> = headers
                .iter()
                .filter_map(|(k, v)| {
                    v.to_str().ok().map(|v_str| (k.to_string(), v_str.to_string()))
                })
                .collect();
            Ok((header_map, body))
        }
    }

    /// Apply HTTP middleware to a response after receiving.
    async fn apply_response_middleware(
        &self,
        method: &str,
        url: &str,
        status: u16,
        headers: &reqwest::header::HeaderMap,
        body: Vec<u8>,
    ) -> Result<Vec<u8>> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpResponse};

        let middleware_chain = self.config.read().http_middleware_chain.clone();
        if let Some(chain) = middleware_chain {
            // Create HttpResponse from reqwest components
            let mut header_map = std::collections::HashMap::new();
            for (key, value) in headers.iter() {
                if let Ok(value_str) = value.to_str() {
                    header_map.insert(key.to_string(), value_str.to_string());
                }
            }

            let mut http_resp = HttpResponse::with_headers(status, header_map, body);

            // Create context
            let context = HttpMiddlewareContext::new(url.to_string(), method.to_string());

            // Run middleware chain
            if let Err(e) = chain.process_response(&mut http_resp, &context).await {
                // Call error handlers
                chain.handle_transport_error(&e, &context).await;
                return Err(e);
            }

            // Return modified body
            Ok(http_resp.body)
        } else {
            // No middleware - return original body
            Ok(body)
        }
    }

    /// Process response headers and extract session/protocol information
    fn process_response_headers(&self, response: &Response) {
        // Update session ID from response header
        if let Some(session_id) = response.headers().get(MCP_SESSION_ID) {
            if let Ok(session_id_str) = session_id.to_str() {
                self.config.write().session_id = Some(session_id_str.to_string());
            }
        }

        // Update protocol version from response header
        if let Some(protocol_version) = response.headers().get(MCP_PROTOCOL_VERSION) {
            if let Ok(protocol_version_str) = protocol_version.to_str() {
                *self.protocol_version.write() = Some(protocol_version_str.to_string());
            }
        }
    }

    /// Send a message with options
    pub async fn send_with_options(
        &mut self,
        message: TransportMessage,
        options: SendOptions,
    ) -> Result<()> {
        // If we have a resumption token, restart the SSE stream
        if let Some(token) = options.resumption_token {
            self.start_sse(Some(token)).await?;
            return Ok(());
        }

        // Use JSON-RPC compatibility layer for serialization
        let body_bytes = crate::shared::StdioTransport::serialize_message(&message)?;
        let body = String::from_utf8(body_bytes)
            .map_err(|e| Error::Transport(TransportError::Serialization(e.to_string())))?;

        let url = self.config.read().url.clone();
        let builder = self.build_request(reqwest::Method::POST, url).await?;

        let response = builder
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(ACCEPT, ACCEPT_STREAMABLE)
            .body(body)
            .send()
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        // Process headers for session and protocol info
        self.process_response_headers(&response);

        if !response.status().is_success() {
            // Special handling for 202 Accepted (notification acknowledged)
            if response.status().as_u16() == 202 {
                // For initialization messages, try to start SSE stream
                if matches!(message, TransportMessage::Notification { .. }) {
                    // Try to start GET SSE (tolerate 405)
                    let _ = self.start_sse(None).await;
                }
                return Ok(());
            }

            return Err(Error::Transport(TransportError::Request(format!(
                "Request failed with status: {}",
                response.status()
            ))));
        }

        // Get response metadata before potentially consuming the response
        let status_code = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        // If it's a 200 response with either Content-Length: 0 or no Content-Type
        // (often happens with notifications), check if it's actually empty
        if status_code == 200 && (content_length == Some(0) || content_type.is_empty()) {
            // Check if there's actually no body by consuming it
            let body = response
                .bytes()
                .await
                .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

            if body.is_empty() {
                // Empty 200 response (e.g., for notifications) - just return Ok
                return Ok(());
            }

            // If there was a body but no content-type, that's an error
            if content_type.is_empty() {
                return Err(Error::Transport(TransportError::Request(
                    "Response has body but no Content-Type header".to_string(),
                )));
            }

            // We have a body with content, parse it as JSON
            // Try to parse as array first (batch response - JSON-RPC 2.0)
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&body) {
                for json_msg in batch {
                    let json_str = serde_json::to_string(&json_msg).map_err(|e| {
                        Error::Transport(TransportError::Deserialization(e.to_string()))
                    })?;
                    // Use JSON-RPC compatibility layer
                    let msg = crate::shared::StdioTransport::parse_message(json_str.as_bytes())?;
                    self.sender
                        .send(msg)
                        .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
                }
            } else {
                // Single message - use JSON-RPC compatibility layer
                let message = crate::shared::StdioTransport::parse_message(&body)?;
                self.sender
                    .send(message)
                    .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
            }
            return Ok(());
        }

        if content_type.contains(APPLICATION_JSON) {
            // JSON response (single or batch)
            let response_bytes = response
                .bytes()
                .await
                .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

            // Try to parse as array first (batch response - JSON-RPC 2.0)
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&response_bytes) {
                for json_msg in batch {
                    let json_str = serde_json::to_string(&json_msg).map_err(|e| {
                        Error::Transport(TransportError::Deserialization(e.to_string()))
                    })?;
                    // Use JSON-RPC compatibility layer
                    let msg = crate::shared::StdioTransport::parse_message(json_str.as_bytes())?;
                    self.sender
                        .send(msg)
                        .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
                }
            } else {
                // Single message - use JSON-RPC compatibility layer
                let message = crate::shared::StdioTransport::parse_message(&response_bytes)?;
                self.sender
                    .send(message)
                    .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
            }
        } else if content_type.contains(TEXT_EVENT_STREAM) {
            // SSE stream response - handle streaming
            let sender = self.sender.clone();
            let on_resumption = self.config.read().on_resumption_token.clone();
            let last_event_id = self.last_event_id.clone();

            tokio::spawn(async move {
                let mut sse_parser = SseParser::new();
                let body = response.text().await.unwrap_or_default();

                // Parse the SSE body
                let events = sse_parser.feed(&body);
                for event in events {
                    // Update last event ID and notify callback
                    if let Some(id) = &event.id {
                        *last_event_id.write() = Some(id.clone());
                        if let Some(callback) = &on_resumption {
                            callback(id.clone());
                        }
                    }

                    // Only process "message" events
                    if event.event.as_deref() == Some("message") || event.event.is_none() {
                        // Use JSON-RPC compatibility layer
                        if let Ok(msg) =
                            crate::shared::StdioTransport::parse_message(event.data.as_bytes())
                        {
                            let _ = sender.send(msg);
                        }
                    }
                }
            });
        } else if status_code.as_u16() == 202 {
            // 202 Accepted with no body is valid
            return Ok(());
        } else {
            return Err(Error::Transport(TransportError::Request(format!(
                "Unsupported content type: {}",
                content_type
            ))));
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.send_with_options(message, SendOptions::default())
            .await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        // Receive from channel - this will block until a message is available
        let mut receiver = self.receiver.lock().await;
        receiver
            .recv()
            .await
            .ok_or_else(|| Error::Transport(TransportError::ConnectionClosed))
    }

    async fn close(&mut self) -> Result<()> {
        // Abort any running SSE stream
        let handle = self.abort_handle.write().take();
        if let Some(handle) = handle {
            handle.abort();
        }

        // Optionally send a DELETE request to terminate the session
        if let Some(_session_id) = self.session_id() {
            let url = self.config.read().url.clone();
            let builder = self.build_request(reqwest::Method::DELETE, url).await?;

            // Send DELETE request (ignore 405 as per spec)
            let response = builder.send().await;
            if let Ok(resp) = response {
                if !resp.status().is_success() && resp.status().as_u16() != 405 {
                    // Log error but don't fail close operation
                    tracing::warn!("Failed to terminate session: {}", resp.status());
                }
            }

            // Clear session ID
            self.config.write().session_id = None;
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        // In streamable HTTP, we're always "connected" in the sense that
        // we can make requests. There's no persistent connection.
        true
    }
}

/// A trait for providing authentication tokens.
#[async_trait]
pub trait AuthProvider: Send + Sync + Debug {
    /// Returns an access token.
    async fn get_access_token(&self) -> Result<String>;
}
