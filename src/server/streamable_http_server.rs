//! Streamable HTTP server implementation for MCP.
use crate::error::Result;
use crate::server::http_middleware::{
    adapters::{from_axum_with_limit, into_axum},
    ServerHttpContext, ServerHttpMiddlewareChain, ServerHttpResponse,
};
use crate::server::tower_layers::{AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer};
use crate::server::Server;
use crate::shared::http_constants::{
    APPLICATION_JSON, LAST_EVENT_ID, MCP_PROTOCOL_VERSION, MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use crate::shared::TransportMessage;
use crate::types::{ClientRequest, Request};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::StreamExt;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

/// Event store trait for resumability support
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event for later retrieval
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()>;

    /// Replay events after a given event ID
    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>>;

    /// Get stream ID for an event ID
    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>>;
}

/// Type alias for event list
type EventList = Vec<(String, TransportMessage)>;

/// Type alias for events map
type EventsMap = HashMap<String, EventList>;

/// In-memory event store implementation
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    /// Events by stream ID
    events: Arc<RwLock<EventsMap>>,
    /// Event ID to stream ID mapping
    event_to_stream: Arc<RwLock<HashMap<String, String>>>,
    /// Ordered list of all event IDs
    event_order: Arc<RwLock<Vec<String>>>,
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()> {
        let mut events = self.events.write();
        let stream_events = events.entry(stream_id.to_string()).or_default();
        stream_events.push((event_id.to_string(), message.clone()));

        self.event_to_stream
            .write()
            .insert(event_id.to_string(), stream_id.to_string());
        self.event_order.write().push(event_id.to_string());

        Ok(())
    }

    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>> {
        let event_order = self.event_order.read();
        let mut result = Vec::new();

        // Find the position of the last event
        let start_pos = event_order
            .iter()
            .position(|id| id == last_event_id)
            .map_or(0, |pos| pos + 1);

        // Collect all events after that position
        let events = self.events.read();
        let event_to_stream = self.event_to_stream.read();

        for i in start_pos..event_order.len() {
            let event_id = &event_order[i];
            if let Some(stream_id) = event_to_stream.get(event_id) {
                if let Some(stream_events) = events.get(stream_id) {
                    for (eid, msg) in stream_events {
                        if eid == event_id {
                            result.push((eid.clone(), msg.clone()));
                            break;
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>> {
        Ok(self.event_to_stream.read().get(event_id).cloned())
    }
}

/// Type alias for session callback
type SessionCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Configuration for the streamable HTTP server.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
/// use std::sync::Arc;
///
/// // Stateless configuration (for serverless/Lambda)
/// let config = StreamableHttpServerConfig {
///     session_id_generator: None,  // No sessions
///     enable_json_response: false,
///     event_store: None,
///     on_session_initialized: None,
///     on_session_closed: None,
///     http_middleware: None,
///     allowed_origins: None,
///     max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
/// };
///
/// // Stateful configuration with custom session IDs
/// let config = StreamableHttpServerConfig {
///     session_id_generator: Some(Box::new(|| {
///         format!("session-{}", uuid::Uuid::new_v4())
///     })),
///     enable_json_response: false,
///     event_store: None,
///     on_session_initialized: Some(Box::new(|session_id| {
///         println!("Session started: {}", session_id);
///     })),
///     on_session_closed: Some(Box::new(|session_id| {
///         println!("Session ended: {}", session_id);
///     })),
///     http_middleware: None,
///     allowed_origins: None,
///     max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
/// };
/// ```
pub struct StreamableHttpServerConfig {
    /// Function to generate session IDs (None for stateless mode)
    pub session_id_generator: Option<Box<dyn Fn() -> String + Send + Sync>>,
    /// Enable JSON responses instead of SSE
    pub enable_json_response: bool,
    /// Event store for resumability (using concrete type for object safety)
    pub event_store: Option<Arc<InMemoryEventStore>>,
    /// Callback when session is initialized
    pub on_session_initialized: Option<SessionCallback>,
    /// Callback when session is closed
    pub on_session_closed: Option<SessionCallback>,
    /// HTTP middleware chain for request/response processing
    pub http_middleware: Option<Arc<ServerHttpMiddlewareChain>>,
    /// Allowed origins for CORS responses.
    ///
    /// When `Some`, replaces wildcard `*` with origin-locked CORS that
    /// reflects the request's `Origin` only when it appears in this set.
    /// When `None`, defaults to [`AllowedOrigins::localhost()`] at runtime.
    ///
    /// Used by the `StreamableHttpServer` path. The `pmcp::axum::router()`
    /// path uses [`crate::server::axum_router::RouterConfig::allowed_origins`]
    /// instead.
    pub allowed_origins: Option<AllowedOrigins>,
    /// Maximum request body size in bytes.
    ///
    /// Requests exceeding this limit are rejected with HTTP 413 before
    /// any JSON parsing occurs. Default: 4 MB (matches AWS API Gateway).
    pub max_request_bytes: usize,
}

impl std::fmt::Debug for StreamableHttpServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpServerConfig")
            .field("session_id_generator", &self.session_id_generator.is_some())
            .field("enable_json_response", &self.enable_json_response)
            .field("event_store", &self.event_store.is_some())
            .field(
                "on_session_initialized",
                &self.on_session_initialized.is_some(),
            )
            .field("on_session_closed", &self.on_session_closed.is_some())
            .field("http_middleware", &self.http_middleware.is_some())
            .field("allowed_origins", &self.allowed_origins)
            .field("max_request_bytes", &self.max_request_bytes)
            .finish()
    }
}

impl Default for StreamableHttpServerConfig {
    fn default() -> Self {
        Self {
            session_id_generator: Some(Box::new(|| Uuid::new_v4().to_string())),
            enable_json_response: false,
            event_store: Some(Arc::new(InMemoryEventStore::default())),
            on_session_initialized: None,
            on_session_closed: None,
            http_middleware: None,
            allowed_origins: None,
            max_request_bytes: crate::server::limits::DEFAULT_MAX_REQUEST_BYTES,
        }
    }
}

impl StreamableHttpServerConfig {
    /// Create a stateless configuration — no sessions, JSON responses.
    /// Ideal for Lambda and serverless deployments.
    /// Create a stateless configuration for serverless/Lambda deployments.
    ///
    /// Uses [`AllowedOrigins::any()`] because stateless servers are behind
    /// a reverse proxy (API Gateway, `CloudFront`) that handles CORS and
    /// origin validation at the edge. DNS rebinding protection adds no
    /// security value when the MCP server is only reachable via loopback
    /// within a Lambda sandbox or container.
    ///
    /// For servers directly exposed to the internet, use `Default::default()`
    /// instead (which defaults to `AllowedOrigins::localhost()`).
    pub fn stateless() -> Self {
        Self {
            session_id_generator: None,
            enable_json_response: true,
            event_store: None,
            on_session_initialized: None,
            on_session_closed: None,
            http_middleware: None,
            allowed_origins: Some(AllowedOrigins::any()),
            max_request_bytes: crate::server::limits::DEFAULT_MAX_REQUEST_BYTES,
        }
    }
}

/// Session information
#[derive(Debug, Clone)]
struct SessionInfo {
    initialized: bool,
    protocol_version: Option<String>,
}

/// Server state shared across routes.
#[derive(Clone)]
pub(crate) struct ServerState {
    server: Arc<tokio::sync::Mutex<Server>>,
    config: Arc<StreamableHttpServerConfig>,
    /// Pre-resolved allowed origins for CORS and DNS rebinding protection.
    allowed_origins: AllowedOrigins,
    /// Active SSE streams by session ID
    sse_streams: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<TransportMessage>>>>,
    /// Session tracking (session ID -> session info)
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

/// Build the base MCP Router without any Tower layers applied.
///
/// Used by both [`StreamableHttpServer::start()`] and `pmcp::axum::router()`.
pub(crate) fn build_mcp_router(state: ServerState) -> Router<()> {
    Router::new()
        .route("/", post(handle_post_request))
        .route("/", get(handle_get_sse))
        .route("/", delete(handle_delete_session))
        .with_state(state)
}

/// Create a [`ServerState`] for the MCP router.
///
/// Used by `pmcp::axum::router()` to construct state without a full
/// [`StreamableHttpServer`].
pub(crate) fn make_server_state(
    server: Arc<tokio::sync::Mutex<Server>>,
    config: StreamableHttpServerConfig,
) -> ServerState {
    let allowed_origins = config
        .allowed_origins
        .clone()
        .unwrap_or_else(AllowedOrigins::localhost);
    ServerState {
        server,
        config: Arc::new(config),
        allowed_origins,
        sse_streams: Arc::new(RwLock::new(HashMap::new())),
        sessions: Arc::new(RwLock::new(HashMap::new())),
    }
}

/// A streamable HTTP server for MCP.
pub struct StreamableHttpServer {
    addr: SocketAddr,
    state: ServerState,
}

impl std::fmt::Debug for StreamableHttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpServer")
            .field("addr", &self.addr)
            .field("state", &"ServerState { ... }")
            .finish()
    }
}

/// Helper function to create JSON-RPC error response.
///
/// CORS headers are added by the `CorsLayer` Tower middleware, so this
/// function no longer needs to handle them.
fn create_error_response(status: StatusCode, code: i32, message: &str) -> Response {
    let error_body = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": null
    });

    (status, Json(error_body)).into_response()
}

