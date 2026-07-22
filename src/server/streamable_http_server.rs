//! Streamable HTTP server implementation for MCP.
use crate::error::Result;
use crate::server::http_middleware::{
    adapters::{from_axum_with_limit, into_axum},
    ServerHttpContext, ServerHttpMiddlewareChain, ServerHttpResponse,
};
use crate::server::tower_layers::{AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer};
use crate::server::Server;
use crate::shared::http_constants::{
    APPLICATION_JSON, LAST_EVENT_ID, MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION, MCP_SESSION_ID,
    TEXT_EVENT_STREAM,
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

// ===========================================================================
// v2 required-header gate (Plan 112-06, VERS-05 / D-05 / D-06 / D-11).
//
// The v2 verdict is Plan 04's RESOLVED `ProtocolContext.era`, CONSUMED here —
// this layer never runs a second independent era resolver (Pitfall 2). The
// streamable-HTTP inbound handler resolves the context ONCE (for this gate) and
// threads that SAME value into `Server::handle_request_with_context`, so
// dispatch is a pass-through, not a re-resolve.
//
// The classifier is decomposed into small single-responsibility helpers, each
// well under cognitive-complexity 25 (PMAT CI gate — WARNING 4), composed by a
// thin top-level `classify_v2_request`. Every new header-violation error sources
// its JSON-RPC code from `error_codes::` (VERS-06); no new bare -326xx literal.
// ===========================================================================

/// Upper bound on a decoded header value we will consider (`DoS` guard, T-112-13).
const MAX_V2_HEADER_VALUE_LEN: usize = 8192;

/// The decoded `MCP-Protocol-Version` header, classified for the era matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderProtocolVersion {
    /// Header not present.
    Absent,
    /// Present but non-UTF-8 or oversized — decoded without panicking.
    Malformed,
    /// Exactly `2026-07-28` (the v2 era).
    V2,
    /// Any other decodable value (v1 or unknown).
    Other,
}

/// The classification of an opted-in request over the header/`_meta` matrix.
enum V2Classification {
    /// v1 / both signals non-v2 → run the legacy path with zero enforcement.
    Legacy,
    /// v2 on BOTH the header and the resolved `_meta` era → enforce headers.
    Enforce,
    /// A conflict cell (v2-header/non-v2-`_meta` or vice-versa) → fail closed.
    Reject(i32, &'static str),
}

/// Outcome of the whole v2 gate for one request.
enum V2GateOutcome {
    /// Not a v2 request (v1 / non-opted-in) — dispatch normally, no v2 headers.
    Passthrough,
    /// Accepted v2 request — dispatch, then echo these headers outbound.
    EnforceOk { method: String, name: String },
    /// Rejected — build a 4xx structured JSON-RPC error with this code/message.
    Reject(i32, String),
}

/// Decode the `MCP-Protocol-Version` header without panicking (T-112-13).
fn decode_version_header(headers: &HeaderMap) -> HeaderProtocolVersion {
    let Some(raw) = headers.get(MCP_PROTOCOL_VERSION) else {
        return HeaderProtocolVersion::Absent;
    };
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN {
        return HeaderProtocolVersion::Malformed;
    }
    match raw.to_str() {
        Err(_) => HeaderProtocolVersion::Malformed,
        Ok(s) if s == crate::types::protocol::PROTOCOL_VERSION_2026_07_28 => {
            HeaderProtocolVersion::V2
        },
        Ok(_) => HeaderProtocolVersion::Other,
    }
}

/// Read a header as a bounded UTF-8 string, or `None` if absent/malformed.
fn bounded_header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?;
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN {
        return None;
    }
    raw.to_str().ok().map(str::to_string)
}

/// Classify one cell of the header/`_meta` matrix on an OPTED-IN server.
///
/// `meta_is_v2` is Plan 04's resolved `ProtocolContext.era == Era::V2` — the
/// authoritative per-request verdict this layer CONSUMES (Pitfall 2 / D-11).
fn classify_era_cell(header: HeaderProtocolVersion, meta_is_v2: bool) -> V2Classification {
    let header_is_v2 = matches!(header, HeaderProtocolVersion::V2);
    match (header_is_v2, meta_is_v2) {
        (true, true) => V2Classification::Enforce,
        (false, false) => V2Classification::Legacy,
        (true, false) => V2Classification::Reject(
            crate::types::protocol::error_codes::INVALID_REQUEST,
            "MCP-Protocol-Version header claims v2 but _meta protocolVersion disagrees",
        ),
        (false, true) => V2Classification::Reject(
            crate::types::protocol::error_codes::INVALID_REQUEST,
            "_meta claims v2 but MCP-Protocol-Version header is absent or not 2026-07-28",
        ),
    }
}

/// Require all THREE v2 headers (VERS-05 / D-05); return `(method, name)`.
fn require_three_headers(
    headers: &HeaderMap,
) -> std::result::Result<(String, String), &'static str> {
    let version_present = headers.get(MCP_PROTOCOL_VERSION).is_some();
    let method = bounded_header_str(headers, MCP_METHOD);
    let name = bounded_header_str(headers, MCP_NAME);
    match (version_present, method, name) {
        (true, Some(m), Some(n)) => Ok((m, n)),
        _ => Err("v2 requests must carry Mcp-Method, Mcp-Name and MCP-Protocol-Version headers"),
    }
}

