use crate::error::{Error, Result, TransportError};
use crate::shared::http_constants::{
    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, LAST_EVENT_ID, MCP_PROTOCOL_VERSION,
    MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use crate::shared::sse_parser::SseParser;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, Response as HyperResponse, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use parking_lot::RwLock;
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
            .field(
                "http_middleware_chain",
                &self.http_middleware_chain.is_some(),
            )
            .finish()
    }
}

/// Builder for `StreamableHttpTransportConfig`.
///
/// Provides a fluent API for configuring HTTP transport with middleware support.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
/// use pmcp::client::http_middleware::HttpMiddlewareChain;
/// use url::Url;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), pmcp::Error> {
/// let mut http_chain = HttpMiddlewareChain::new();
/// // Add middleware to chain...
///
/// let config = StreamableHttpTransportConfigBuilder::new(
///         Url::parse("http://localhost:8080").unwrap()
///     )
///     .with_http_middleware(Arc::new(http_chain))
///     .with_header("X-API-Key", "secret")
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct StreamableHttpTransportConfigBuilder {
    url: Url,
    extra_headers: Vec<(String, String)>,
    auth_provider: Option<Arc<dyn AuthProvider>>,
    session_id: Option<String>,
    enable_json_response: bool,
    on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
    http_middleware_chain: Option<Arc<crate::client::http_middleware::HttpMiddlewareChain>>,
}

impl Debug for StreamableHttpTransportConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransportConfigBuilder")
            .field("url", &self.url)
            .field("extra_headers", &self.extra_headers)
            .field("auth_provider", &self.auth_provider.is_some())
            .field("session_id", &self.session_id)
            .field("enable_json_response", &self.enable_json_response)
            .field("on_resumption_token", &self.on_resumption_token.is_some())
            .field(
                "http_middleware_chain",
                &self.http_middleware_chain.is_some(),
            )
            .finish()
    }
}

impl StreamableHttpTransportConfigBuilder {
    /// Create a new config builder with the specified URL.
    pub fn new(url: Url) -> Self {
        Self {
            url,
            extra_headers: Vec::new(),
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        }
    }

    /// Add an HTTP header to include in all requests.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    /// Set the authentication provider.
    pub fn with_auth_provider(mut self, provider: Arc<dyn AuthProvider>) -> Self {
        self.auth_provider = Some(provider);
        self
    }

    /// Set the session ID for stateful operation.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Enable JSON responses instead of SSE streams.
    pub fn enable_json_response(mut self) -> Self {
        self.enable_json_response = true;
        self
    }

    /// Set callback for resumption token updates.
    pub fn on_resumption_token(mut self, callback: Arc<dyn Fn(String) + Send + Sync>) -> Self {
        self.on_resumption_token = Some(callback);
        self
    }

    /// Set the HTTP middleware chain for request/response transformation.
    ///
    /// HTTP middleware operates at the transport layer, before protocol processing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
    /// use pmcp::client::http_middleware::HttpMiddlewareChain;
    /// use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
    /// use url::Url;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let mut http_chain = HttpMiddlewareChain::new();
    ///
    /// // Add OAuth middleware
    /// let token = BearerToken::with_expiry("my-token".to_string(), Duration::from_secs(3600));
    /// http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));
    ///
    /// let config = StreamableHttpTransportConfigBuilder::new(
    ///         Url::parse("http://localhost:8080").unwrap()
    ///     )
    ///     .with_http_middleware(Arc::new(http_chain))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_http_middleware(
        mut self,
        chain: Arc<crate::client::http_middleware::HttpMiddlewareChain>,
    ) -> Self {
        self.http_middleware_chain = Some(chain);
        self
    }

    /// Build the configuration.
    pub fn build(self) -> StreamableHttpTransportConfig {
        StreamableHttpTransportConfig {
            url: self.url,
            extra_headers: self.extra_headers,
            auth_provider: self.auth_provider,
            session_id: self.session_id,
            enable_json_response: self.enable_json_response,
            on_resumption_token: self.on_resumption_token,
            http_middleware_chain: self.http_middleware_chain,
        }
    }
}