impl StreamableHttpServer {
    /// Creates a new `StreamableHttpServer` with default config
    pub fn new(addr: SocketAddr, server: Arc<tokio::sync::Mutex<Server>>) -> Self {
        Self::with_config(addr, server, StreamableHttpServerConfig::default())
    }

    /// Creates a new `StreamableHttpServer` with custom config
    pub fn with_config(
        addr: SocketAddr,
        server: Arc<tokio::sync::Mutex<Server>>,
        config: StreamableHttpServerConfig,
    ) -> Self {
        let state = make_server_state(server, config);
        Self { addr, state }
    }

    /// Starts the server and returns the bound address and a task handle.
    ///
    /// Applies the same Tower layer security stack as
    /// [`pmcp::axum::router()`](crate::server::axum_router::router):
    /// - `CorsLayer` -- origin-locked CORS (no wildcard `*`)
    /// - [`DnsRebindingLayer`] -- Host/Origin header validation
    /// - [`SecurityHeadersLayer`] -- nosniff, DENY, no-store
    pub async fn start(self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let allowed = self.state.allowed_origins.clone();
        let cors = crate::server::tower_layers::build_mcp_cors_layer(&allowed);

        // Layer ordering: CORS (outermost) -> DnsRebinding -> SecurityHeaders -> handler
        let app = build_mcp_router(self.state)
            .layer(SecurityHeadersLayer::default())
            .layer(DnsRebindingLayer::new(allowed))
            .layer(cors);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Ok((local_addr, server_task))
    }
}