/// Cross-check `Mcp-Method` against the JSON-RPC body `method` (D-06).
fn cross_check_method(
    mcp_method: &str,
    body_method: Option<&str>,
) -> std::result::Result<(), &'static str> {
    match body_method {
        Some(bm) if bm == mcp_method => Ok(()),
        _ => Err("Mcp-Method header does not match the JSON-RPC body method"),
    }
}

/// Methods whose logical name lives in `params.name` and must be cross-checked.
fn is_name_bearing_method(method: &str) -> bool {
    matches!(method, "tools/call" | "prompts/get" | "resources/read")
}

/// Cross-check `Mcp-Name` against `params.name` for name-bearing methods (D-06).
/// Name-less methods are presence-only (already enforced upstream).
fn cross_check_name(
    mcp_name: &str,
    method: &str,
    body_name: Option<&str>,
) -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) {
        return Ok(());
    }
    match body_name {
        Some(bn) if bn == mcp_name => Ok(()),
        _ => Err("Mcp-Name header does not match the request's logical name (params.name)"),
    }
}

/// The thin top-level classifier over the full matrix (cog-safe composition).
///
/// Inputs: decoded header signals + Plan-04 resolved `meta_is_v2` + the untrusted
/// body `method`/`params.name`. Output: accept (with echo headers) | reject(code)
/// | passthrough. Pure and non-panicking — property-tested.
fn classify_v2_request(
    headers: &HeaderMap,
    meta_is_v2: bool,
    body_method: Option<&str>,
    body_name: Option<&str>,
) -> V2GateOutcome {
    use crate::types::protocol::error_codes::INVALID_REQUEST;
    let header = decode_version_header(headers);
    match classify_era_cell(header, meta_is_v2) {
        V2Classification::Legacy => V2GateOutcome::Passthrough,
        V2Classification::Reject(code, msg) => V2GateOutcome::Reject(code, msg.to_string()),
        V2Classification::Enforce => {
            let (method, name) = match require_three_headers(headers) {
                Ok(pair) => pair,
                Err(msg) => return V2GateOutcome::Reject(INVALID_REQUEST, msg.to_string()),
            };
            if let Err(msg) = cross_check_method(&method, body_method) {
                return V2GateOutcome::Reject(INVALID_REQUEST, msg.to_string());
            }
            if let Err(msg) = cross_check_name(&name, &method, body_name) {
                return V2GateOutcome::Reject(INVALID_REQUEST, msg.to_string());
            }
            V2GateOutcome::EnforceOk { method, name }
        },
    }
}

/// Extract the untrusted `(method, params.name)` pair from the raw JSON-RPC body.
///
/// Re-parses the raw bytes (the transport parse already succeeded) so the
/// cross-check compares the header against the LITERAL wire `method`/`params.name`
/// a WAF would see — the smuggling-relevant view (D-06). Never panics.
fn extract_body_method_and_name(body: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return (None, None);
    };
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let name = value
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    (method, name)
}

/// Emit the three required v2 headers outbound WITHOUT panicking (T-112-13).
///
/// Sets `Mcp-Method`, `Mcp-Name` and forces `MCP-Protocol-Version` to the v2
/// value. Called on BOTH the success and structured-error response of an
/// accepted v2 request. On an unrepresentable value the individual insert is
/// skipped (caller already produced a valid response) rather than unwrapping.
fn apply_v2_outbound_headers(headers: &mut HeaderMap, method: &str, name: &str) {
    if let Ok(v) = HeaderValue::from_str(method) {
        headers.insert(MCP_METHOD, v);
    }
    if let Ok(v) = HeaderValue::from_str(name) {
        headers.insert(MCP_NAME, v);
    }
    if let Ok(v) = HeaderValue::from_str(crate::types::protocol::PROTOCOL_VERSION_2026_07_28) {
        headers.insert(MCP_PROTOCOL_VERSION, v);
    }
}

/// Run the v2 gate for a parsed `Request` on an opted-in server.
///
/// Resolves the `ProtocolContext` ONCE via the SAME shared resolver dispatch
/// uses, and returns both that resolved context (to thread into
/// `handle_request_with_context`) and the gate outcome. The HTTP layer CONSUMES
/// the resolved era — it never re-derives "is v2" from the raw header alone.
async fn run_v2_header_gate(
    state: &ServerState,
    request: &Request,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    let resolved = {
        let server = state.server.lock().await;
        // Non-opted-in servers run ZERO era-detection — v1 path byte-for-byte
        // unchanged (D-04). `resolve_ingress_protocol_context` already
        // short-circuits to `Ok(None)` there.
        server.resolve_ingress_protocol_context(request)
    };
    let context = match resolved {
        Ok(ctx) => ctx,
        Err(err) => {
            // Unsupported/malformed per-request version — reject via the SAME
            // mapping dispatch uses (structured INVALID_PARAMS, VERS-06).
            let (code, message) = crate::server::core::negotiation_error_to_rejection(&err);
            return (None, V2GateOutcome::Reject(code, message));
        },
    };
    // `Ok(None)` == not opted in → zero enforcement (D-04).
    let Some(ref pc) = context else {
        return (context.clone(), V2GateOutcome::Passthrough);
    };
    let meta_is_v2 = pc.era == crate::types::protocol::Era::V2;
    let (body_method, body_name) = extract_body_method_and_name(raw_body);
    let outcome = classify_v2_request(
        headers,
        meta_is_v2,
        body_method.as_deref(),
        body_name.as_deref(),
    );
    (context, outcome)
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

/// Validate `Content-Type: application/json` for POST.
fn validate_content_type_json(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            -32700,
            "Content-Type header is required",
        ));
    };
    let ct = content_type.to_str().unwrap_or("");
    if !ct.contains(APPLICATION_JSON) {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            -32700,
            "Content-Type must be application/json",
        ));
    }
    Ok(())
}