/// A streamable HTTP transport for MCP.
///
/// This transport supports both stateless and stateful operation modes:
/// - Stateless: No session tracking, each request is independent (suitable for Lambda)
/// - Stateful: Optional session ID tracking for persistent sessions
///
/// The transport can handle both JSON responses and SSE streams based on server response.
///
/// HTTPS is supported via rustls with the ring crypto provider, which is compatible
/// with AWS Lambda and other serverless environments.
#[derive(Clone)]
pub struct StreamableHttpTransport {
    config: Arc<RwLock<StreamableHttpTransportConfig>>,
    client: Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        Full<Bytes>,
    >,
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
    ///
    /// This automatically sets up HTTPS support using rustls with the ring crypto provider.
    /// Both HTTP and HTTPS URLs are supported.
    pub fn new(config: StreamableHttpTransportConfig) -> Self {
        // Install ring crypto provider explicitly to avoid conflicts with aws-lc-rs
        // in Lambda environments. This is idempotent - safe to call multiple times.
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Create HTTPS connector that supports both HTTP and HTTPS
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("Failed to load native root certificates")
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();

        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build(https);

        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            config: Arc::new(RwLock::new(config)),
            client,
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

    /// Start a GET SSE stream with middleware support
    pub async fn start_sse(&self, resumption_token: Option<String>) -> Result<()> {
        // Abort any existing SSE stream
        let handle = self.abort_handle.write().take();
        if let Some(handle) = handle {
            handle.abort();
        }

        let url = self.config.read().url.clone();

        // Build GET request with middleware integration
        let mut request = self
            .build_request_with_middleware(
                Method::GET,
                url.as_str(),
                vec![], // Empty body for GET
            )
            .await?;

        // Add SSE-specific headers
        request.headers_mut().insert(
            ACCEPT,
            TEXT_EVENT_STREAM.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {}",
                    e
                )))
            })?,
        );

        // Add Last-Event-ID for resumability
        if let Some(token) = &resumption_token {
            request.headers_mut().insert(
                LAST_EVENT_ID,
                token.parse().map_err(|e| {
                    Error::Transport(TransportError::InvalidMessage(format!(
                        "Invalid header: {}",
                        e
                    )))
                })?,
            );
        }

        // Send request
        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        // Handle 405 (SSE not supported) gracefully
        if response.status() == StatusCode::METHOD_NOT_ALLOWED {
            // Server doesn't support GET SSE, which is OK
            return Ok(());
        }

        if !response.status().is_success() {
            return Err(Error::Transport(TransportError::Request(format!(
                "SSE request failed with status: {}",
                response.status()
            ))));
        }

        // Process response headers
        self.process_response_headers(&response);

        // Collect body (for now - could be streamed in future)
        let body_bytes = response
            .collect()
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?
            .to_bytes();

        // Fast path: Check if middleware exists before creating temp response
        let modified_body = if self.config.read().http_middleware_chain.is_some() {
            // Run response middleware (create a minimal response for middleware processing)
            let temp_response = HyperResponse::builder()
                .status(200)
                .body(Full::new(Bytes::new()))
                .unwrap();
            self.apply_response_middleware("GET", url.as_str(), &temp_response, body_bytes.to_vec())
                .await?
        } else {
            // No middleware - use body directly (fast path)
            body_bytes.to_vec()
        };

        // Start streaming task
        let sender = self.sender.clone();
        let on_resumption = self.config.read().on_resumption_token.clone();
        let last_event_id = self.last_event_id.clone();

        let handle = tokio::spawn(async move {
            let mut sse_parser = SseParser::new();
            let body = String::from_utf8_lossy(&modified_body);

            // Parse SSE events
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

    /// Build a `hyper::Request` with middleware integration.
    ///
    /// This method:
    /// 1. Builds initial request with config headers, auth, session, protocol version
    /// 2. Runs HTTP middleware on the request
    /// 3. Returns the modified `hyper::Request` ready to send
    async fn build_request_with_middleware(
        &self,
        method: Method,
        url: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpRequest};

        // Extract config data
        let (extra_headers, auth_provider, session_id, middleware_chain) = {
            let config = self.config.read();
            (
                config.extra_headers.clone(),
                config.auth_provider.clone(),
                config.session_id.clone(),
                config.http_middleware_chain.clone(),
            )
        };

        // Start building request with hyper
        let mut request_builder = Request::builder().method(method.clone()).uri(url);

        // Add extra headers from config
        for (key, value) in &extra_headers {
            request_builder = request_builder.header(key.as_str(), value.as_str());
        }

        // Add auth header if provider is present (highest priority)
        let has_auth = if let Some(auth_provider) = auth_provider {
            let token = auth_provider.get_access_token().await?;
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
            true
        } else {
            false
        };

        // Add session ID header if we have one
        if let Some(session_id) = &session_id {
            request_builder = request_builder.header(MCP_SESSION_ID, session_id.as_str());
        }

        // Add protocol version header if we have one
        if let Some(protocol_version) = self.protocol_version.read().as_ref() {
            request_builder =
                request_builder.header(MCP_PROTOCOL_VERSION, protocol_version.as_str());
        }

        // Build temporary request to extract headers for middleware
        let temp_req = request_builder
            .body(Full::new(Bytes::from(body.clone())))
            .map_err(|e| Error::Transport(TransportError::InvalidMessage(e.to_string())))?;

        // Extract headers from temp request
        let headers = temp_req.headers();

        // Run HTTP middleware if configured
        if let Some(chain) = middleware_chain {
            // Create HttpRequest from hyper components
            let mut http_req = HttpRequest::new(method.as_str().to_string(), url.to_string(), body);

            // Copy headers
            for (key, value) in headers {
                if let Ok(value_str) = value.to_str() {
                    http_req.add_header(key.as_str(), value_str);
                }
            }

            // Create context
            let context = HttpMiddlewareContext::new(url.to_string(), method.as_str().to_string());

            // Set metadata if auth was already set by transport
            if has_auth {
                context.set_metadata("auth_already_set".to_string(), "true".to_string());
            }

            // Run middleware chain
            if let Err(e) = chain.process_request(&mut http_req, &context).await {
                // Call error handlers
                chain.handle_transport_error(&e, &context).await;
                return Err(e);
            }

            // Rebuild request with modified headers and body
            let mut final_builder = Request::builder().method(method).uri(url);

            for (key, value) in &http_req.headers {
                final_builder = final_builder.header(key, value);
            }

            final_builder
                .body(Full::new(Bytes::from(http_req.body)))
                .map_err(|e| Error::Transport(TransportError::InvalidMessage(e.to_string())))
        } else {
            // No middleware - return original request
            Ok(temp_req)
        }
    }

    /// Apply HTTP middleware to a response after receiving.
    #[allow(clippy::future_not_send)]
    async fn apply_response_middleware(
        &self,
        method: &str,
        url: &str,
        response: &HyperResponse<impl hyper::body::Body>,
        body: Vec<u8>,
    ) -> Result<Vec<u8>> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpResponse};

        let middleware_chain = self.config.read().http_middleware_chain.clone();
        if let Some(chain) = middleware_chain {
            // Create HttpResponse from hyper components
            let header_map = response.headers().clone();

            let mut http_resp =
                HttpResponse::with_headers(response.status().as_u16(), header_map, body);

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
    fn process_response_headers(&self, response: &HyperResponse<impl hyper::body::Body>) {
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

    /// Send a message with options (hyper-based with middleware)
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

        let url = self.config.read().url.clone();

        // Build POST request with middleware integration
        let mut request = self
            .build_request_with_middleware(Method::POST, url.as_str(), body_bytes)
            .await?;

        // Add request-specific headers
        request.headers_mut().insert(
            CONTENT_TYPE,
            APPLICATION_JSON.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {}",
                    e
                )))
            })?,
        );
        request.headers_mut().insert(
            ACCEPT,
            ACCEPT_STREAMABLE.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {}",
                    e
                )))
            })?,
        );

        // Send request
        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        // Process headers for session and protocol info
        self.process_response_headers(&response);

        // Handle non-success responses
        if !response.status().is_success() {
            // Special handling for 202 Accepted (notification acknowledged)
            if response.status() == StatusCode::ACCEPTED {
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

        // Get response metadata before consuming the response
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

        // Collect response body
        let body_bytes = response
            .collect()
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?
            .to_bytes();

        // Fast path: Check if middleware exists before creating temp response
        let modified_body = if self.config.read().http_middleware_chain.is_some() {
            // Run response middleware (create a minimal response for middleware processing)
            let temp_response = HyperResponse::builder()
                .status(status_code)
                .body(Full::new(Bytes::new()))
                .unwrap();
            self.apply_response_middleware(
                "POST",
                url.as_str(),
                &temp_response,
                body_bytes.to_vec(),
            )
            .await?
        } else {
            // No middleware - use body directly (fast path)
            body_bytes.to_vec()
        };

        // If it's a 200 response with Content-Length: 0 or no Content-Type
        if status_code == StatusCode::OK && (content_length == Some(0) || content_type.is_empty()) {
            if modified_body.is_empty() {
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
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&modified_body) {
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
                let msg_parsed = crate::shared::StdioTransport::parse_message(&modified_body)?;
                self.sender
                    .send(msg_parsed)
                    .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
            }
            return Ok(());
        }

        if content_type.contains(APPLICATION_JSON) {
            // JSON response (single or batch)
            // Try to parse as array first (batch response - JSON-RPC 2.0)
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&modified_body) {
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
                let msg_parsed = crate::shared::StdioTransport::parse_message(&modified_body)?;
                self.sender
                    .send(msg_parsed)
                    .map_err(|e| Error::Transport(TransportError::Send(e.to_string())))?;
            }
        } else if content_type.contains(TEXT_EVENT_STREAM) {
            // SSE stream response - handle streaming
            let sender = self.sender.clone();
            let on_resumption = self.config.read().on_resumption_token.clone();
            let last_event_id = self.last_event_id.clone();

            tokio::spawn(async move {
                let mut sse_parser = SseParser::new();
                let body = String::from_utf8_lossy(&modified_body);

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
        } else if status_code == StatusCode::ACCEPTED {
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
            let request = self
                .build_request_with_middleware(Method::DELETE, url.as_str(), vec![])
                .await?;

            // Send DELETE request (ignore 405 as per spec)
            let response = self.client.request(request).await;
            if let Ok(resp) = response {
                if !resp.status().is_success() && resp.status() != StatusCode::METHOD_NOT_ALLOWED {
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