/// Validate request headers and return appropriate error response.
fn validate_headers(headers: &HeaderMap, method: &str) -> std::result::Result<(), Response> {
    match method {
        "POST" => {
            // Validate Content-Type
            if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
                let ct = content_type.to_str().unwrap_or("");
                if !ct.contains(APPLICATION_JSON) {
                    return Err(create_error_response(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        -32700,
                        "Content-Type must be application/json",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    -32700,
                    "Content-Type header is required",
                ));
            }

            // Validate Accept
            if let Some(accept) = headers.get(header::ACCEPT) {
                let accept_str = accept.to_str().unwrap_or("");
                if !accept_str.contains(APPLICATION_JSON) && !accept_str.contains(TEXT_EVENT_STREAM)
                {
                    return Err(create_error_response(
                        StatusCode::NOT_ACCEPTABLE,
                        -32700,
                        "Accept header must include application/json or text/event-stream",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::NOT_ACCEPTABLE,
                    -32700,
                    "Accept header is required",
                ));
            }
        },
        "GET" => {
            // Validate Accept for SSE
            if let Some(accept) = headers.get(header::ACCEPT) {
                let accept_str = accept.to_str().unwrap_or("");
                if !accept_str.contains(TEXT_EVENT_STREAM) {
                    return Err(create_error_response(
                        StatusCode::NOT_ACCEPTABLE,
                        -32700,
                        "Accept header must be text/event-stream for SSE",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::NOT_ACCEPTABLE,
                    -32700,
                    "Accept header is required for SSE",
                ));
            }
        },
        _ => {},
    }

    Ok(())
}

/// Process session for initialization request.
fn process_init_session(
    state: &ServerState,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    if let Some(generator) = &state.config.session_id_generator {
        // Stateful mode
        if let Some(sid) = session_id {
            // Check if session already exists and is initialized
            if let Some(session_info) = state.sessions.read().get(&sid) {
                if session_info.initialized {
                    // Session already initialized - reject re-initialization
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        -32600,
                        "Session already initialized",
                    ));
                }
            }
            // Use existing session ID
            Ok((Some(sid), false))
        } else {
            // Generate new session ID
            let new_id = generator();
            // Create new session entry
            state.sessions.write().insert(
                new_id.clone(),
                SessionInfo {
                    initialized: false,
                    protocol_version,
                },
            );
            if let Some(callback) = &state.config.on_session_initialized {
                callback(&new_id);
            }
            Ok((Some(new_id), true))
        }
    } else {
        // Stateless mode
        Ok((None, false))
    }
}

/// Validate session for non-initialization request.
fn validate_non_init_session(
    state: &ServerState,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if state.config.session_id_generator.is_some() {
        // Stateful mode - require and validate session ID
        match session_id {
            None => {
                // Missing session ID
                Err(create_error_response(
                    StatusCode::BAD_REQUEST,
                    -32600,
                    "Session ID required for non-initialization requests",
                ))
            },
            Some(sid) => {
                // Validate session exists
                if !state.sessions.read().contains_key(&sid) {
                    // Unknown session ID
                    Err(create_error_response(
                        StatusCode::NOT_FOUND,
                        -32600,
                        "Unknown session ID",
                    ))
                } else {
                    Ok(Some(sid))
                }
            },
        }
    } else {
        // Stateless mode
        Ok(None)
    }
}

/// Extract negotiated protocol version from initialize response
fn extract_negotiated_version(response: &TransportMessage) -> Option<String> {
    if let TransportMessage::Response(ref json_resp) = response {
        if let crate::types::jsonrpc::ResponsePayload::Result(ref value) = json_resp.payload {
            if let Ok(init_result) =
                serde_json::from_value::<crate::types::InitializeResult>(value.clone())
            {
                return Some(init_result.protocol_version.0);
            }
        }
    }
    None
}

/// Update session info after initialization
fn update_session_after_init(
    state: &ServerState,
    session_id: Option<&String>,
    negotiated_version: Option<String>,
) {
    if let Some(sid) = session_id {
        if let Some(session_info) = state.sessions.write().get_mut(sid) {
            session_info.initialized = true;
            session_info.protocol_version =
                negotiated_version.or_else(|| Some(crate::DEFAULT_PROTOCOL_VERSION.to_string()));
        }
    }
}