/// Validate `Accept: application/json` or `text/event-stream` for POST.
fn validate_accept_post(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(accept) = headers.get(header::ACCEPT) else {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            -32700,
            "Accept header is required",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(APPLICATION_JSON) && !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            -32700,
            "Accept header must include application/json or text/event-stream",
        ));
    }
    Ok(())
}

/// Validate `Accept: text/event-stream` for GET (SSE).
fn validate_accept_sse(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(accept) = headers.get(header::ACCEPT) else {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            -32700,
            "Accept header is required for SSE",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            -32700,
            "Accept header must be text/event-stream for SSE",
        ));
    }
    Ok(())
}

/// Validate request headers and return appropriate error response.
///
/// Refactored in 75-01 Task 1a-A: per-header checks extracted to
/// [`validate_content_type_json`], [`validate_accept_post`], and
/// [`validate_accept_sse`] (P3).
fn validate_headers(headers: &HeaderMap, method: &str) -> std::result::Result<(), Response> {
    match method {
        "POST" => {
            validate_content_type_json(headers)?;
            validate_accept_post(headers)?;
        },
        "GET" => validate_accept_sse(headers)?,
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
/// Serialize a `TransportMessage` and re-parse as a `serde_json::Value`, or
/// return a 500 error response on failure.
fn serialize_response_as_json_value(
    response: &TransportMessage,
) -> std::result::Result<serde_json::Value, Response> {
    let json_bytes = crate::shared::StdioTransport::serialize_message(response).map_err(|e| {
        create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            -32603,
            &format!("Failed to serialize response: {}", e),
        )
    })?;
    tracing::debug!(
        target: "mcp.http",
        response = %String::from_utf8_lossy(&json_bytes),
        "HTTP response serialized bytes"
    );
    let json_value: serde_json::Value = serde_json::from_slice(&json_bytes).map_err(|e| {
        create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            -32603,
            &format!("Failed to parse JSON response: {}", e),
        )
    })?;
    Ok(json_value)
}

/// Build an OK JSON response body from a `TransportMessage`.
fn build_json_response(response: &TransportMessage, trace_source: &'static str) -> Response {
    let json_value = match serialize_response_as_json_value(response) {
        Ok(v) => v,
        Err(error_response) => return error_response,
    };
    tracing::debug!(
        target: "mcp.http",
        source = trace_source,
        response = %serde_json::to_string(&json_value).unwrap_or_default(),
        "HTTP response (JSON mode)"
    );
    (StatusCode::OK, Json(json_value)).into_response()
}

/// Build an SSE streaming response from a single `TransportMessage`.
///
/// Each element of the stream is serialized via `StdioTransport` for
/// JSON-RPC-compat framing.
fn build_sse_response_from_single_message(response: TransportMessage) -> Response {
    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(response).unwrap();
    let stream = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(stream.map(|msg| {
        let event_id = Uuid::new_v4().to_string();
        let json_bytes =
            crate::shared::StdioTransport::serialize_message(&msg).unwrap_or_else(|e| {
                tracing::error!(target: "mcp.sse", error = %e, "Failed to serialize SSE message");
                Vec::new()
            });
        let json_str = String::from_utf8(json_bytes).unwrap_or_else(|_| "{}".to_string());
        Ok::<_, Infallible>(
            Event::default()
                .id(event_id)
                .event("message")
                .data(json_str),
        )
    }));
    sse.into_response()
}

/// Build response with appropriate format (JSON or SSE).
///
/// Refactored in 75-01 Task 1a-A (P1): extracted
/// [`serialize_response_as_json_value`], [`build_json_response`], and
/// [`build_sse_response_from_single_message`] so this function is a thin
/// per-mode dispatcher.
fn build_response(
    state: &ServerState,
    response: TransportMessage,
    session_id: Option<&String>,
) -> Response {
    if state.config.enable_json_response {
        return build_json_response(&response, "JSON mode");
    }
    // SSE streaming mode
    let Some(sid) = session_id else {
        return build_json_response(&response, "SSE no-session fallback");
    };
    if let Some(sender) = state.sse_streams.read().get(sid) {
        let _ = sender.send(response);
        return StatusCode::ACCEPTED.into_response();
    }
    build_sse_response_from_single_message(response)
}

/// Validate that a provided protocol version is in the supported set.
fn validate_protocol_version_supported(
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    let Some(version) = protocol_version else {
        return Ok(());
    };
    if crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str()) {
        return Ok(());
    }
    Err(create_error_response(
        StatusCode::BAD_REQUEST,
        -32600,
        &format!("Unsupported protocol version: {}", version),
    ))
}