/// Build response with appropriate format (JSON or SSE).
fn build_response(
    state: &ServerState,
    response: TransportMessage,
    session_id: Option<&String>,
) -> Response {
    if state.config.enable_json_response {
        // JSON response mode - use JSON-RPC compatibility layer
        let json_bytes = match crate::shared::StdioTransport::serialize_message(&response) {
            Ok(bytes) => bytes,
            Err(e) => {
                return create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    -32603,
                    &format!("Failed to serialize response: {}", e),
                );
            },
        };

        // Trace serialized bytes for debugging (single-line for CloudWatch compatibility)
        tracing::debug!(
            target: "mcp.http",
            response = %String::from_utf8_lossy(&json_bytes),
            "HTTP response serialized bytes"
        );

        // Parse JSON bytes to Value for Json response
        let json_value: serde_json::Value = match serde_json::from_slice(&json_bytes) {
            Ok(val) => val,
            Err(e) => {
                return create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    -32603,
                    &format!("Failed to parse JSON response: {}", e),
                );
            },
        };

        // Trace final JSON value (compact for CloudWatch compatibility)
        tracing::debug!(
            target: "mcp.http",
            response = %serde_json::to_string(&json_value).unwrap_or_default(),
            "HTTP response (JSON mode)"
        );

        (StatusCode::OK, Json(json_value)).into_response()
    } else {
        // SSE streaming mode
        if let Some(sid) = session_id {
            if let Some(sender) = state.sse_streams.read().get(sid) {
                // Send to existing SSE stream
                let _ = sender.send(response);
                StatusCode::ACCEPTED.into_response()
            } else {
                // Return as SSE stream
                let (tx, rx) = mpsc::unbounded_channel();
                tx.send(response).unwrap();

                let stream = UnboundedReceiverStream::new(rx);
                let sse = Sse::new(stream.map(|msg| {
                    let event_id = Uuid::new_v4().to_string();
                    // Use JSON-RPC compatibility layer for SSE messages
                    let json_bytes = crate::shared::StdioTransport::serialize_message(&msg)
                        .unwrap_or_else(|e| {
                            tracing::error!(target: "mcp.sse", error = %e, "Failed to serialize SSE message");
                            Vec::new()
                        });
                    let json_str =
                        String::from_utf8(json_bytes).unwrap_or_else(|_| "{}".to_string());
                    Ok::<_, Infallible>(
                        Event::default()
                            .id(event_id)
                            .event("message")
                            .data(json_str),
                    )
                }));

                sse.into_response()
            }
        } else {
            // No session, return JSON using JSON-RPC compatibility layer
            let json_bytes = match crate::shared::StdioTransport::serialize_message(&response) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        -32603,
                        &format!("Failed to serialize response: {}", e),
                    );
                },
            };

            let json_value: serde_json::Value = match serde_json::from_slice(&json_bytes) {
                Ok(val) => val,
                Err(e) => {
                    return create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        -32603,
                        &format!("Failed to parse JSON response: {}", e),
                    );
                },
            };

            (StatusCode::OK, Json(json_value)).into_response()
        }
    }
}

/// Validate protocol version for non-init requests.
fn validate_protocol_version(
    state: &ServerState,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if let Some(version) = protocol_version {
        // Check if the provided version is supported
        if !crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str()) {
            return Err(create_error_response(
                StatusCode::BAD_REQUEST,
                -32600,
                &format!("Unsupported protocol version: {}", version),
            ));
        }
    }

    // For stateful mode, also validate against session's negotiated version if exists
    if state.config.session_id_generator.is_some() {
        if let Some(sid) = session_id {
            if let Some(session_info) = state.sessions.read().get(sid.as_str()) {
                if let Some(ref negotiated_version) = session_info.protocol_version {
                    // If header provided, it should match the negotiated version
                    if let Some(provided_version) = protocol_version {
                        if provided_version != negotiated_version {
                            return Err(create_error_response(
                                StatusCode::BAD_REQUEST,
                                -32600,
                                &format!(
                                    "Protocol version mismatch: expected {}, got {}",
                                    negotiated_version, provided_version
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Handle POST requests
async fn handle_post_request(
    State(state): State<ServerState>,
    request: axum::extract::Request<Body>,
) -> impl IntoResponse {
    // Fast path: No HTTP middleware chain
    if state.config.http_middleware.is_none() {
        return handle_post_fast_path(state, request).await;
    }

    // Middleware path: Process through HTTP middleware chain
    handle_post_with_middleware(state, request).await
}

/// Extract and validate authentication from headers.
async fn extract_and_validate_auth(
    state: &ServerState,
    headers: &HeaderMap,
) -> std::result::Result<Option<crate::server::auth::AuthContext>, Response> {
    let server = state.server.lock().await;
    if let Some(auth_provider) = server.get_auth_provider() {
        // Extract Authorization header
        let auth_header = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        // Validate the request and get auth context
        match auth_provider.validate_request(auth_header).await {
            Ok(ctx) => Ok(ctx),
            Err(e) => {
                // Auth validation failed - return 401 Unauthorized
                Err(create_error_response(
                    StatusCode::UNAUTHORIZED,
                    -32003,
                    &format!("Authentication failed: {}", e),
                ))
            },
        }
    } else {
        // No auth provider - try to extract auth from proxy headers (X-PMCP-*)
        // This is used when running behind a proxy that validates auth and forwards claims
        Ok(extract_auth_from_proxy_headers(headers))
    }
}

/// Extract authentication context from proxy-forwarded headers (X-PMCP-*)
///
/// When running behind the pmcp.run proxy or similar, the proxy validates OAuth
/// tokens and forwards user claims as X-PMCP-* headers. This function extracts
/// those headers into an `AuthContext`.
fn extract_auth_from_proxy_headers(
    headers: &HeaderMap,
) -> Option<crate::server::auth::AuthContext> {
    // Check for user ID header (required)
    let user_id = headers
        .get("x-pmcp-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())?;

    // Extract optional claims
    let email = headers
        .get("x-pmcp-user-email")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let name = headers
        .get("x-pmcp-user-name")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let groups = headers
        .get("x-pmcp-user-groups")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let tenant_id = headers
        .get("x-pmcp-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Build claims map
    let mut claims = std::collections::HashMap::new();
    if let Some(ref email) = email {
        claims.insert(
            "email".to_string(),
            serde_json::Value::String(email.clone()),
        );
    }
    if let Some(ref name) = name {
        claims.insert("name".to_string(), serde_json::Value::String(name.clone()));
    }
    if let Some(ref groups) = groups {
        // Parse comma-separated groups into a JSON array so that
        // AuthContext::groups() can deserialize it as Vec<String>.
        let groups_array: Vec<serde_json::Value> = groups
            .split(',')
            .map(|g| serde_json::Value::String(g.trim().to_string()))
            .filter(|v| v.as_str() != Some(""))
            .collect();
        claims.insert("groups".to_string(), serde_json::Value::Array(groups_array));
    }
    if let Some(ref tenant_id) = tenant_id {
        claims.insert(
            "tenant_id".to_string(),
            serde_json::Value::String(tenant_id.clone()),
        );
    }

    tracing::debug!(
        user_id = %user_id,
        email = ?email,
        "Extracted auth context from proxy headers"
    );

    Some(crate::server::auth::AuthContext {
        subject: user_id,
        scopes: vec![],
        claims,
        token: None,
        client_id: None,
        expires_at: None,
        authenticated: true,
    })
}

/// Extract session ID and protocol version headers from a raw axum `HeaderMap`.
///
/// Shared by both the fast path and middleware-path POST handlers so the two
/// entry points read the same two headers in the same way.
fn extract_session_and_protocol_headers(
    headers: &HeaderMap,
) -> (Option<String>, Option<String>) {
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let protocol_version = headers
        .get(MCP_PROTOCOL_VERSION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    (session_id, protocol_version)
}

/// Classify a `TransportMessage` as an `initialize` request or not.
///
/// Extracted so both POST handlers can short-circuit protocol-version
/// validation and session creation without re-implementing the `matches!`.
fn is_initialize_request(message: &TransportMessage) -> bool {
    matches!(
        message,
        TransportMessage::Request { request: Request::Client(boxed), .. }
            if matches!(**boxed, ClientRequest::Initialize(_))
    )
}

/// Resolve the response session ID given the request type and incoming headers.
///
/// For initialize requests this delegates to [`process_init_session`]; for
/// subsequent requests to [`validate_non_init_session`]. Used by both POST
/// handlers.
fn resolve_session_for_request(
    state: &ServerState,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if is_init_request {
        let (sid, _is_new) = process_init_session(state, session_id, protocol_version)?;
        Ok(sid)
    } else {
        validate_non_init_session(state, session_id)
    }
}

/// Compute the outbound `MCP-Protocol-Version` header value.
///
/// Used by both POST handlers to echo either the negotiated version from an
/// initialize response or the session's recorded version for subsequent
/// requests, falling back to `DEFAULT_PROTOCOL_VERSION` when no session is
/// associated with the response.
fn compute_outbound_protocol_version(
    state: &ServerState,
    response_session_id: Option<&String>,
    is_init_request: bool,
    negotiated_version: Option<&str>,
) -> String {
    if is_init_request {
        return negotiated_version
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string());
    }
    if let Some(sid) = response_session_id {
        if let Some(session_info) = state.sessions.read().get(sid) {
            return session_info
                .protocol_version
                .clone()
                .unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string());
        }
    }
    crate::DEFAULT_PROTOCOL_VERSION.to_string()
}

/// Best-effort error-hook dispatch for the middleware path.
///
/// Wraps the `http_middleware.handle_error` call so the caller can short-circuit
/// to a `Response` without a second level of match nesting. The middleware's
/// error hook is intentionally fire-and-forget (return value ignored) — we do
/// not want a misbehaving hook to mask the original failure.
async fn report_middleware_error(
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
    error_kind: &str,
) {
    let err = crate::Error::protocol_msg(error_kind);
    let _ = http_middleware.handle_error(&err, context).await;
}

/// Run request-side middleware and return an error response if rejected.
///
/// Consolidates the `process_request` + error-hook-then-return pattern used
/// at the top of [`handle_post_with_middleware`].
async fn run_request_middleware(
    http_middleware: &ServerHttpMiddlewareChain,
    server_request: &mut crate::server::http_middleware::ServerHttpRequest,
    context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if let Err(e) = http_middleware
        .process_request(server_request, context)
        .await
    {
        let _ = http_middleware.handle_error(&e, context).await;
        return Err(create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            -32603,
            &format!("Middleware rejected request: {}", e),
        ));
    }
    Ok(())
}

/// Parse a JSON-RPC message from raw bytes with middleware-aware error handling.
///
/// On parse failure, runs the request-side response middleware over a
/// manufactured 400 response so downstream observers (logging, metrics) still
/// see the failure.
async fn parse_transport_message_with_middleware(
    body: &[u8],
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> std::result::Result<TransportMessage, Response> {
    match crate::shared::StdioTransport::parse_message(body) {
        Ok(msg) => Ok(msg),
        Err(e) => {
            let mut error_response = ServerHttpResponse::new(
                StatusCode::BAD_REQUEST,
                HeaderMap::new(),
                format!("{{\"error\":\"Invalid JSON: {}\"}}", e).into_bytes(),
            );
            let _ = http_middleware
                .process_response(&mut error_response, context)
                .await;
            Err(into_axum(error_response))
        },
    }
}

/// Extract and validate authentication for the middleware POST path.
///
/// Mirrors [`extract_and_validate_auth`] but wires the middleware error hook
/// into the 401 path. Returns `Ok(None)` when no auth provider is configured
/// (matching the existing middleware-path behavior, which does NOT fall back
/// to proxy-header extraction).
async fn extract_auth_with_middleware(
    state: &ServerState,
    server_request: &crate::server::http_middleware::ServerHttpRequest,
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> std::result::Result<Option<crate::server::auth::AuthContext>, Response> {
    let server = state.server.lock().await;
    let Some(auth_provider) = server.get_auth_provider() else {
        return Ok(None);
    };
    let auth_header = server_request.get_header("authorization");
    match auth_provider.validate_request(auth_header).await {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            let auth_error =
                crate::Error::authentication(format!("Authentication failed: {}", e));
            let _ = http_middleware.handle_error(&auth_error, context).await;
            Err(create_error_response(
                StatusCode::UNAUTHORIZED,
                -32003,
                &format!("Authentication failed: {}", e),
            ))
        },
    }
}

/// Assemble the JSON-RPC success response + headers, run response middleware,
/// and convert to an axum `Response`.
///
/// Returns either the built axum response or a 500 error response when
/// serialization fails.
async fn build_success_response_with_middleware(
    response_msg: &TransportMessage,
    response_session_id: Option<&String>,
    version_to_send: &str,
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> Response {
    let response_body = match serde_json::to_vec(response_msg) {
        Ok(b) => b,
        Err(e) => {
            let serialization_error =
                crate::Error::internal(format!("Failed to serialize response: {}", e));
            let _ = http_middleware
                .handle_error(&serialization_error, context)
                .await;
            return create_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                -32603,
                &format!("Failed to serialize response: {}", e),
            );
        },
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, APPLICATION_JSON.parse().unwrap());
    if let Some(sid) = response_session_id {
        response_headers.insert(MCP_SESSION_ID, sid.parse().unwrap());
    }
    response_headers.insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    let mut server_response =
        ServerHttpResponse::new(StatusCode::OK, response_headers, response_body);

    if let Err(e) = http_middleware
        .process_response(&mut server_response, context)
        .await
    {
        tracing::warn!("Response middleware processing failed: {}", e);
    }

    into_axum(server_response)
}

/// Persist the initialize response event if an event store is configured.
///
/// Shared by both POST handlers — same condition (init OR non-init request
/// with a response session ID), same store-event call, same fire-and-forget
/// error handling.
async fn store_response_event(state: &ServerState, response_session_id: Option<&String>, response_msg: &TransportMessage) {
    if let Some(event_store) = &state.config.event_store {
        if let Some(sid) = response_session_id {
            let event_id = Uuid::new_v4().to_string();
            let _ = event_store.store_event(sid, &event_id, response_msg).await;
        }
    }
}

/// Fast path handler without HTTP middleware
async fn handle_post_fast_path(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;

    // Read body to string
    let body_bytes = match axum::body::to_bytes(body, state.config.max_request_bytes).await {
        Ok(b) => b,
        Err(e) => {
            return create_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                -32600,
                &format!("Request body exceeds limit: {}", e),
            );
        },
    };
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    // Validate headers
    if let Err(error_response) = validate_headers(&headers, "POST") {
        return error_response;
    }

    // Parse the JSON body using JSON-RPC compatibility layer
    let message: TransportMessage =
        match crate::shared::StdioTransport::parse_message(body.as_bytes()) {
            Ok(msg) => msg,
            Err(e) => {
                return create_error_response(
                    StatusCode::BAD_REQUEST,
                    -32700,
                    &format!("Invalid JSON: {}", e),
                );
            },
        };

    // Extract session ID from headers
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Extract protocol version from headers
    let protocol_version = headers
        .get(MCP_PROTOCOL_VERSION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check if this is an initialization request
    let is_init_request = matches!(
        &message,
        TransportMessage::Request { request: Request::Client(boxed), .. }
            if matches!(**boxed, ClientRequest::Initialize(_))
    );

    // Handle session ID logic based on request type
    let (response_session_id, _is_new_session) = if is_init_request {
        match process_init_session(&state, session_id.clone(), protocol_version.clone()) {
            Ok(result) => result,
            Err(error_response) => return error_response,
        }
    } else {
        match validate_non_init_session(&state, session_id.clone()) {
            Ok(sid) => (sid, false),
            Err(error_response) => return error_response,
        }
    };

    // Validate protocol version for non-init requests
    if !is_init_request {
        if let Err(error_response) =
            validate_protocol_version(&state, session_id.as_ref(), protocol_version.as_ref())
        {
            return error_response;
        }
    }

    // Extract and validate authentication if auth_provider is configured
    let auth_context = match extract_and_validate_auth(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    // Process the message
    match message {
        TransportMessage::Request { id, request } => {
            let server = state.server.lock().await;
            let json_response = server.handle_request(id, request, auth_context).await;

            // Trace response payload (compact for CloudWatch compatibility)
            tracing::debug!(
                target: "mcp.http",
                response = %serde_json::to_string(&json_response).unwrap_or_default(),
                "StreamableHttpServer response"
            );

            let response = TransportMessage::Response(json_response.clone());

            // Handle initialization response
            let negotiated_version = if is_init_request {
                let version = extract_negotiated_version(&response);
                update_session_after_init(&state, response_session_id.as_ref(), version.clone());
                version
            } else {
                None
            };

            // Store event if we have an event store
            if let Some(event_store) = &state.config.event_store {
                if let Some(sid) = &response_session_id {
                    let event_id = Uuid::new_v4().to_string();
                    let _ = event_store.store_event(sid, &event_id, &response).await;
                }
            }

            // Build response with headers
            let mut response = build_response(&state, response, session_id.as_ref());

            // Always add session header in stateful mode
            if let Some(sid) = &response_session_id {
                response
                    .headers_mut()
                    .insert(MCP_SESSION_ID, sid.parse().unwrap());
            }

            // Add protocol version header
            let version_to_send = if is_init_request {
                // For init responses, use the negotiated version
                negotiated_version.unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string())
            } else {
                // For subsequent responses, echo the session's negotiated version
                if let Some(ref sid) = response_session_id {
                    if let Some(session_info) = state.sessions.read().get(sid) {
                        session_info
                            .protocol_version
                            .clone()
                            .unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string())
                    } else {
                        crate::DEFAULT_PROTOCOL_VERSION.to_string()
                    }
                } else {
                    // Stateless mode or no session - use default
                    crate::DEFAULT_PROTOCOL_VERSION.to_string()
                }
            };

            response
                .headers_mut()
                .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

            response
        },
        TransportMessage::Notification { .. } => {
            // Notifications get 202 Accepted
            StatusCode::ACCEPTED.into_response()
        },
        TransportMessage::Response(_) => StatusCode::ACCEPTED.into_response(),
    }
}

/// Build the HTTP middleware context from a middleware-adapted request.
fn build_middleware_context(
    server_request: &crate::server::http_middleware::ServerHttpRequest,
) -> ServerHttpContext {
    let session_id = server_request
        .get_header(MCP_SESSION_ID)
        .map(str::to_string);
    let request_id = server_request
        .get_header("x-request-id")
        .map_or_else(|| Uuid::new_v4().to_string(), str::to_string);
    ServerHttpContext {
        request_id,
        start_time: std::time::Instant::now(),
        session_id,
    }
}

/// Convert the axum request into a middleware `ServerHttpRequest`, handling
/// the body-size-limit failure path.
async fn convert_axum_to_middleware_request(
    request: axum::extract::Request<Body>,
    max_request_bytes: usize,
) -> std::result::Result<crate::server::http_middleware::ServerHttpRequest, Response> {
    let (parts, body) = request.into_parts();
    from_axum_with_limit(parts, body, max_request_bytes)
        .await
        .map_err(|e| {
            create_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                -32600,
                &format!("Request body exceeds limit: {}", e),
            )
        })
}

/// Resolve the session ID and run the middleware error hook on failure.
///
/// Wraps [`resolve_session_for_request`] so the caller doesn't have to
/// branch on `is_init_request` for the error-kind string.
async fn resolve_session_with_error_hook(
    state: &ServerState,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<Option<String>, Response> {
    match resolve_session_for_request(state, is_init_request, session_id, protocol_version) {
        Ok(sid) => Ok(sid),
        Err(error_response) => {
            let kind = if is_init_request {
                "Session initialization failed"
            } else {
                "Session validation failed"
            };
            report_middleware_error(http_middleware, http_context, kind).await;
            Err(error_response)
        },
    }
}

/// Run protocol-version validation for non-init requests, wiring the middleware
/// error hook on failure. A no-op for init requests.
async fn validate_protocol_version_with_error_hook(
    state: &ServerState,
    is_init_request: bool,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if is_init_request {
        return Ok(());
    }
    if let Err(error_response) = validate_protocol_version(state, session_id, protocol_version) {
        report_middleware_error(
            http_middleware,
            http_context,
            "Protocol version validation failed",
        )
        .await;
        return Err(error_response);
    }
    Ok(())
}

/// Dispatch the parsed `TransportMessage` on the middleware path.
///
/// Handles `Request` (server-handled + response assembly), `Notification`
/// (202 Accepted), and `Response` (202 Accepted) in separate arms.
async fn dispatch_message_with_middleware(
    state: &ServerState,
    message: TransportMessage,
    is_init_request: bool,
    response_session_id: Option<String>,
    auth_context: Option<crate::server::auth::AuthContext>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    match message {
        TransportMessage::Request { id, request } => {
            let json_response = {
                let server = state.server.lock().await;
                server.handle_request(id, request, auth_context).await
            };
            let response_msg = TransportMessage::Response(json_response);

            let negotiated_version = if is_init_request {
                let version = extract_negotiated_version(&response_msg);
                update_session_after_init(state, response_session_id.as_ref(), version.clone());
                version
            } else {
                None
            };

            store_response_event(state, response_session_id.as_ref(), &response_msg).await;

            let version_to_send = compute_outbound_protocol_version(
                state,
                response_session_id.as_ref(),
                is_init_request,
                negotiated_version.as_deref(),
            );

            build_success_response_with_middleware(
                &response_msg,
                response_session_id.as_ref(),
                &version_to_send,
                http_middleware,
                http_context,
            )
            .await
        },
        TransportMessage::Notification { .. } | TransportMessage::Response(_) => {
            StatusCode::ACCEPTED.into_response()
        },
    }
}

/// Handler with HTTP middleware integration.
///
/// Refactored in 75-01 Task 1a-A: extracted
/// [`convert_axum_to_middleware_request`], [`build_middleware_context`],
/// [`run_request_middleware`], [`parse_transport_message_with_middleware`],
/// [`resolve_session_for_request`], [`extract_auth_with_middleware`], and
/// [`dispatch_message_with_middleware`] so this orchestrator is a thin
/// early-return pipeline.
async fn handle_post_with_middleware(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> Response {
    let http_middleware = state
        .config
        .http_middleware
        .as_ref()
        .expect("Middleware chain must exist");

    let mut server_request =
        match convert_axum_to_middleware_request(request, state.config.max_request_bytes).await {
            Ok(req) => req,
            Err(response) => return response,
        };

    let http_context = build_middleware_context(&server_request);

    if let Err(response) =
        run_request_middleware(http_middleware, &mut server_request, &http_context).await
    {
        return response;
    }

    if let Err(error_response) = validate_headers(&server_request.headers, "POST") {
        report_middleware_error(http_middleware, &http_context, "Header validation failed").await;
        return error_response;
    }

    let message = match parse_transport_message_with_middleware(
        &server_request.body,
        http_middleware,
        &http_context,
    )
    .await
    {
        Ok(msg) => msg,
        Err(response) => return response,
    };

    let (session_id, protocol_version) =
        extract_session_and_protocol_headers(&server_request.headers);
    let is_init_request = is_initialize_request(&message);

    let response_session_id = match resolve_session_with_error_hook(
        &state,
        is_init_request,
        session_id.clone(),
        protocol_version.clone(),
        http_middleware,
        &http_context,
    )
    .await
    {
        Ok(sid) => sid,
        Err(response) => return response,
    };

    if let Err(response) = validate_protocol_version_with_error_hook(
        &state,
        is_init_request,
        session_id.as_ref(),
        protocol_version.as_ref(),
        http_middleware,
        &http_context,
    )
    .await
    {
        return response;
    }

    let auth_context = match extract_auth_with_middleware(
        &state,
        &server_request,
        http_middleware,
        &http_context,
    )
    .await
    {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    dispatch_message_with_middleware(
        &state,
        message,
        is_init_request,
        response_session_id,
        auth_context,
        http_middleware,
        &http_context,
    )
    .await
}

/// Handle GET requests for SSE streams
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    // Validate headers
    if let Err(error_response) = validate_headers(&headers, "GET") {
        return error_response;
    }

    // Extract session ID
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Validate or generate session ID
    let session_id = if let Some(sid) = session_id {
        // Validate session exists
        if state.config.session_id_generator.is_some() && !state.sessions.read().contains_key(&sid)
        {
            return create_error_response(StatusCode::NOT_FOUND, -32600, "Unknown session ID");
        }
        sid
    } else if let Some(generator) = &state.config.session_id_generator {
        // Generate new session for GET SSE
        let new_id = generator();
        state.sessions.write().insert(
            new_id.clone(),
            SessionInfo {
                initialized: true, // GET SSE implicitly initializes
                protocol_version: None,
            },
        );
        if let Some(callback) = &state.config.on_session_initialized {
            callback(&new_id);
        }
        new_id
    } else {
        // Stateless mode, no SSE
        return create_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            -32601,
            "SSE not supported in stateless mode",
        );
    };

    // Check if stream already exists for this session
    if state.sse_streams.read().contains_key(&session_id) {
        return create_error_response(
            StatusCode::CONFLICT,
            -32600,
            "SSE stream already exists for this session",
        );
    }

    // Create SSE stream
    let (tx, rx) = mpsc::unbounded_channel();
    state
        .sse_streams
        .write()
        .insert(session_id.clone(), tx.clone());

    // Check for Last-Event-ID for resumability
    if let Some(last_event_id) = headers.get(LAST_EVENT_ID) {
        if let Ok(last_id) = last_event_id.to_str() {
            if let Some(event_store) = &state.config.event_store {
                // Replay events after the last event ID
                if let Ok(events) = event_store.replay_events_after(last_id).await {
                    for (_event_id, msg) in events {
                        let _ = tx.send(msg);
                    }
                }
            }
        }
    }

    let stream = UnboundedReceiverStream::new(rx);
    let session_id_header = session_id.clone();

    let sse = Sse::new(stream.map(move |msg| {
        let event_id = Uuid::new_v4().to_string();

        // Store event if we have an event store
        if let Some(event_store) = &state.config.event_store {
            let sid = session_id.clone();
            let msg_clone = msg.clone();
            let store = event_store.clone();
            let event_id_clone = event_id.clone();
            tokio::spawn(async move {
                let _ = store.store_event(&sid, &event_id_clone, &msg_clone).await;
            });
        }

        Ok::<_, Infallible>(
            Event::default()
                .id(event_id)
                .event("message")
                .data(serde_json::to_string(&msg).unwrap()),
        )
    }));

    let mut response = sse.into_response();

    // Add session ID header
    response
        .headers_mut()
        .insert(MCP_SESSION_ID, session_id_header.parse().unwrap());

    // Add SSE-specific headers for hardening
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    // Content-Type is already set by Axum's Sse

    response
}

/// Handle DELETE requests to terminate sessions
async fn handle_delete_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Extract session ID
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(sid) = session_id {
        // Check if session exists
        let session_exists = state.sessions.read().contains_key(&sid);

        if !session_exists && state.config.session_id_generator.is_some() {
            // Unknown session in stateful mode
            return create_error_response(StatusCode::NOT_FOUND, -32600, "Unknown session ID");
        }

        // Remove SSE stream if exists
        state.sse_streams.write().remove(&sid);

        // Remove session from tracking
        state.sessions.write().remove(&sid);

        // Notify callback
        if let Some(callback) = &state.config.on_session_closed {
            callback(&sid);
        }

        (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
    } else {
        // No session to delete
        create_error_response(StatusCode::NOT_FOUND, -32600, "No session ID provided")
    }
}