/// In stateful mode, verify that a provided protocol version matches the
/// session's recorded negotiated version (if any). Pure early-return chain.
fn validate_protocol_version_matches_session(
    state: &ServerState,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if state.config.session_id_generator.is_none() {
        return Ok(());
    }
    let Some(sid) = session_id else {
        return Ok(());
    };
    let sessions = state.sessions.read();
    let Some(session_info) = sessions.get(sid.as_str()) else {
        return Ok(());
    };
    let Some(negotiated_version) = session_info.protocol_version.as_ref() else {
        return Ok(());
    };
    let Some(provided_version) = protocol_version else {
        return Ok(());
    };
    if provided_version == negotiated_version {
        return Ok(());
    }
    Err(create_error_response(
        StatusCode::BAD_REQUEST,
        -32600,
        &format!(
            "Protocol version mismatch: expected {}, got {}",
            negotiated_version, provided_version
        ),
    ))
}

/// Validate the `MCP-Protocol-Version` header (if any) against the supported
/// set and any negotiated session version.
///
/// Refactored in 75-01 Task 1a-A (P2): extracted
/// [`validate_protocol_version_supported`] and
/// [`validate_protocol_version_matches_session`] as early-return chains.
fn validate_protocol_version(
    state: &ServerState,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    validate_protocol_version_supported(protocol_version)?;
    validate_protocol_version_matches_session(state, session_id, protocol_version)
}

/// Handle POST requests
async fn handle_post_request(
    State(state): State<ServerState>,
    request: axum::extract::Request<Body>,
) -> impl IntoResponse {
    // Fast path: No HTTP middleware chain.
    // `Box::pin` both dispatch futures: the v2 header gate (Plan 112-06) grows the
    // POST future past clippy's large_future threshold; boxing keeps the axum
    // handler future small without changing behavior.
    if state.config.http_middleware.is_none() {
        return Box::pin(handle_post_fast_path(state, request)).await;
    }

    // Middleware path: Process through HTTP middleware chain
    Box::pin(handle_post_with_middleware(state, request)).await
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

    // pmcp.run mcp-proxy emits `x-pmcp-claim-custom-<kebab-suffix>: <value>` for every
    // Cognito `custom:*` user attribute it sees in the authorizer context (see
    // rust-mcp-sdk docs/proxy-contract.md). Re-insert each one into `claims` under
    // the canonical Cognito attribute name `custom:<snake_suffix>` so consumers
    // can read either via `ctx.claim::<T>("custom:foo")` or the raw `ctx.claims` map.
    //
    // mcp-proxy strips inbound `x-pmcp-claim-custom-*` from client requests before
    // injection, so every header observed here is platform-trusted.
    for (name, value) in headers {
        let Some(suffix) = name.as_str().strip_prefix("x-pmcp-claim-custom-") else {
            continue;
        };
        let Ok(val_str) = value.to_str() else {
            continue;
        };
        if suffix.is_empty() || val_str.is_empty() {
            continue;
        }
        let snake: String = suffix
            .chars()
            .map(|c| if c == '-' { '_' } else { c })
            .collect();
        claims.insert(
            format!("custom:{}", snake),
            serde_json::Value::String(val_str.to_string()),
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
fn extract_session_and_protocol_headers(headers: &HeaderMap) -> (Option<String>, Option<String>) {
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
        return negotiated_version.map_or_else(
            || crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            std::string::ToString::to_string,
        );
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
            let auth_error = crate::Error::authentication(format!("Authentication failed: {}", e));
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
async fn store_response_event(
    state: &ServerState,
    response_session_id: Option<&String>,
    response_msg: &TransportMessage,
) {
    if let Some(event_store) = &state.config.event_store {
        if let Some(sid) = response_session_id {
            let event_id = Uuid::new_v4().to_string();
            let _ = event_store.store_event(sid, &event_id, response_msg).await;
        }
    }
}

/// Fast path handler without HTTP middleware
/// Read the axum request body with enforced byte limit.
///
/// Returns the body bytes as a `String` on success, or a 413 error response
/// when the body exceeds `max_bytes`.
async fn read_body_with_limit(
    body: Body,
    max_bytes: usize,
) -> std::result::Result<String, Response> {
    let body_bytes = axum::body::to_bytes(body, max_bytes).await.map_err(|e| {
        create_error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            -32600,
            &format!("Request body exceeds limit: {}", e),
        )
    })?;
    Ok(String::from_utf8_lossy(&body_bytes).to_string())
}

/// Parse a JSON-RPC message on the fast path, returning a 400 error response
/// on failure.
fn parse_transport_message_fast(body: &[u8]) -> std::result::Result<TransportMessage, Response> {
    crate::shared::StdioTransport::parse_message(body).map_err(|e| {
        create_error_response(
            StatusCode::BAD_REQUEST,
            -32700,
            &format!("Invalid JSON: {}", e),
        )
    })
}

/// Handle the successful-request arm on the fast path: dispatch to the
/// server, persist event, and attach session/version headers to the response.
/// Per-request dispatch inputs threaded into the fast-path handler.
///
/// Bundles the response-shaping flags with the Plan-04-resolved
/// `ProtocolContext` (threaded into dispatch, never re-resolved — Plan 06) and
/// the optional v2 outbound headers to echo on success AND error.
struct FastPathDispatch {
    is_init_request: bool,
    response_session_id: Option<String>,
    /// Plan-04-resolved `ProtocolContext`, CONSUMED at dispatch (D-11).
    protocol_context: Option<crate::types::protocol::ProtocolContext>,
    /// When `Some((method, name))`, this is an accepted v2 request whose
    /// response echoes `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version`.
    v2_outbound: Option<(String, String)>,
}

async fn handle_fast_path_request(
    state: &ServerState,
    id: crate::types::RequestId,
    request: Request,
    auth_context: Option<crate::server::auth::AuthContext>,
    dispatch: FastPathDispatch,
    session_id: Option<&String>,
) -> Response {
    let FastPathDispatch {
        is_init_request,
        response_session_id,
        protocol_context,
        v2_outbound,
    } = dispatch;

    let json_response = {
        let server = state.server.lock().await;
        // Thread the ALREADY-RESOLVED ProtocolContext into dispatch — the HTTP
        // layer resolved it once for the header gate; dispatch does NOT
        // re-resolve (Plan 06 / D-11 / Pitfall 2).
        server
            .handle_request_with_context(id, request, auth_context, protocol_context)
            .await
    };

    tracing::debug!(
        target: "mcp.http",
        response = %serde_json::to_string(&json_response).unwrap_or_default(),
        "StreamableHttpServer response"
    );

    let response_msg = TransportMessage::Response(json_response);

    let negotiated_version = if is_init_request {
        let version = extract_negotiated_version(&response_msg);
        update_session_after_init(state, response_session_id.as_ref(), version.clone());
        version
    } else {
        None
    };

    store_response_event(state, response_session_id.as_ref(), &response_msg).await;

    let mut response = build_response(state, response_msg, session_id);

    if let Some(sid) = &response_session_id {
        response
            .headers_mut()
            .insert(MCP_SESSION_ID, sid.parse().unwrap());
    }

    let version_to_send = compute_outbound_protocol_version(
        state,
        response_session_id.as_ref(),
        is_init_request,
        negotiated_version.as_deref(),
    );
    response
        .headers_mut()
        .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    // v2 outbound headers (VERS-05): echoed on BOTH the handler's success and its
    // structured JSON-RPC error, built without panicking. Overwrites the
    // MCP-Protocol-Version above with the v2 value for an accepted v2 request.
    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }

    response
}

/// Fast path handler without HTTP middleware.
///
/// Refactored in 75-01 Task 1a-A: extracted [`read_body_with_limit`],
/// [`parse_transport_message_fast`], and [`handle_fast_path_request`] so
/// this orchestrator is a thin early-return pipeline, sharing
/// [`extract_session_and_protocol_headers`], [`is_initialize_request`],
/// [`resolve_session_for_request`], and [`compute_outbound_protocol_version`]
/// with the middleware path.
async fn handle_post_fast_path(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;

    let body = match read_body_with_limit(body, state.config.max_request_bytes).await {
        Ok(b) => b,
        Err(response) => return response,
    };

    if let Err(error_response) = validate_headers(&headers, "POST") {
        return error_response;
    }

    let message = match parse_transport_message_fast(body.as_bytes()) {
        Ok(msg) => msg,
        Err(response) => return response,
    };

    let (session_id, protocol_version) = extract_session_and_protocol_headers(&headers);
    let is_init_request = is_initialize_request(&message);

    let response_session_id = match resolve_session_for_request(
        &state,
        is_init_request,
        session_id.clone(),
        protocol_version.clone(),
    ) {
        Ok(sid) => sid,
        Err(error_response) => return error_response,
    };

    // v2 required-header gate (VERS-05): resolve the ProtocolContext ONCE
    // (consumed by dispatch), classify the header/_meta matrix fail-closed, and
    // derive the outbound-header echo. Runs BEFORE the legacy protocol-version
    // check because an accepted v2 request carries MCP-Protocol-Version:
    // 2026-07-28, which the static-SUPPORTED check would otherwise reject. v1 /
    // non-opted-in → Passthrough (zero enforcement, D-04).
    let (protocol_context, v2_outbound) = match &message {
        TransportMessage::Request { request, .. } => {
            let (ctx, gate) = run_v2_header_gate(&state, request, &headers, body.as_bytes()).await;
            match gate {
                V2GateOutcome::Reject(code, msg) => {
                    return create_error_response(StatusCode::BAD_REQUEST, code, &msg);
                },
                V2GateOutcome::Passthrough => (ctx, None),
                V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
            }
        },
        _ => (None, None),
    };
    let is_v2_request = v2_outbound.is_some();

    // Legacy protocol-version validation applies to v1 non-init requests ONLY —
    // an accepted v2 request is validated by the gate above (D-11 untouched v1).
    if !is_init_request && !is_v2_request {
        if let Err(error_response) =
            validate_protocol_version(&state, session_id.as_ref(), protocol_version.as_ref())
        {
            return error_response;
        }
    }

    let auth_context = match extract_and_validate_auth(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match message {
        TransportMessage::Request { id, request } => {
            handle_fast_path_request(
                &state,
                id,
                request,
                auth_context,
                FastPathDispatch {
                    is_init_request,
                    response_session_id,
                    protocol_context,
                    v2_outbound,
                },
                session_id.as_ref(),
            )
            .await
        },
        TransportMessage::Notification { .. } | TransportMessage::Response(_) => {
            StatusCode::ACCEPTED.into_response()
        },
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

/// Per-request dispatch inputs threaded into the middleware-path handler.
///
/// The middleware-path twin of [`FastPathDispatch`]: carries the Plan-04-resolved
/// `ProtocolContext` (CONSUMED at dispatch, never re-resolved) and the optional
/// v2 outbound headers to echo on success AND error.
struct MiddlewareDispatch {
    is_init_request: bool,
    response_session_id: Option<String>,
    protocol_context: Option<crate::types::protocol::ProtocolContext>,
    v2_outbound: Option<(String, String)>,
}

/// Dispatch the parsed `TransportMessage` on the middleware path.
///
/// Handles `Request` (server-handled + response assembly), `Notification`
/// (202 Accepted), and `Response` (202 Accepted) in separate arms.
async fn dispatch_message_with_middleware(
    state: &ServerState,
    message: TransportMessage,
    dispatch: MiddlewareDispatch,
    auth_context: Option<crate::server::auth::AuthContext>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    let MiddlewareDispatch {
        is_init_request,
        response_session_id,
        protocol_context,
        v2_outbound,
    } = dispatch;
    match message {
        TransportMessage::Request { id, request } => {
            let json_response = {
                let server = state.server.lock().await;
                // Thread the ALREADY-RESOLVED ProtocolContext into dispatch
                // (Plan 06 / D-11): never re-resolved downstream.
                server
                    .handle_request_with_context(id, request, auth_context, protocol_context)
                    .await
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

            let mut response = build_success_response_with_middleware(
                &response_msg,
                response_session_id.as_ref(),
                &version_to_send,
                http_middleware,
                http_context,
            )
            .await;

            // v2 outbound headers on BOTH success and structured error (VERS-05).
            if let Some((method, name)) = &v2_outbound {
                apply_v2_outbound_headers(response.headers_mut(), method, name);
            }
            response
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

    // v2 required-header gate (VERS-05): resolve the ProtocolContext ONCE and
    // classify the header/_meta matrix fail-closed before dispatch. Runs BEFORE
    // the legacy protocol-version check because an accepted v2 request carries
    // MCP-Protocol-Version: 2026-07-28 (which the static-SUPPORTED check would
    // reject). Only Request messages carry a header contract; v1 / non-opted-in
    // → Passthrough (zero enforcement, D-04).
    let (protocol_context, v2_outbound) = if let TransportMessage::Request { request, .. } =
        &message
    {
        let (ctx, gate) = run_v2_header_gate(
            &state,
            request,
            &server_request.headers,
            &server_request.body,
        )
        .await;
        match gate {
            V2GateOutcome::Reject(code, msg) => {
                report_middleware_error(http_middleware, &http_context, "v2 header gate rejected")
                    .await;
                return create_error_response(StatusCode::BAD_REQUEST, code, &msg);
            },
            V2GateOutcome::Passthrough => (ctx, None),
            V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
        }
    } else {
        (None, None)
    };
    let is_v2_request = v2_outbound.is_some();

    // Legacy protocol-version validation applies to v1 non-init requests ONLY —
    // an accepted v2 request is validated by the gate above (v1 path untouched).
    if !is_v2_request {
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
    }

    let auth_context =
        match extract_auth_with_middleware(&state, &server_request, http_middleware, &http_context)
            .await
        {
            Ok(ctx) => ctx,
            Err(response) => return response,
        };

    dispatch_message_with_middleware(
        &state,
        message,
        MiddlewareDispatch {
            is_init_request,
            response_session_id,
            protocol_context,
            v2_outbound,
        },
        auth_context,
        http_middleware,
        &http_context,
    )
    .await
}

/// Handle GET requests for SSE streams
/// Resolve the SSE session ID: validate an incoming one or mint a new one.
///
/// Returns `Ok(session_id)` on success, or an error response (404 unknown
/// session, 405 stateless-mode).
fn resolve_sse_session(
    state: &ServerState,
    incoming_session_id: Option<String>,
) -> std::result::Result<String, Response> {
    if let Some(sid) = incoming_session_id {
        if state.config.session_id_generator.is_some() && !state.sessions.read().contains_key(&sid)
        {
            return Err(create_error_response(
                StatusCode::NOT_FOUND,
                -32600,
                "Unknown session ID",
            ));
        }
        return Ok(sid);
    }
    let Some(generator) = &state.config.session_id_generator else {
        return Err(create_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            -32601,
            "SSE not supported in stateless mode",
        ));
    };
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
    Ok(new_id)
}

/// Replay events from the event store after a `Last-Event-ID` header value
/// into an SSE sender channel. Fire-and-forget on any intermediate failure.
async fn replay_sse_events_from_header(
    headers: &HeaderMap,
    tx: &mpsc::UnboundedSender<TransportMessage>,
    event_store: Option<&Arc<InMemoryEventStore>>,
) {
    let Some(last_event_id) = headers.get(LAST_EVENT_ID) else {
        return;
    };
    let Ok(last_id) = last_event_id.to_str() else {
        return;
    };
    let Some(store) = event_store else {
        return;
    };
    if let Ok(events) = store.replay_events_after(last_id).await {
        for (_event_id, msg) in events {
            let _ = tx.send(msg);
        }
    }
}

/// Map a `TransportMessage` to an SSE `Event`, spawning a best-effort event
/// store write in parallel.
fn sse_event_for_message(
    msg: &TransportMessage,
    session_id: &str,
    event_store: Option<&Arc<InMemoryEventStore>>,
) -> Event {
    let event_id = Uuid::new_v4().to_string();
    if let Some(store) = event_store {
        let sid = session_id.to_string();
        let msg_clone = msg.clone();
        let store = store.clone();
        let event_id_clone = event_id.clone();
        tokio::spawn(async move {
            let _ = store.store_event(&sid, &event_id_clone, &msg_clone).await;
        });
    }
    Event::default()
        .id(event_id)
        .event("message")
        .data(serde_json::to_string(msg).unwrap())
}

/// Attach SSE-specific hardening headers (session, cache-control, connection)
/// to the given axum response.
fn attach_sse_response_headers(response: &mut Response, session_id: &str) {
    response
        .headers_mut()
        .insert(MCP_SESSION_ID, session_id.parse().unwrap());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
}

/// Handle GET requests for SSE streams.
///
/// Refactored in 75-01 Task 1a-A: extracted [`resolve_sse_session`],
/// [`replay_sse_events_from_header`], [`sse_event_for_message`], and
/// [`attach_sse_response_headers`] so this orchestrator is a short pipeline.
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(error_response) = validate_headers(&headers, "GET") {
        return error_response;
    }

    let incoming_session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let session_id = match resolve_sse_session(&state, incoming_session_id) {
        Ok(sid) => sid,
        Err(response) => return response,
    };

    if state.sse_streams.read().contains_key(&session_id) {
        return create_error_response(
            StatusCode::CONFLICT,
            -32600,
            "SSE stream already exists for this session",
        );
    }

    let (tx, rx) = mpsc::unbounded_channel();
    state
        .sse_streams
        .write()
        .insert(session_id.clone(), tx.clone());

    replay_sse_events_from_header(&headers, &tx, state.config.event_store.as_ref()).await;

    let stream = UnboundedReceiverStream::new(rx);
    let session_id_for_header = session_id.clone();
    let session_id_for_stream = session_id.clone();
    let event_store = state.config.event_store.clone();

    let sse = Sse::new(stream.map(move |msg| {
        Ok::<_, Infallible>(sse_event_for_message(
            &msg,
            &session_id_for_stream,
            event_store.as_ref(),
        ))
    }));

    let mut response = sse.into_response();
    attach_sse_response_headers(&mut response, &session_id_for_header);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_custom_claim_header_inserted_under_cognito_key() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "user-123".parse().unwrap());
        h.insert(
            "x-pmcp-claim-custom-primary-creator",
            "rosen".parse().unwrap(),
        );
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(
            ctx.claims.get("custom:primary_creator"),
            Some(&serde_json::Value::String("rosen".into())),
        );
    }

    #[test]
    // Why: spec sdk-issue-pmcp-claim-custom-extraction.md line 112 pins
    // this assertion byte-identically; clippy::unnecessary_get_then_check
    // would rewrite to !contains_key(...) which is semantically equivalent
    // but breaks the cross-repo verbatim invariant.
    #[allow(clippy::unnecessary_get_then_check)]
    fn extract_custom_claim_empty_value_dropped() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "user-123".parse().unwrap());
        h.insert("x-pmcp-claim-custom-empty", "".parse().unwrap());
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert!(ctx.claims.get("custom:empty").is_none());
    }

    #[test]
    fn extract_custom_claim_kebab_to_snake() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "u".parse().unwrap());
        h.insert(
            "x-pmcp-claim-custom-promo-code",
            "SUMMER25".parse().unwrap(),
        );
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(
            ctx.claims.get("custom:promo_code"),
            Some(&serde_json::Value::String("SUMMER25".into())),
        );
    }

    #[test]
    fn extract_custom_claim_coexists_with_standard_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "u".parse().unwrap());
        h.insert("x-pmcp-user-email", "u@example.com".parse().unwrap());
        h.insert("x-pmcp-user-groups", "g1,g2".parse().unwrap());
        h.insert("x-pmcp-claim-custom-tier", "gold".parse().unwrap());
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(ctx.subject, "u");
        assert_eq!(ctx.claims["email"], "u@example.com");
        assert_eq!(ctx.claims["custom:tier"], "gold");
    }

    // ======================================================================
    // v2 required-header classifier (Plan 112-06, VERS-05 / D-05 / D-06).
    // Unit + property coverage of the PURE, non-panicking gate helpers.
    // ======================================================================

    use crate::types::protocol::error_codes::INVALID_REQUEST;
    use crate::types::protocol::PROTOCOL_VERSION_2026_07_28 as V2;

    /// Build a `HeaderMap` from `(name, value)` pairs for classifier tests.
    fn headers_from(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            let name = http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            h.insert(name, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn decode_version_header_classifies_each_kind() {
        assert_eq!(
            decode_version_header(&headers_from(&[])),
            HeaderProtocolVersion::Absent
        );
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, V2)])),
            HeaderProtocolVersion::V2
        );
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, "2025-11-25")])),
            HeaderProtocolVersion::Other
        );
        // Oversized value → Malformed, never a panic.
        let big = "x".repeat(MAX_V2_HEADER_VALUE_LEN + 1);
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, &big)])),
            HeaderProtocolVersion::Malformed
        );
    }

    #[test]
    fn classify_era_cell_covers_every_matrix_cell() {
        // v2/v2 → enforce
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::V2, true),
            V2Classification::Enforce
        ));
        // v1/v1 → legacy
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Other, false),
            V2Classification::Legacy
        ));
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Absent, false),
            V2Classification::Legacy
        ));
        // v2-header / non-v2-meta → reject
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::V2, false),
            V2Classification::Reject(INVALID_REQUEST, _)
        ));
        // non-v2-header / v2-meta → reject
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Absent, true),
            V2Classification::Reject(INVALID_REQUEST, _)
        ));
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Malformed, true),
            V2Classification::Reject(INVALID_REQUEST, _)
        ));
    }

    #[test]
    fn require_three_headers_needs_all_three() {
        // All three present → Ok
        let ok = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, "search"),
        ]);
        assert_eq!(
            require_three_headers(&ok).unwrap(),
            ("tools/call".to_string(), "search".to_string())
        );
        // Missing Mcp-Name → Err
        let missing = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, "tools/call")]);
        assert!(require_three_headers(&missing).is_err());
    }

    #[test]
    fn cross_check_method_and_name_fail_closed() {
        assert!(cross_check_method("tools/call", Some("tools/call")).is_ok());
        assert!(cross_check_method("tools/call", Some("resources/read")).is_err());
        assert!(cross_check_method("tools/call", None).is_err());

        // name-bearing: must match params.name
        assert!(cross_check_name("search", "tools/call", Some("search")).is_ok());
        assert!(cross_check_name("search", "tools/call", Some("other")).is_err());
        assert!(cross_check_name("search", "tools/call", None).is_err());
        // name-less method: presence-only, body name irrelevant
        assert!(cross_check_name("anything", "tools/list", None).is_ok());
    }

    #[test]
    fn classify_v2_request_accepts_well_formed_v2() {
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, "search"),
        ]);
        let out = classify_v2_request(&h, true, Some("tools/call"), Some("search"));
        assert!(matches!(out, V2GateOutcome::EnforceOk { .. }));
    }

    #[test]
    fn classify_v2_request_rejects_method_body_mismatch() {
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, "search"),
        ]);
        // body method disagrees with Mcp-Method (smuggling)
        let out = classify_v2_request(&h, true, Some("resources/read"), Some("search"));
        assert!(matches!(out, V2GateOutcome::Reject(INVALID_REQUEST, _)));
    }

    #[test]
    fn extract_body_method_and_name_reads_wire_shape() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("tools/call"));
        assert_eq!(n.as_deref(), Some("search"));
        // Garbage bytes → (None, None), never a panic.
        assert_eq!(extract_body_method_and_name(b"not json"), (None, None));
    }

    #[test]
    fn apply_v2_outbound_headers_sets_all_three_without_panic() {
        let mut h = HeaderMap::new();
        apply_v2_outbound_headers(&mut h, "tools/call", "search");
        assert_eq!(h.get(MCP_METHOD).unwrap(), "tools/call");
        assert_eq!(h.get(MCP_NAME).unwrap(), "search");
        assert_eq!(h.get(MCP_PROTOCOL_VERSION).unwrap(), V2);
    }

    proptest::proptest! {
        /// The classifier NEVER panics over arbitrary header bytes + signal
        /// combinations, and holds the accept/reject invariants (T-112-13).
        #[test]
        fn v2_header_gate_proptest(
            header_kind in 0u8..4,
            meta_is_v2 in proptest::bool::ANY,
            have_method in proptest::bool::ANY,
            have_name in proptest::bool::ANY,
            method_val in "[a-z/]{0,20}",
            name_val in "[a-z]{0,20}",
            body_method in proptest::option::of("[a-z/]{0,20}"),
            body_name in proptest::option::of("[a-z]{0,20}"),
        ) {
            let mut pairs: Vec<(&str, String)> = Vec::new();
            match header_kind {
                0 => {}, // absent
                1 => pairs.push((MCP_PROTOCOL_VERSION, V2.to_string())),
                2 => pairs.push((MCP_PROTOCOL_VERSION, "2025-11-25".to_string())),
                _ => pairs.push((MCP_PROTOCOL_VERSION, "\u{ff}bogus".to_string())),
            }
            if have_method {
                pairs.push((MCP_METHOD, method_val.clone()));
            }
            if have_name {
                pairs.push((MCP_NAME, name_val.clone()));
            }
            let mut h = HeaderMap::new();
            for (k, v) in &pairs {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    let name = http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
                    h.insert(name, hv);
                }
            }

            // Must not panic.
            let out = classify_v2_request(&h, meta_is_v2, body_method.as_deref(), body_name.as_deref());

            let header_is_v2 = decode_version_header(&h) == HeaderProtocolVersion::V2;
            match out {
                V2GateOutcome::Passthrough => {
                    // Only when neither signal is v2.
                    proptest::prop_assert!(!header_is_v2 && !meta_is_v2);
                },
                V2GateOutcome::EnforceOk { .. } => {
                    // Only when BOTH signals are v2 AND all three headers present.
                    proptest::prop_assert!(header_is_v2 && meta_is_v2);
                    proptest::prop_assert!(have_method && have_name);
                },
                V2GateOutcome::Reject(code, _) => {
                    proptest::prop_assert_eq!(code, INVALID_REQUEST);
                },
            }
        }
    }
}
