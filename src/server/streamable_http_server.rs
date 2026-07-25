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

// ---------------------------------------------------------------------------
// Session era gate (Plan 113-04, HTTP-01).
//
// `stateless()` is a BUILD-TIME config: it clears `session_id_generator` once,
// when the server is constructed. A dual-version server is built with
// `Default::default()`, which keeps a live generator — so every session decision
// that keys off the CONFIG would mint, demand and echo session ids for v2
// requests too (RESEARCH Pitfall 1). HTTP-01 requires the opposite: on v2 there
// is no handshake and no session at all.
//
// The fix is one predicate, not a transport fork. Every session decision routes
// through `sessions_active`, which makes the ERA the decider and leaves the v1
// path byte-for-byte unchanged.
// ---------------------------------------------------------------------------

/// The pure session-era rule: are sessions live for THIS request?
///
/// | `cfg_has_generator` | `era`            | result | why |
/// |---------------------|------------------|--------|-----|
/// | `true`              | `Some(Era::V2)`  | `false`| v2 is handshake-free and session-free (HTTP-01) |
/// | `true`              | `Some(Era::V1)`  | `true` | v1 session behavior is untouched |
/// | `true`              | `None`           | `true` | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `false`             | anything         | `false`| an explicitly `stateless()` server stays stateless |
///
/// Split out from [`sessions_active`] so the RULE is unit- and property-testable
/// without constructing a live [`ServerState`].
const fn sessions_active_for(
    cfg_has_generator: bool,
    era: Option<crate::types::protocol::Era>,
) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2)) && cfg_has_generator
}

/// Are sessions live for this request? THE single reader of
/// `config.session_id_generator`'s presence.
///
/// `era` is the ALREADY-RESOLVED [`ProtocolContext::era`](crate::types::protocol::ProtocolContext)
/// being CONSUMED here — this layer never runs a second era resolver (Pitfall 2 /
/// D-11). The POST entrypoints resolve it once via the v2 header gate and thread
/// that same value into every session decision below.
///
/// `None` means the server is NOT opted into v2, so no era detection ran at all
/// and the v1 path executes with zero era code (D-04).
fn sessions_active(state: &ServerState, era: Option<crate::types::protocol::Era>) -> bool {
    sessions_active_for(state.config.session_id_generator.is_some(), era)
}

/// The session-id generator to use for THIS request, or `None` when sessions are
/// not active for it.
///
/// The second (and last) permitted reader of `config.session_id_generator`: it
/// gates the borrow behind [`sessions_active`] so no caller can reach the
/// generator on a request whose era suppresses sessions.
fn active_session_generator(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
) -> Option<&(dyn Fn() -> String + Send + Sync)> {
    if !sessions_active(state, era) {
        return None;
    }
    state.config.session_id_generator.as_deref()
}

/// The ONE place a `Mcp-Session-Id` response header is emitted.
///
/// `response_session_id` is already `None` for a v2 request (both session
/// resolvers return `None` when [`sessions_active`] is false), so this is
/// defense in depth: even a future caller that manufactured a session id could
/// not leak it onto a v2 response. Non-panicking — an unrepresentable id is
/// skipped rather than unwrapped (T-112-13 discipline).
fn apply_session_header(
    headers: &mut HeaderMap,
    response_session_id: Option<&String>,
    sessions_on: bool,
) {
    if !sessions_on {
        return;
    }
    let Some(sid) = response_session_id else {
        return;
    };
    if let Ok(value) = HeaderValue::from_str(sid) {
        headers.insert(MCP_SESSION_ID, value);
    }
}

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

// ---------------------------------------------------------------------------
// v2 HTTP status mapping (Plan 113-04, HTTP-01).
//
// The transport spec turns several JSON-RPC error codes into specific HTTP
// statuses on the v2 path — most notably "If the server does not implement the
// requested RPC method, it MUST respond with 404 Not Found and a JSON-RPC error
// with code -32601", which pmcp answered at HTTP 200 (v1 behavior) before this
// plan.
//
// The mapper below is CODE-driven, never call-site-driven: -32021 is emitted by
// dispatch (plan 09), not by the header gate, and a code that reaches the wire
// from anywhere must map identically. It is also era-gated: on v1 / a
// non-opted-in server every status is exactly what it was before.
// ---------------------------------------------------------------------------

/// The HTTP status the v2 transport requires for a JSON-RPC error `code`.
///
/// Values come from the centralized table (VERS-06); the per-constant rustdoc in
/// `error_codes.rs` is the single documented source for each mapping. Anything
/// not listed is handler semantics rather than a transport-layer rejection and
/// stays at HTTP 200 with the JSON-RPC error in the body.
fn v2_status_for_code(code: i32) -> StatusCode {
    use crate::types::protocol::error_codes as ec;
    match code {
        ec::METHOD_NOT_FOUND => StatusCode::NOT_FOUND,
        ec::HEADER_MISMATCH
        | ec::MISSING_REQUIRED_CLIENT_CAPABILITY
        | ec::UNSUPPORTED_PROTOCOL_VERSION
        | ec::PARSE_ERROR
        | ec::INVALID_REQUEST
        | ec::INVALID_PARAMS => StatusCode::BAD_REQUEST,
        _ => StatusCode::OK,
    }
}

/// Era-gated status for an error `code`: v2 uses [`v2_status_for_code`], every
/// other era keeps `v1_status` byte-for-byte.
fn status_for_error(
    era: Option<crate::types::protocol::Era>,
    code: i32,
    v1_status: StatusCode,
) -> StatusCode {
    if matches!(era, Some(crate::types::protocol::Era::V2)) {
        v2_status_for_code(code)
    } else {
        v1_status
    }
}

/// The JSON-RPC `id` of a raw request body, or `Null` when it has none.
///
/// Used so a v2 error envelope built BEFORE (or INSTEAD OF) a successful typed
/// parse still carries the ORIGINAL request id — HTTP-05 depends on it and plan
/// 08 asserts it. Never panics on adversarial input.
fn raw_request_id(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(serde_json::Value::Null)
}

/// Build a JSON-RPC error response with an explicit id and optional structured
/// `data`.
///
/// The v2 counterpart of [`create_error_response`], which hardcodes `id: null`.
/// Kept separate so no v1 response byte changes: only v2 paths call this.
fn create_error_response_with_id(
    status: StatusCode,
    id: serde_json::Value,
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> Response {
    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("message".to_string(), json!(message));
    if let Some(data) = data {
        error.insert("data".to_string(), data);
    }
    // Built through a `Map` rather than the `json!` macro because the macro
    // BORROWS its interpolated values, which would leave `id` passed-by-value
    // but never consumed.
    let mut body = serde_json::Map::new();
    body.insert("jsonrpc".to_string(), json!("2.0"));
    body.insert("error".to_string(), serde_json::Value::Object(error));
    body.insert("id".to_string(), id);
    (status, Json(serde_json::Value::Object(body))).into_response()
}

/// Re-map a pre-dispatch parse rejection onto the v2 status table.
///
/// The typed parse is where an UNKNOWN METHOD surfaces: `parse_request_or_internal`
/// answers `Error::method_not_found` for any method string that matches no
/// `ClientRequest` / `ServerRequest` variant, which the transport stringifies into
/// an "Invalid request" parse failure. On v1 that has always been HTTP 400 with
/// `-32700` and `id: null`, and it stays exactly that.
///
/// On v2 the spec is explicit: "If the server does not implement the requested RPC
/// method, it MUST respond with `404 Not Found` and a JSON-RPC error with code
/// `-32601`." A body whose method never deserializes therefore cannot be diagnosed
/// from an already-built TYPED response — this mapping has to happen at the RAW
/// level, from the body bytes, which is what this function does.
///
/// The era is resolved from the RAW `params._meta` (the same read the
/// `server/discover` ingress uses) because no typed request exists to read it
/// from. A body that is not a well-formed JSON-RPC request, or a server that is
/// not opted into v2, or a v1 request, all keep `v1_response` untouched.
///
/// KNOWN LIMITATION: a KNOWN method whose params fail to deserialize also reaches
/// `method_not_found` at this seam and is therefore reported as `-32601`/404 on
/// v2 rather than `-32602`/400. Distinguishing the two requires a method-string
/// table this layer does not own; plan 06 (MRTR param parse errors) adds the
/// precise per-parameter mapping.
async fn map_unparsed_body_for_v2(
    state: &ServerState,
    raw_body: &[u8],
    v1_response: Response,
) -> Response {
    use crate::types::protocol::error_codes::METHOD_NOT_FOUND;
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(raw_body) else {
        return v1_response;
    };
    // Only a well-formed JSON-RPC REQUEST (method + id) can be an unknown-method
    // rejection; anything else keeps the v1 parse-error response.
    let Some(method) = envelope.get("method").and_then(serde_json::Value::as_str) else {
        return v1_response;
    };
    if envelope.get("id").is_none() {
        return v1_response;
    }
    let raw_meta = envelope.get("params").and_then(|p| p.get("_meta")).cloned();
    let resolved = {
        let server = state.server.lock().await;
        server.resolve_discover_protocol_context(raw_meta.as_ref())
    };
    let Ok(Some(context)) = resolved else {
        return v1_response;
    };
    if context.era != crate::types::protocol::Era::V2 {
        return v1_response;
    }
    create_error_response_with_id(
        v2_status_for_code(METHOD_NOT_FOUND),
        envelope
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        METHOD_NOT_FOUND,
        &format!("Method not found: {method}"),
        None,
    )
}

/// The v2 status a built JSON-RPC response must carry, or `None` to keep the
/// status the response already has.
///
/// This is the CODE-driven half of the mapper: `MISSING_REQUIRED_CLIENT_CAPABILITY`
/// (-32021) is emitted by dispatch (plan 09), not by the header gate, so the
/// mapping cannot be attached at rejection call sites — it has to read the code
/// that is actually about to reach the wire.
fn v2_dispatch_response_status(
    era: Option<crate::types::protocol::Era>,
    response: &crate::types::JSONRPCResponse,
) -> Option<StatusCode> {
    if !matches!(era, Some(crate::types::protocol::Era::V2)) {
        return None;
    }
    let crate::types::jsonrpc::ResponsePayload::Error(ref error) = response.payload else {
        return None;
    };
    Some(v2_status_for_code(error.code))
}

/// Assemble the response for a [`V2GateOutcome::Reject`].
///
/// The status is code-driven via [`status_for_error`] with a `400` v1 floor (the
/// gate only rejects requests that already carry a v2 signal on one side, and
/// `400` is what Phase 112 returned for every such cell). The id is recovered
/// from the RAW body so a rejection that happened before — or instead of — a
/// successful typed parse still echoes the client's id.
fn v2_gate_reject_response(
    raw_body: &[u8],
    era: Option<crate::types::protocol::Era>,
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> Response {
    let status = status_for_error(era, code, StatusCode::BAD_REQUEST);
    create_error_response_with_id(status, raw_request_id(raw_body), code, message, data)
}

/// Outcome of the whole v2 gate for one request.
enum V2GateOutcome {
    /// Not a v2 request (v1 / non-opted-in) — dispatch normally, no v2 headers.
    Passthrough,
    /// Accepted v2 request — dispatch, then echo these headers outbound.
    EnforceOk { method: String, name: String },
    /// Rejected — build a 4xx structured JSON-RPC error with this code/message
    /// and, when the code defines one, a structured `error.data` payload.
    ///
    /// `data` is not optional decoration: `UNSUPPORTED_PROTOCOL_VERSION`
    /// (`-32022`) MUST carry a `supported` array so the client can pick a
    /// mutually supported version instead of probing, and
    /// `MISSING_REQUIRED_CLIENT_CAPABILITY` (`-32021`, emitted by dispatch in
    /// plan 09) MUST carry an object-shaped `requiredCapabilities`. A
    /// `(code, message)` pair alone cannot express either.
    Reject {
        code: i32,
        message: String,
        data: Option<serde_json::Value>,
    },
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
        // Both conflict cells are a HEADER/BODY DISAGREEMENT, which is exactly
        // what the spec allocates `HEADER_MISMATCH` (-32020) for. Before Phase
        // 113 these emitted the generic `INVALID_REQUEST` (-32600) because the
        // v2 code did not exist yet.
        (true, false) => V2Classification::Reject(
            crate::types::protocol::error_codes::HEADER_MISMATCH,
            "MCP-Protocol-Version header claims v2 but _meta protocolVersion disagrees",
        ),
        (false, true) => V2Classification::Reject(
            crate::types::protocol::error_codes::HEADER_MISMATCH,
            "_meta claims v2 but MCP-Protocol-Version header is absent or not 2026-07-28",
        ),
    }
}

/// Require all THREE v2 headers (VERS-05 / D-05); return `(method, name)`.
///
/// # The `Mcp-Name` header rule (locked cross-plan contract)
///
/// > `Mcp-Name` MUST be PRESENT on every v2 request. Its VALUE is cross-checked
/// > against the request's logical name only for the name-bearing methods
/// > (`tools/call`, `prompts/get` → `params.name`; `resources/read` →
/// > `params.uri`). For every other v2 method the value is the EMPTY STRING and
/// > is not cross-checked.
///
/// Verbatim from `113-SPEC-RECHECK.md` § `Mcp-Name Header Rule`, and locked by
/// Phase-112 D-05. This function enforces the PRESENCE half (an absent header is
/// a rejection even when the value would be empty); [`cross_check_name`] enforces
/// the VALUE half and returns `Ok` immediately for a non-name-bearing method.
///
/// The draft transport spec requires the header only for the three name-bearing
/// methods. pmcp deliberately keeps the stricter, fail-closed rule (Phase-113
/// DRIFT-1 adjudication): a header a WAF can rely on being present on every
/// request is worth more than matching the laxer wording, and plan 05's client
/// emits exactly this — `Mcp-Name: ""` for a name-less method.
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

/// Methods whose logical name must be cross-checked against `Mcp-Name` (D-06).
///
/// The name-bearing set and the "where the logical name lives" map are ONE table,
/// [`crate::types::mrtr::logical_name_key`] — shared with the client emitter
/// (plan 05) so the two ends can never disagree about which methods carry a name
/// or which params key holds it.
fn is_name_bearing_method(method: &str) -> bool {
    crate::types::mrtr::logical_name_key(method).is_some()
}

/// Cross-check `Mcp-Name` against the request's logical name for name-bearing
/// methods (D-06). Name-less methods are presence-only (enforced upstream by
/// [`require_three_headers`]).
///
/// # The sentinel decode is load-bearing
///
/// A logical name that is not header-safe (non-ASCII, or containing an RFC 9110
/// field-value delimiter) MUST travel in the `=?base64?<b64>?=` sentinel form. A
/// verbatim comparison would therefore reject a legitimate conformant request, so
/// the header value is decoded through the SHARED codec
/// [`crate::types::mrtr::decode_header_value`] — the same one the client emitter
/// uses — before it is compared. A value that starts the sentinel but does not
/// decode is a malformed header, i.e. a `HEADER_MISMATCH` rejection, never a
/// silent pass.
fn cross_check_name(
    mcp_name: &str,
    method: &str,
    body_name: Option<&str>,
) -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) {
        return Ok(());
    }
    let Some(decoded) = crate::types::mrtr::decode_header_value(mcp_name) else {
        return Err("Mcp-Name header is a malformed =?base64?...?= sentinel value");
    };
    match body_name {
        Some(bn) if bn == decoded => Ok(()),
        _ => Err("Mcp-Name header does not match the request's logical name"),
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
    use crate::types::protocol::error_codes::HEADER_MISMATCH;
    // Every rejection this classifier can produce is a missing-required-header
    // or a header/body mismatch, so they all carry `HEADER_MISMATCH` and no
    // structured `data`.
    let reject = |msg: &str| V2GateOutcome::Reject {
        code: HEADER_MISMATCH,
        message: msg.to_string(),
        data: None,
    };
    let header = decode_version_header(headers);
    match classify_era_cell(header, meta_is_v2) {
        V2Classification::Legacy => V2GateOutcome::Passthrough,
        V2Classification::Reject(code, msg) => V2GateOutcome::Reject {
            code,
            message: msg.to_string(),
            data: None,
        },
        V2Classification::Enforce => {
            let (method, name) = match require_three_headers(headers) {
                Ok(pair) => pair,
                Err(msg) => return reject(msg),
            };
            if let Err(msg) = cross_check_method(&method, body_method) {
                return reject(msg);
            }
            if let Err(msg) = cross_check_name(&name, &method, body_name) {
                return reject(msg);
            }
            V2GateOutcome::EnforceOk { method, name }
        },
    }
}

/// Extract the untrusted `(method, logical-name)` pair from the raw JSON-RPC body.
///
/// Re-parses the raw bytes (the transport parse already succeeded) so the
/// cross-check compares the header against the LITERAL wire value a WAF would see
/// — the smuggling-relevant view (D-06). Never panics.
///
/// The logical name is resolved METHOD-AWARELY because different name-bearing
/// methods carry it in different params keys:
/// - `tools/call` → `params.name`
/// - `prompts/get` → `params.name`
/// - `resources/read` → `params.uri` (a [`ReadResourceRequest`](crate::types::ReadResourceRequest)
///   has a `uri` field and NO `name` field, so reading `params.name` would always
///   yield `None` and wrongly reject a standards-shaped `resources/read`)
/// - any other method → `None` (presence-only; `cross_check_name` returns Ok for
///   non-name-bearing methods)
fn extract_body_method_and_name(body: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return (None, None);
    };
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    // The logical-name params key is method-derived via the single `logical_name_key`
    // table; non-name-bearing methods yield `None` (presence-only cross-check).
    let name = method
        .as_deref()
        .and_then(crate::types::mrtr::logical_name_key)
        .and_then(|name_key| value.get("params").and_then(|p| p.get(name_key)))
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

/// Map a per-request version-negotiation failure to a structured gate rejection.
///
/// An UNSUPPORTED version is the spec's `UNSUPPORTED_PROTOCOL_VERSION` (-32022),
/// and its `error.data` MUST list the versions the server DOES accept so the
/// client can pick a mutually supported one and retry rather than probe. A
/// MALFORMED reserved `_meta` key is a bad method parameter, so it keeps the
/// `INVALID_PARAMS` mapping the shared dispatch resolver uses.
fn negotiation_error_to_gate_reject(
    error: &crate::types::protocol::context::ProtocolNegotiationError,
    accept_list: &[crate::types::ProtocolVersion],
) -> V2GateOutcome {
    use crate::types::protocol::context::ProtocolNegotiationError;
    use crate::types::protocol::error_codes::UNSUPPORTED_PROTOCOL_VERSION;
    match error {
        ProtocolNegotiationError::UnsupportedVersion(requested) => {
            let supported: Vec<&str> = accept_list.iter().map(|v| v.as_str()).collect();
            V2GateOutcome::Reject {
                code: UNSUPPORTED_PROTOCOL_VERSION,
                message: format!("Unsupported protocol version: {requested}"),
                data: Some(json!({ "requested": requested, "supported": supported })),
            }
        },
        ProtocolNegotiationError::MalformedMeta(_) => {
            let (code, message) = crate::server::core::negotiation_error_to_rejection(error);
            V2GateOutcome::Reject {
                code,
                message,
                data: None,
            }
        },
    }
}

/// Shared tail of BOTH v2 gates: map a resolved-context result to a
/// `(ProtocolContext, V2GateOutcome)` pair via the SAME error-mapping, D-04
/// passthrough ordering, and `classify_v2_request` matrix.
///
/// The typed and raw gates differ ONLY in how they produce `resolved` and in
/// `body_method_override` (a discover ingress pins `Some("server/discover")`;
/// the typed path passes `None` to read the method from the wire body). Keeping
/// the tail here means those two paths can never drift on the reject mapping,
/// passthrough short-circuit, or cross-check wiring.
fn finish_v2_gate(
    resolved: std::result::Result<
        Option<crate::types::protocol::ProtocolContext>,
        crate::types::protocol::context::ProtocolNegotiationError,
    >,
    accept_list: &[crate::types::ProtocolVersion],
    headers: &HeaderMap,
    raw_body: &[u8],
    body_method_override: Option<&str>,
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    let context = match resolved {
        Ok(ctx) => ctx,
        Err(err) => return (None, negotiation_error_to_gate_reject(&err, accept_list)),
    };
    // `Ok(None)` == not opted in → zero enforcement (D-04).
    let Some(ref pc) = context else {
        return (context.clone(), V2GateOutcome::Passthrough);
    };
    let meta_is_v2 = pc.era == crate::types::protocol::Era::V2;
    let (extracted_method, body_name) = extract_body_method_and_name(raw_body);
    let body_method = body_method_override.or(extracted_method.as_deref());
    let outcome = classify_v2_request(headers, meta_is_v2, body_method, body_name.as_deref());
    (context, outcome)
}

/// Run the v2 gate for a parsed `Request` on an opted-in server.
async fn run_v2_header_gate(
    state: &ServerState,
    request: &Request,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    let (resolved, accept_list) = {
        let server = state.server.lock().await;
        // Non-opted-in servers run ZERO era-detection — v1 path byte-for-byte
        // unchanged (D-04). `resolve_ingress_protocol_context` already
        // short-circuits to `Ok(None)` there.
        (
            server.resolve_ingress_protocol_context(request),
            server.supported_protocol_versions().to_vec(),
        )
    };
    finish_v2_gate(resolved, &accept_list, headers, raw_body, None)
}

/// Crate-LOCAL ingress classification for the POST pipeline (Phase 112, VERS-04).
///
/// This is NOT the public [`TransportMessage`] enum — it never adds a variant to
/// that semver-sensitive type. It only distinguishes an internally-routed
/// `server/discover` request (which has no public enum variant) from every other
/// message, so both flow through the SAME POST stages (session → v2 header matrix
/// → legacy-version → auth → dispatch → event store → response assembly) and
/// `server/discover` is routed only at the final per-path response-assembly step
/// (the classify-then-continue design — no pipeline bypass).
enum HttpIngress {
    /// Any normal message (typed request, notification, or response) — the
    /// existing public-enum dispatch path, unchanged.
    Public(TransportMessage),
    /// A v2-only `server/discover` request carrying the ORIGINAL request id and
    /// the RAW `_meta` value (for the raw-_meta v2 header gate + era resolution).
    Discover {
        id: crate::types::RequestId,
        raw_meta: Option<serde_json::Value>,
    },
}

/// Classify a raw POST body as an internally-routed `server/discover` request,
/// if it is one (Phase 112, VERS-04). Never panics (T-112-13).
///
/// Returns `Some(HttpIngress::Discover{..})` ONLY when the raw body is a
/// well-formed single JSON-RPC request whose method classifies — via the shared
/// [`parse_request_or_internal`](crate::shared::protocol_helpers::parse_request_or_internal)
/// seam — as [`InternalClientRequest::ServerDiscover`](crate::types::protocol::InternalClientRequest).
/// Every other input (malformed JSON, a batch/notification with no `id`, a
/// non-object, or any other method) returns `None`, so the caller falls through
/// to the existing public parse path with byte-identical behavior.
fn classify_http_ingress(body: &[u8]) -> Option<HttpIngress> {
    let req: crate::types::JSONRPCRequest<serde_json::Value> = serde_json::from_slice(body).ok()?;
    // Fast reject: `server/discover` is the only internally-routed method, so for
    // ~100% of traffic we skip the typed `parse_client_request` conversion and the
    // `_meta` clone below. `parse_request_or_internal` remains the authority for
    // the discover case (its `IngressRequest::Internal(ServerDiscover)` arm is the
    // only path that yields `Discover`), so this peek changes no classification —
    // any non-discover method returned `None` before too, via `Public(_) => None`.
    if req.method != "server/discover" {
        return None;
    }
    // Capture the raw `_meta` BEFORE the routing seam consumes `params`.
    let raw_meta = req.params.as_ref().and_then(|p| p.get("_meta")).cloned();
    let (id, ingress) = crate::shared::protocol_helpers::parse_request_or_internal(req).ok()?;
    match ingress {
        // The inner match is exhaustive over `InternalClientRequest`, so adding a
        // future internally-routed method is a compile-time tripwire here.
        crate::shared::protocol_helpers::IngressRequest::Internal(internal) => match internal {
            crate::types::protocol::InternalClientRequest::ServerDiscover(_) => {
                Some(HttpIngress::Discover { id, raw_meta })
            },
        },
        // A public request re-parsed here is DISCARDED; the caller re-parses it via
        // the existing `StdioTransport::parse_message` path so all non-discover
        // bytes (incl. parse-error responses) stay exactly as before.
        crate::shared::protocol_helpers::IngressRequest::Public(_) => None,
    }
}

/// Run the v2 header gate for a `server/discover` ingress from its RAW `_meta`
/// (Phase 112, VERS-04, findings #1/#3).
///
/// The raw-_meta counterpart of [`run_v2_header_gate`]: a discover ingress has NO
/// parsed [`Request`], so the era is resolved from `params._meta` via
/// [`Server::resolve_discover_protocol_context`](crate::server::Server::resolve_discover_protocol_context).
/// It runs the SAME [`classify_v2_request`] matrix every other method runs, with
/// `body_method` fixed to `"server/discover"`. A non-opted-in server
/// short-circuits to [`V2GateOutcome::Passthrough`] FIRST, WITHOUT inspecting the
/// v2 `_meta` (D-04 ordering).
async fn run_v2_header_gate_raw(
    state: &ServerState,
    raw_meta: Option<&serde_json::Value>,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    let (resolved, accept_list) = {
        let server = state.server.lock().await;
        // Non-opted-in → `Ok(None)` WITHOUT touching the v2 `_meta` (D-04).
        (
            server.resolve_discover_protocol_context(raw_meta),
            server.supported_protocol_versions().to_vec(),
        )
    };
    // `server/discover` is NOT a name-bearing method, so `Mcp-Name` is
    // presence-only; the body method is FIXED (override) so the header/body
    // cross-check pins `Mcp-Method` regardless of the raw body's method field.
    finish_v2_gate(
        resolved,
        &accept_list,
        headers,
        raw_body,
        Some("server/discover"),
    )
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

/// Reject a v2 `GET` / `DELETE` with `405 Method Not Allowed`, or `None` to let
/// the existing v1 handler run.
///
/// Spec, verbatim: "HTTP GET or DELETE to the MCP endpoint: respond with
/// `405 Method Not Allowed`." Neither verb carries a body, so `_meta` is
/// unavailable and the ONLY era signal is the `MCP-Protocol-Version` header —
/// read through the existing non-panicking [`decode_version_header`], so an
/// oversized or non-UTF-8 value classifies as `Malformed` (v1 behavior) rather
/// than 405.
///
/// pmcp is dual-version, so the routes STAY registered: every other header value
/// reaches today's handler unchanged. The guard runs BEFORE header validation and
/// before session validation, so a v2 GET never touches session state or the
/// event store (T-113-18).
fn v2_method_not_allowed(headers: &HeaderMap, verb: &str) -> Option<Response> {
    if !matches!(decode_version_header(headers), HeaderProtocolVersion::V2) {
        return None;
    }
    Some(create_error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        &format!("HTTP {verb} is not supported on the MCP endpoint for protocol 2026-07-28"),
    ))
}

/// Validate `Content-Type: application/json` for POST.
fn validate_content_type_json(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Content-Type header is required",
        ));
    };
    let ct = content_type.to_str().unwrap_or("");
    if !ct.contains(APPLICATION_JSON) {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            crate::types::protocol::error_codes::PARSE_ERROR,
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
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header is required",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(APPLICATION_JSON) && !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
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
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header is required for SSE",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
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
///
/// `era` is the resolved per-request era (see [`sessions_active`]). A v2 request
/// never reaches `initialize` — v2 has no handshake — but the site is defensive:
/// with sessions inactive it mints nothing.
fn process_init_session(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    if let Some(generator) = active_session_generator(state, era) {
        // Stateful mode
        if let Some(sid) = session_id {
            // Check if session already exists and is initialized
            if let Some(session_info) = state.sessions.read().get(&sid) {
                if session_info.initialized {
                    // Session already initialized - reject re-initialization
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        crate::types::protocol::error_codes::INVALID_REQUEST,
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
        // Sessions inactive (stateless config, or a v2 request) — mint nothing.
        Ok((None, false))
    }
}

/// Validate session for non-initialization request.
///
/// When sessions are inactive for this request — a `stateless()` server, or ANY
/// v2 request regardless of config — nothing is required and nothing is
/// validated. An inbound `Mcp-Session-Id` on a v2 request is IGNORED rather than
/// rejected, per the transport spec: "An `Mcp-Session-Id` header on a request:
/// ignore it, and do not mint or echo session IDs."
fn validate_non_init_session(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if sessions_active(state, era) {
        // Stateful mode - require and validate session ID
        match session_id {
            None => {
                // Missing session ID
                Err(create_error_response(
                    StatusCode::BAD_REQUEST,
                    crate::types::protocol::error_codes::INVALID_REQUEST,
                    "Session ID required for non-initialization requests",
                ))
            },
            Some(sid) => {
                // Validate session exists
                if !state.sessions.read().contains_key(&sid) {
                    // Unknown session ID
                    Err(create_error_response(
                        StatusCode::NOT_FOUND,
                        crate::types::protocol::error_codes::INVALID_REQUEST,
                        "Unknown session ID",
                    ))
                } else {
                    Ok(Some(sid))
                }
            },
        }
    } else {
        // Sessions inactive (stateless config, or a v2 request) — any inbound
        // `Mcp-Session-Id` is ignored, and none is echoed back.
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
            crate::types::protocol::error_codes::INTERNAL_ERROR,
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
            crate::types::protocol::error_codes::INTERNAL_ERROR,
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
        crate::types::protocol::error_codes::INVALID_REQUEST,
        &format!("Unsupported protocol version: {}", version),
    ))
}

/// In stateful mode, verify that a provided protocol version matches the
/// session's recorded negotiated version (if any). Pure early-return chain.
///
/// Short-circuits `Ok(())` whenever sessions are inactive for this request. On v2
/// that is not merely an optimization: there IS no session, and the PER-REQUEST
/// version is authoritative over any session state (the Phase-112 lock), so a
/// session-recorded version must never be consulted.
fn validate_protocol_version_matches_session(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if !sessions_active(state, era) {
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
        crate::types::protocol::error_codes::INVALID_REQUEST,
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
    era: Option<crate::types::protocol::Era>,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    validate_protocol_version_supported(protocol_version)?;
    validate_protocol_version_matches_session(state, era, session_id, protocol_version)
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
                    crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
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
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if is_init_request {
        let (sid, _is_new) = process_init_session(state, era, session_id, protocol_version)?;
        Ok(sid)
    } else {
        validate_non_init_session(state, era, session_id)
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
            crate::types::protocol::error_codes::INTERNAL_ERROR,
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
) -> std::result::Result<HttpIngress, Response> {
    // Classify an internally-routed `server/discover` request first; every other
    // body keeps the existing middleware-aware parse + 400 assembly path.
    if let Some(ingress) = classify_http_ingress(body) {
        return Ok(ingress);
    }
    match crate::shared::StdioTransport::parse_message(body) {
        Ok(msg) => Ok(HttpIngress::Public(msg)),
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
                crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
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
    sessions_on: bool,
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
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                &format!("Failed to serialize response: {}", e),
            );
        },
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, APPLICATION_JSON.parse().unwrap());
    apply_session_header(&mut response_headers, response_session_id, sessions_on);
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
            crate::types::protocol::error_codes::INVALID_REQUEST,
            &format!("Request body exceeds limit: {}", e),
        )
    })?;
    Ok(String::from_utf8_lossy(&body_bytes).to_string())
}

/// Parse a JSON-RPC message on the fast path, returning a 400 error response
/// on failure.
///
/// Classifies an internally-routed `server/discover` request as
/// [`HttpIngress::Discover`] (which then CONTINUES the pipeline); every other
/// body flows through the existing [`StdioTransport::parse_message`] path as
/// [`HttpIngress::Public`], so all non-discover parse bytes are byte-identical.
fn parse_transport_message_fast(body: &[u8]) -> std::result::Result<HttpIngress, Response> {
    if let Some(ingress) = classify_http_ingress(body) {
        return Ok(ingress);
    }
    crate::shared::StdioTransport::parse_message(body)
        .map(HttpIngress::Public)
        .map_err(|e| {
            create_error_response(
                StatusCode::BAD_REQUEST,
                crate::types::protocol::error_codes::PARSE_ERROR,
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
    /// [`sessions_active`] for THIS request — gates the `Mcp-Session-Id`
    /// response header (HTTP-01).
    sessions_on: bool,
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
        sessions_on,
    } = dispatch;

    let era = protocol_context.as_ref().map(|pc| pc.era);
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

    // Code-driven v2 status: an error the HANDLER produced (e.g. -32601 for an
    // unsupported method, or plan 09's -32021) maps to its spec HTTP status.
    // `None` on v1 / not-opted-in, so every legacy status is unchanged.
    let v2_status = v2_dispatch_response_status(era, &json_response);

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

    apply_session_header(
        response.headers_mut(),
        response_session_id.as_ref(),
        sessions_on,
    );

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

    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }

    response
}

/// Assemble the `server/discover` response on the fast path (Phase 112, VERS-04).
///
/// Runs the SAME response tail as any fast-path request — projects via
/// [`Server::handle_discover`](crate::server::Server::handle_discover) (the ONE
/// shared `build_discover_response` era gate), stores the response event, builds
/// the response, and attaches session/version/outbound-v2 headers — preserving
/// the ORIGINAL request id. This is reached only AFTER session resolution, the v2
/// header matrix, legacy-version validation, and auth (classify-then-continue —
/// no pipeline bypass).
///
/// Response-shaping inputs shared by BOTH `server/discover` assemblers, so the
/// fast and middleware paths can never drift on session-header gating or the v2
/// outbound echo.
struct DiscoverResponseShape<'a> {
    /// The session id to echo, if any — already `None` on v2.
    response_session_id: Option<&'a String>,
    /// `Some((method, name))` for an accepted v2 discover (VERS-05 echo).
    v2_outbound: Option<(String, String)>,
    /// [`sessions_active`] for THIS request (HTTP-01).
    sessions_on: bool,
}

/// D-10 decision (finding #4): a v2 connection projects the server's
/// already-computed capabilities (incl. the `extensions` map); a v1 /
/// non-opted-in connection returns JSON-RPC `-32601` at HTTP 200 with the
/// original id. This `-32601@200` is a DELIBERATE, benign change from the
/// pre-112 incidental `PARSE_ERROR` 400 (`id: null`) — justified because
/// `server/discover` is a v2-only method NO conforming v1 client sends, so no
/// v1-relied-upon response byte changes (milestone byte-identity reconciled).
async fn assemble_discover_response_fast(
    state: &ServerState,
    id: crate::types::RequestId,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    shape: DiscoverResponseShape<'_>,
    session_id: Option<&String>,
) -> Response {
    let DiscoverResponseShape {
        response_session_id,
        v2_outbound,
        sessions_on,
    } = shape;
    let json_response = {
        let server = state.server.lock().await;
        server.handle_discover(id, protocol_context)
    };
    let v2_status = v2_dispatch_response_status(protocol_context.map(|pc| pc.era), &json_response);
    let response_msg = TransportMessage::Response(json_response);

    store_response_event(state, response_session_id, &response_msg).await;

    let mut response = build_response(state, response_msg, session_id);

    apply_session_header(response.headers_mut(), response_session_id, sessions_on);

    // Discover is never an init request → compute the outbound version normally.
    let version_to_send =
        compute_outbound_protocol_version(state, response_session_id, false, None);
    response
        .headers_mut()
        .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    // Echo the v2 outbound headers on an accepted v2 discover (VERS-05).
    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }

    if let Some(status) = v2_status {
        *response.status_mut() = status;
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

    let ingress = match parse_transport_message_fast(body.as_bytes()) {
        Ok(i) => i,
        // A v2 unknown method must be 404 + -32601 with the ORIGINAL id, even
        // though its body never produced a typed request (raw-level mapping).
        Err(response) => return map_unparsed_body_for_v2(&state, body.as_bytes(), response).await,
    };

    let (session_id, protocol_version) = extract_session_and_protocol_headers(&headers);
    // `server/discover` is a non-init request (stateless capability projection).
    let is_init_request = match &ingress {
        HttpIngress::Public(msg) => is_initialize_request(msg),
        HttpIngress::Discover { .. } => false,
    };

    // v2 required-header gate (VERS-05): resolve the ProtocolContext ONCE
    // (consumed by dispatch), classify the header/_meta matrix fail-closed, and
    // derive the outbound-header echo. Runs BEFORE session resolution (Plan
    // 113-04 / HTTP-01): the ERA decides whether sessions apply at all, so it
    // must be known before the first session decision. It also runs before the
    // legacy protocol-version check because an accepted v2 request carries
    // MCP-Protocol-Version: 2026-07-28, which the static-SUPPORTED check would
    // otherwise reject. v1 / non-opted-in → Passthrough (zero enforcement, D-04).
    // A `server/discover` ingress runs the SAME matrix via the raw-_meta
    // counterpart (finding #1).
    let (protocol_context, v2_outbound) = match &ingress {
        HttpIngress::Public(TransportMessage::Request { request, .. }) => {
            let (ctx, gate) = run_v2_header_gate(&state, request, &headers, body.as_bytes()).await;
            match gate {
                V2GateOutcome::Reject {
                    code,
                    message,
                    data,
                } => {
                    let era = ctx.as_ref().map(|pc| pc.era);
                    return v2_gate_reject_response(body.as_bytes(), era, code, &message, data);
                },
                V2GateOutcome::Passthrough => (ctx, None),
                V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
            }
        },
        HttpIngress::Discover { raw_meta, .. } => {
            let (ctx, gate) =
                run_v2_header_gate_raw(&state, raw_meta.as_ref(), &headers, body.as_bytes()).await;
            match gate {
                V2GateOutcome::Reject {
                    code,
                    message,
                    data,
                } => {
                    let era = ctx.as_ref().map(|pc| pc.era);
                    return v2_gate_reject_response(body.as_bytes(), era, code, &message, data);
                },
                V2GateOutcome::Passthrough => (ctx, None),
                V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
            }
        },
        HttpIngress::Public(_) => (None, None),
    };
    let is_v2_request = v2_outbound.is_some();
    let era = protocol_context.as_ref().map(|pc| pc.era);
    let sessions_on = sessions_active(&state, era);

    let response_session_id = match resolve_session_for_request(
        &state,
        era,
        is_init_request,
        session_id.clone(),
        protocol_version.clone(),
    ) {
        Ok(sid) => sid,
        Err(error_response) => return error_response,
    };

    // Legacy protocol-version validation applies to v1 non-init requests ONLY —
    // an accepted v2 request is validated by the gate above (D-11 untouched v1).
    // A v1 / non-opted-in `server/discover` also flows through here (no bypass).
    if !is_init_request && !is_v2_request {
        if let Err(error_response) =
            validate_protocol_version(&state, era, session_id.as_ref(), protocol_version.as_ref())
        {
            return error_response;
        }
    }

    let auth_context = match extract_and_validate_auth(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match ingress {
        HttpIngress::Public(TransportMessage::Request { id, request }) => {
            // `Box::pin`: the dispatch future crosses clippy's large_future
            // threshold once the v2 status mapping is threaded through it —
            // boxing keeps the handler future small without changing behavior
            // (same treatment the two POST entrypoints already get).
            Box::pin(handle_fast_path_request(
                &state,
                id,
                request,
                auth_context,
                FastPathDispatch {
                    is_init_request,
                    response_session_id,
                    protocol_context,
                    v2_outbound,
                    sessions_on,
                },
                session_id.as_ref(),
            ))
            .await
        },
        // Per-path response assembly (finding #3/#4): reached AFTER session, the v2
        // matrix, legacy-version validation, and auth — never an early return.
        HttpIngress::Discover { id, .. } => {
            assemble_discover_response_fast(
                &state,
                id,
                protocol_context.as_ref(),
                DiscoverResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    v2_outbound,
                    sessions_on,
                },
                session_id.as_ref(),
            )
            .await
        },
        HttpIngress::Public(
            TransportMessage::Notification { .. } | TransportMessage::Response(_),
        ) => StatusCode::ACCEPTED.into_response(),
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
                crate::types::protocol::error_codes::INVALID_REQUEST,
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
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<Option<String>, Response> {
    match resolve_session_for_request(state, era, is_init_request, session_id, protocol_version) {
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
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if is_init_request {
        return Ok(());
    }
    if let Err(error_response) = validate_protocol_version(state, era, session_id, protocol_version)
    {
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
    /// [`sessions_active`] for THIS request — gates the `Mcp-Session-Id`
    /// response header (HTTP-01).
    sessions_on: bool,
}

/// Assemble the `server/discover` response on the middleware path (VERS-04).
///
/// The middleware-path twin of [`assemble_discover_response_fast`]: projects via
/// [`Server::handle_discover`](crate::server::Server::handle_discover), stores the
/// response event, runs the SAME response-middleware assembly every other
/// response runs ([`build_success_response_with_middleware`]), and echoes the v2
/// outbound headers on an accepted v2 discover — preserving the original id.
/// Reached only AFTER session, the v2 matrix, legacy-version validation, and auth
/// (no bypass). See [`assemble_discover_response_fast`] for the D-10 `-32601@200`
/// decision on v1 / non-opted-in discover.
async fn assemble_discover_response_with_middleware(
    state: &ServerState,
    id: crate::types::RequestId,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    shape: DiscoverResponseShape<'_>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    let DiscoverResponseShape {
        response_session_id,
        v2_outbound,
        sessions_on,
    } = shape;
    let json_response = {
        let server = state.server.lock().await;
        server.handle_discover(id, protocol_context)
    };
    let v2_status = v2_dispatch_response_status(protocol_context.map(|pc| pc.era), &json_response);
    let response_msg = TransportMessage::Response(json_response);

    store_response_event(state, response_session_id, &response_msg).await;

    // Discover is never an init request → compute the outbound version normally.
    let version_to_send =
        compute_outbound_protocol_version(state, response_session_id, false, None);

    let mut response = build_success_response_with_middleware(
        &response_msg,
        response_session_id,
        &version_to_send,
        sessions_on,
        http_middleware,
        http_context,
    )
    .await;

    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }
    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }
    response
}

/// Dispatch the classified ingress on the middleware path.
///
/// Handles a public `Request` (server-handled + response assembly), a
/// `server/discover` ingress (the VERS-04 per-path assembly), `Notification`
/// (202 Accepted), and `Response` (202 Accepted) in separate arms.
async fn dispatch_message_with_middleware(
    state: &ServerState,
    ingress: HttpIngress,
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
        sessions_on,
    } = dispatch;
    match ingress {
        HttpIngress::Discover { id, .. } => {
            assemble_discover_response_with_middleware(
                state,
                id,
                protocol_context.as_ref(),
                DiscoverResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    v2_outbound,
                    sessions_on,
                },
                http_middleware,
                http_context,
            )
            .await
        },
        HttpIngress::Public(TransportMessage::Request { id, request }) => {
            let era = protocol_context.as_ref().map(|pc| pc.era);
            let json_response = {
                let server = state.server.lock().await;
                // Thread the ALREADY-RESOLVED ProtocolContext into dispatch
                // (Plan 06 / D-11): never re-resolved downstream.
                server
                    .handle_request_with_context(id, request, auth_context, protocol_context)
                    .await
            };
            // Code-driven v2 status (see the fast-path twin).
            let v2_status = v2_dispatch_response_status(era, &json_response);
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
                sessions_on,
                http_middleware,
                http_context,
            )
            .await;

            // v2 outbound headers on BOTH success and structured error (VERS-05).
            if let Some((method, name)) = &v2_outbound {
                apply_v2_outbound_headers(response.headers_mut(), method, name);
            }
            if let Some(status) = v2_status {
                *response.status_mut() = status;
            }
            response
        },
        HttpIngress::Public(
            TransportMessage::Notification { .. } | TransportMessage::Response(_),
        ) => StatusCode::ACCEPTED.into_response(),
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

    let ingress = match parse_transport_message_with_middleware(
        &server_request.body,
        http_middleware,
        &http_context,
    )
    .await
    {
        Ok(i) => i,
        // A v2 unknown method must be 404 + -32601 with the ORIGINAL id, even
        // though its body never produced a typed request (raw-level mapping).
        Err(response) => {
            return map_unparsed_body_for_v2(&state, &server_request.body, response).await
        },
    };

    let (session_id, protocol_version) =
        extract_session_and_protocol_headers(&server_request.headers);
    let is_init_request = match &ingress {
        HttpIngress::Public(msg) => is_initialize_request(msg),
        HttpIngress::Discover { .. } => false,
    };

    // v2 required-header gate (VERS-05): resolve the ProtocolContext ONCE and
    // classify the header/_meta matrix fail-closed before dispatch. Runs BEFORE
    // session resolution (Plan 113-04 / HTTP-01): the ERA decides whether
    // sessions apply at all. It also runs before the legacy protocol-version
    // check because an accepted v2 request carries MCP-Protocol-Version:
    // 2026-07-28 (which the static-SUPPORTED check would reject). Only Request /
    // discover ingresses carry a header contract; v1 / non-opted-in →
    // Passthrough (zero enforcement, D-04). A `server/discover` ingress runs the
    // SAME matrix via the raw-_meta counterpart (finding #1).
    let (protocol_context, v2_outbound) = match &ingress {
        HttpIngress::Public(TransportMessage::Request { request, .. }) => {
            let (ctx, gate) = run_v2_header_gate(
                &state,
                request,
                &server_request.headers,
                &server_request.body,
            )
            .await;
            match gate {
                V2GateOutcome::Reject {
                    code,
                    message,
                    data,
                } => {
                    report_middleware_error(
                        http_middleware,
                        &http_context,
                        "v2 header gate rejected",
                    )
                    .await;
                    let era = ctx.as_ref().map(|pc| pc.era);
                    return v2_gate_reject_response(
                        &server_request.body,
                        era,
                        code,
                        &message,
                        data,
                    );
                },
                V2GateOutcome::Passthrough => (ctx, None),
                V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
            }
        },
        HttpIngress::Discover { raw_meta, .. } => {
            let (ctx, gate) = run_v2_header_gate_raw(
                &state,
                raw_meta.as_ref(),
                &server_request.headers,
                &server_request.body,
            )
            .await;
            match gate {
                V2GateOutcome::Reject {
                    code,
                    message,
                    data,
                } => {
                    report_middleware_error(
                        http_middleware,
                        &http_context,
                        "v2 header gate rejected",
                    )
                    .await;
                    let era = ctx.as_ref().map(|pc| pc.era);
                    return v2_gate_reject_response(
                        &server_request.body,
                        era,
                        code,
                        &message,
                        data,
                    );
                },
                V2GateOutcome::Passthrough => (ctx, None),
                V2GateOutcome::EnforceOk { method, name } => (ctx, Some((method, name))),
            }
        },
        HttpIngress::Public(_) => (None, None),
    };
    let is_v2_request = v2_outbound.is_some();
    let era = protocol_context.as_ref().map(|pc| pc.era);
    let sessions_on = sessions_active(&state, era);

    let response_session_id = match resolve_session_with_error_hook(
        &state,
        era,
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

    // Legacy protocol-version validation applies to v1 non-init requests ONLY —
    // an accepted v2 request is validated by the gate above (v1 path untouched).
    if !is_v2_request {
        if let Err(response) = validate_protocol_version_with_error_hook(
            &state,
            era,
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

    // `Box::pin` the dispatch future: the discover per-path assembly (Plan 112-10)
    // grows it past clippy's large_future threshold; boxing keeps the handler
    // future small without changing behavior (mirrors the fast-path handler).
    Box::pin(dispatch_message_with_middleware(
        &state,
        ingress,
        MiddlewareDispatch {
            is_init_request,
            response_session_id,
            protocol_context,
            v2_outbound,
            sessions_on,
        },
        auth_context,
        http_middleware,
        &http_context,
    ))
    .await
}

/// Handle GET requests for SSE streams
/// Resolve the SSE session ID: validate an incoming one or mint a new one.
///
/// Returns `Ok(session_id)` on success, or an error response (404 unknown
/// session, 405 stateless-mode).
/// A GET carries no body and therefore no `_meta`, so the ONLY era signal is the
/// `MCP-Protocol-Version` header — and a v2 GET is already answered `405` by
/// [`handle_get_sse`] before this runs. Sessions are therefore evaluated at
/// `era = None`, which [`sessions_active`] resolves to exactly the pre-113
/// config-only behavior for the v1 / non-opted-in traffic that can reach here.
fn resolve_sse_session(
    state: &ServerState,
    incoming_session_id: Option<String>,
) -> std::result::Result<String, Response> {
    let sessions_on = sessions_active(state, None);
    if let Some(sid) = incoming_session_id {
        if sessions_on && !state.sessions.read().contains_key(&sid) {
            return Err(create_error_response(
                StatusCode::NOT_FOUND,
                crate::types::protocol::error_codes::INVALID_REQUEST,
                "Unknown session ID",
            ));
        }
        return Ok(sid);
    }
    let Some(generator) = active_session_generator(state, None) else {
        return Err(create_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
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
    if let Some(rejection) = v2_method_not_allowed(&headers, "GET") {
        return rejection;
    }
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
            crate::types::protocol::error_codes::INVALID_REQUEST,
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
    if let Some(rejection) = v2_method_not_allowed(&headers, "DELETE") {
        return rejection;
    }
    // Extract session ID
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(sid) = session_id {
        // Check if session exists
        let session_exists = state.sessions.read().contains_key(&sid);

        // A DELETE carries no body, so `era = None` — and a v2 DELETE is already
        // answered `405` above, so only v1 / non-opted-in traffic reaches here.
        // `sessions_active(state, None)` is exactly the pre-113 config-only read.
        if !session_exists && sessions_active(&state, None) {
            // Unknown session in stateful mode
            return create_error_response(
                StatusCode::NOT_FOUND,
                crate::types::protocol::error_codes::INVALID_REQUEST,
                "Unknown session ID",
            );
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
        create_error_response(
            StatusCode::NOT_FOUND,
            crate::types::protocol::error_codes::INVALID_REQUEST,
            "No session ID provided",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::protocol::Era;

    // -----------------------------------------------------------------------
    // Session era gate (Plan 113-04, HTTP-01).
    // -----------------------------------------------------------------------

    /// The full four-row truth table from the plan's `<behavior>` block.
    #[test]
    fn sessions_active_truth_table() {
        // A stateful config + a v2 request → sessions OFF (the whole point of
        // HTTP-01: the era overrides the build-time config).
        assert!(!sessions_active_for(true, Some(Era::V2)));
        // A stateful config + a v1 request → sessions ON, exactly as before.
        assert!(sessions_active_for(true, Some(Era::V1)));
        // A stateful config on a server NOT opted into v2 → sessions ON. `None`
        // means zero era code ran at all (D-04).
        assert!(sessions_active_for(true, None));
        // An explicitly `stateless()` server stays stateless in every era.
        assert!(!sessions_active_for(false, Some(Era::V2)));
        assert!(!sessions_active_for(false, Some(Era::V1)));
        assert!(!sessions_active_for(false, None));
    }

    /// A v2 request NEVER has sessions, whatever the config says.
    #[test]
    fn v2_always_suppresses_sessions() {
        for cfg in [true, false] {
            assert!(
                !sessions_active_for(cfg, Some(Era::V2)),
                "v2 must be session-free with cfg_has_generator = {cfg}"
            );
        }
    }

    /// `apply_session_header` is the ONLY session-header emitter, and it emits
    /// nothing when sessions are inactive — defense in depth for HTTP-01.
    #[test]
    fn session_header_is_never_emitted_when_sessions_are_inactive() {
        let sid = "sess-123".to_string();

        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&sid), false);
        assert!(
            headers.get(MCP_SESSION_ID).is_none(),
            "sessions inactive → no Mcp-Session-Id"
        );

        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&sid), true);
        assert_eq!(
            headers.get(MCP_SESSION_ID).and_then(|v| v.to_str().ok()),
            Some("sess-123"),
        );

        // No id to emit → nothing emitted, even with sessions active.
        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, None, true);
        assert!(headers.get(MCP_SESSION_ID).is_none());

        // A header-unrepresentable id is SKIPPED, never unwrapped (T-112-13).
        let bad = "bad\nvalue".to_string();
        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&bad), true);
        assert!(headers.get(MCP_SESSION_ID).is_none());
    }

    proptest::proptest! {
        /// The predicate never panics and is EXACTLY the stated boolean
        /// expression over arbitrary `(bool, Option<Era>)` inputs.
        #[test]
        fn sessions_active_is_exactly_its_stated_expression(
            cfg_has_generator in proptest::prelude::any::<bool>(),
            era_code in 0u8..3,
        ) {
            let era = match era_code {
                0 => None,
                1 => Some(Era::V1),
                _ => Some(Era::V2),
            };
            let expected = !matches!(era, Some(Era::V2)) && cfg_has_generator;
            proptest::prop_assert_eq!(sessions_active_for(cfg_has_generator, era), expected);
        }
    }

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

    use crate::types::protocol::error_codes::{HEADER_MISMATCH, METHOD_NOT_FOUND};
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
            V2Classification::Reject(HEADER_MISMATCH, _)
        ));
        // non-v2-header / v2-meta → reject
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Absent, true),
            V2Classification::Reject(HEADER_MISMATCH, _)
        ));
        assert!(matches!(
            classify_era_cell(HeaderProtocolVersion::Malformed, true),
            V2Classification::Reject(HEADER_MISMATCH, _)
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
        assert!(matches!(
            out,
            V2GateOutcome::Reject {
                code: HEADER_MISMATCH,
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // The locked `Mcp-Name` header rule, in BOTH directions (Plan 113-04).
    //
    // RULE (113-SPEC-RECHECK.md § `Mcp-Name Header Rule`, Phase-112 D-05):
    // `Mcp-Name` MUST be PRESENT on every v2 request; its VALUE is cross-checked
    // only for the name-bearing methods. Plan 05's client emits exactly this.
    // -----------------------------------------------------------------------

    #[test]
    fn name_less_method_with_empty_mcp_name_is_enforce_ok() {
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/list"),
            (MCP_NAME, ""),
        ]);
        let out = classify_v2_request(&h, true, Some("tools/list"), None);
        assert!(
            matches!(out, V2GateOutcome::EnforceOk { .. }),
            "an EMPTY Mcp-Name on a name-less v2 method must be ACCEPTED"
        );
    }

    #[test]
    fn name_less_method_with_absent_mcp_name_is_rejected() {
        // Header OMITTED entirely — presence is required on EVERY v2 request.
        let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, "tools/list")]);
        let out = classify_v2_request(&h, true, Some("tools/list"), None);
        assert!(
            matches!(
                out,
                V2GateOutcome::Reject {
                    code: HEADER_MISMATCH,
                    ..
                }
            ),
            "an ABSENT Mcp-Name must be rejected even for a name-less method"
        );
    }

    #[test]
    fn sentinel_encoded_mcp_name_matches_a_non_ascii_body_name() {
        let name = "日本語ツール";
        let encoded = crate::types::mrtr::encode_header_value(name);
        assert_ne!(encoded, name, "a non-ASCII name must be sentinel-encoded");

        // The pure cross-check decodes before comparing.
        assert!(cross_check_name(&encoded, "tools/call", Some(name)).is_ok());
        // ...and still rejects a genuine mismatch.
        assert!(cross_check_name(&encoded, "tools/call", Some("other")).is_err());

        // End to end through the classifier.
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, &encoded),
        ]);
        let out = classify_v2_request(&h, true, Some("tools/call"), Some(name));
        assert!(matches!(out, V2GateOutcome::EnforceOk { .. }));
    }

    #[test]
    fn malformed_mcp_name_sentinel_is_a_header_mismatch() {
        // Opens the sentinel but never closes it / is not valid base64.
        for bad in ["=?base64?not-base64!!", "=?base64?%%%%?="] {
            assert!(
                cross_check_name(bad, "tools/call", Some("search")).is_err(),
                "malformed sentinel `{bad}` must be rejected"
            );
            let h = headers_from(&[
                (MCP_PROTOCOL_VERSION, V2),
                (MCP_METHOD, "tools/call"),
                (MCP_NAME, bad),
            ]);
            let out = classify_v2_request(&h, true, Some("tools/call"), Some("search"));
            assert!(matches!(
                out,
                V2GateOutcome::Reject {
                    code: HEADER_MISMATCH,
                    ..
                }
            ));
        }
    }

    // -----------------------------------------------------------------------
    // v2 HTTP status mapping (Plan 113-04).
    // -----------------------------------------------------------------------

    #[test]
    fn v2_status_table_covers_every_transport_code() {
        use crate::types::protocol::error_codes as ec;
        assert_eq!(
            v2_status_for_code(ec::METHOD_NOT_FOUND),
            StatusCode::NOT_FOUND
        );
        for code in [
            ec::HEADER_MISMATCH,
            ec::MISSING_REQUIRED_CLIENT_CAPABILITY,
            ec::UNSUPPORTED_PROTOCOL_VERSION,
            ec::PARSE_ERROR,
            ec::INVALID_REQUEST,
            ec::INVALID_PARAMS,
        ] {
            assert_eq!(
                v2_status_for_code(code),
                StatusCode::BAD_REQUEST,
                "{code} must map to 400 on v2"
            );
        }
        // Handler semantics stay at HTTP 200 with the error in the body.
        for code in [ec::INTERNAL_ERROR, ec::REQUEST_TIMEOUT, ec::V1_TASK_PENDING] {
            assert_eq!(v2_status_for_code(code), StatusCode::OK);
        }
    }

    #[test]
    fn status_mapping_is_era_gated_so_v1_is_untouched() {
        use crate::types::protocol::Era;
        // v1 and not-opted-in keep the caller's v1 status for EVERY code.
        for era in [None, Some(Era::V1)] {
            for code in [
                METHOD_NOT_FOUND,
                HEADER_MISMATCH,
                crate::types::protocol::error_codes::PARSE_ERROR,
            ] {
                assert_eq!(status_for_error(era, code, StatusCode::OK), StatusCode::OK);
            }
        }
        // v2 re-maps from the table.
        assert_eq!(
            status_for_error(Some(Era::V2), METHOD_NOT_FOUND, StatusCode::OK),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn raw_request_id_survives_a_body_that_never_typed_parses() {
        // Numeric, string and absent ids, plus adversarial bytes — never panics.
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","id":7,"method":"totally/unknown"}"#),
            serde_json::json!(7)
        );
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","id":"abc","method":"nope","params":{}}"#),
            serde_json::json!("abc")
        );
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","method":"notify"}"#),
            serde_json::Value::Null
        );
        assert_eq!(raw_request_id(b"{not json"), serde_json::Value::Null);
        assert_eq!(raw_request_id(&[0xff, 0xfe, 0x00]), serde_json::Value::Null);
    }

    #[test]
    fn v2_dispatch_status_reads_the_code_not_the_call_site() {
        use crate::types::jsonrpc::{JSONRPCError, ResponsePayload};
        use crate::types::protocol::Era;

        let error_response = |code: i32| crate::types::JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: crate::types::RequestId::Number(1),
            payload: ResponsePayload::Error(JSONRPCError {
                code,
                message: "x".to_string(),
                data: None,
            }),
        };

        // -32021 is emitted by DISPATCH (plan 09), never by the header gate, so
        // the mapping must be code-driven to reach it at all.
        assert_eq!(
            v2_dispatch_response_status(
                Some(Era::V2),
                &error_response(
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                )
            ),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            v2_dispatch_response_status(Some(Era::V2), &error_response(METHOD_NOT_FOUND)),
            Some(StatusCode::NOT_FOUND)
        );
        // v1 / not-opted-in → no re-map at all.
        assert_eq!(
            v2_dispatch_response_status(Some(Era::V1), &error_response(METHOD_NOT_FOUND)),
            None
        );
        assert_eq!(
            v2_dispatch_response_status(None, &error_response(METHOD_NOT_FOUND)),
            None
        );
        // A successful result is never re-mapped.
        let ok = crate::types::JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: crate::types::RequestId::Number(1),
            payload: ResponsePayload::Result(serde_json::json!({})),
        };
        assert_eq!(v2_dispatch_response_status(Some(Era::V2), &ok), None);
    }

    #[test]
    fn v2_method_not_allowed_only_fires_on_the_v2_version_header() {
        // v2 header → 405 on both verbs.
        for verb in ["GET", "DELETE"] {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2)]);
            let response = v2_method_not_allowed(&h, verb).expect("v2 must be 405");
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        }
        // Absent / v1 / unknown / malformed → the v1 handler runs unchanged.
        assert!(v2_method_not_allowed(&headers_from(&[]), "GET").is_none());
        assert!(v2_method_not_allowed(
            &headers_from(&[(MCP_PROTOCOL_VERSION, "2025-11-25")]),
            "GET"
        )
        .is_none());
        let big = "x".repeat(MAX_V2_HEADER_VALUE_LEN + 1);
        assert!(
            v2_method_not_allowed(&headers_from(&[(MCP_PROTOCOL_VERSION, &big)]), "DELETE")
                .is_none()
        );
    }

    #[test]
    fn unsupported_version_reject_carries_a_supported_array() {
        use crate::types::protocol::context::ProtocolNegotiationError;
        let accept = vec![ProtocolVersion("2025-11-25".to_string()), v2_version()];
        let outcome = negotiation_error_to_gate_reject(
            &ProtocolNegotiationError::UnsupportedVersion("1999-01-01".to_string()),
            &accept,
        );
        let V2GateOutcome::Reject { code, data, .. } = outcome else {
            panic!("an unsupported version must reject");
        };
        assert_eq!(
            code,
            crate::types::protocol::error_codes::UNSUPPORTED_PROTOCOL_VERSION
        );
        let data = data.expect("UNSUPPORTED_PROTOCOL_VERSION MUST carry structured data");
        assert!(
            data["supported"].is_array(),
            "data.supported must be an ARRAY: {data}"
        );
        assert_eq!(data["supported"][0], "2025-11-25");
        assert_eq!(data["requested"], "1999-01-01");

        // A MALFORMED _meta keeps the shared INVALID_PARAMS mapping, no data.
        let outcome = negotiation_error_to_gate_reject(
            &ProtocolNegotiationError::MalformedMeta("bad"),
            &accept,
        );
        let V2GateOutcome::Reject { code, data, .. } = outcome else {
            panic!("malformed _meta must reject");
        };
        assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
        assert!(data.is_none());
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
    fn extract_body_method_and_name_uses_uri_for_resources_read() {
        // resources/read carries its logical name in params.uri (NO params.name).
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"mem://greeting"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("resources/read"));
        assert_eq!(
            n.as_deref(),
            Some("mem://greeting"),
            "resources/read logical name must come from params.uri"
        );

        // prompts/get still resolves the logical name from params.name.
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"greeting"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("prompts/get"));
        assert_eq!(n.as_deref(), Some("greeting"));

        // tools/call remains params.name (unchanged).
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("tools/call"));
        assert_eq!(n.as_deref(), Some("search"));

        // A resources/read carrying only uri yields NO name under the old
        // params.name view — the regression guard for review finding #2.
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"file:///x"}}"#;
        let (_, n) = extract_body_method_and_name(body);
        assert_eq!(n.as_deref(), Some("file:///x"));
    }

    #[test]
    fn cross_check_name_accepts_resources_read_uri() {
        // A standards-shaped resources/read cross-checks Mcp-Name against the URI.
        let uri = "mem://greeting";
        assert!(cross_check_name(uri, "resources/read", Some(uri)).is_ok());
        // A disagreeing Mcp-Name is rejected.
        assert!(cross_check_name(uri, "resources/read", Some("mem://other")).is_err());
        // Absent body name (would happen if extraction wrongly read params.name)
        // still fails closed for the name-bearing method.
        assert!(cross_check_name(uri, "resources/read", None).is_err());
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
                V2GateOutcome::Reject { code, .. } => {
                    proptest::prop_assert_eq!(code, HEADER_MISMATCH);
                },
            }
        }
    }

    // ---- Phase 112 Plan 10: HttpIngress classification + raw-_meta gate ----

    use crate::types::ProtocolVersion;

    fn v2_version() -> ProtocolVersion {
        ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string())
    }

    /// Build a `ServerState` whose backing `Server` carries `accept` as its
    /// supported-protocol accept-list (the only field the raw gate consults).
    fn state_with_accept(accept: Vec<ProtocolVersion>) -> ServerState {
        let server = Server::builder()
            .name("raw-gate-test")
            .version("1.0.0")
            .with_supported_protocol_versions(accept)
            .build()
            .expect("server builds");
        make_server_state(
            Arc::new(tokio::sync::Mutex::new(server)),
            StreamableHttpServerConfig::default(),
        )
    }

    /// `server/discover` is NOT a name-bearing method — its logical name is
    /// presence-only, so it must not appear in `is_name_bearing_method`.
    #[test]
    fn server_discover_is_not_name_bearing() {
        assert!(!is_name_bearing_method("server/discover"));
    }

    /// A well-formed `server/discover` body classifies as `HttpIngress::Discover`
    /// carrying the original id and the raw `_meta`; any other method or malformed
    /// input classifies as `Public`/`None` (never `Discover`), and never panics.
    #[test]
    fn classify_http_ingress_routes_server_discover_only() {
        let body = br#"{"jsonrpc":"2.0","id":7,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        let ingress = classify_http_ingress(body).expect("server/discover classifies");
        match ingress {
            HttpIngress::Discover { id, raw_meta } => {
                assert_eq!(id, crate::types::RequestId::from(7i64));
                assert_eq!(
                    raw_meta.unwrap()["io.modelcontextprotocol/protocolVersion"],
                    "2026-07-28"
                );
            },
            HttpIngress::Public(_) => panic!("server/discover must classify as Discover"),
        }

        // A normal method is NOT a discover ingress.
        let tools = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x"}}"#;
        assert!(classify_http_ingress(tools).is_none());
        // A notification (no id) is NOT a discover ingress.
        let notif = br#"{"jsonrpc":"2.0","method":"server/discover"}"#;
        assert!(classify_http_ingress(notif).is_none());
        // Garbage never panics and never classifies as Discover.
        assert!(classify_http_ingress(b"not json").is_none());
    }

    /// D-04 ordering: a NON-opted-in server short-circuits a discover request to
    /// Passthrough EVEN WITH a v2 `_meta` present — it must NOT reject as an
    /// unsupported version (the v2 `_meta` is never inspected).
    #[tokio::test]
    async fn run_v2_header_gate_raw_non_opted_in_passes_through() {
        let state = state_with_accept(vec![ProtocolVersion("2025-11-25".to_string())]);
        let raw_meta = serde_json::json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28"
        });
        let headers = headers_from(&[(MCP_PROTOCOL_VERSION, V2)]);
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}"#;
        let (ctx, outcome) = run_v2_header_gate_raw(&state, Some(&raw_meta), &headers, body).await;
        assert!(ctx.is_none(), "non-opted-in resolves no context");
        assert!(
            matches!(outcome, V2GateOutcome::Passthrough),
            "non-opted-in + v2 _meta must Passthrough, not Reject"
        );
    }

    /// An opted-in server with a well-formed v2 `_meta` + all three v2 headers
    /// accepts the discover request via the SAME matrix (`EnforceOk`).
    #[tokio::test]
    async fn run_v2_header_gate_raw_v2_opted_in_enforces() {
        let state = state_with_accept(vec![
            ProtocolVersion("2025-11-25".to_string()),
            v2_version(),
        ]);
        let raw_meta = serde_json::json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28"
        });
        // server/discover is presence-only for Mcp-Name (require_three_headers
        // still requires it present).
        let headers = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "server/discover"),
            (MCP_NAME, "server/discover"),
        ]);
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}"#;
        let (ctx, outcome) = run_v2_header_gate_raw(&state, Some(&raw_meta), &headers, body).await;
        assert_eq!(ctx.map(|c| c.era), Some(crate::types::protocol::Era::V2));
        assert!(matches!(outcome, V2GateOutcome::EnforceOk { .. }));
    }

    /// An opted-in server sees a discover request with a v2 `_meta` but NO
    /// `MCP-Protocol-Version` header rejected by the SAME matrix cell that
    /// rejects a tools/call with the same defect.
    #[tokio::test]
    async fn run_v2_header_gate_raw_v2_meta_without_header_rejects() {
        let state = state_with_accept(vec![
            ProtocolVersion("2025-11-25".to_string()),
            v2_version(),
        ]);
        let raw_meta = serde_json::json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28"
        });
        // No MCP-Protocol-Version header → conflict cell → Reject.
        let headers = headers_from(&[(MCP_METHOD, "server/discover"), (MCP_NAME, "x")]);
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}"#;
        let (_ctx, outcome) = run_v2_header_gate_raw(&state, Some(&raw_meta), &headers, body).await;
        assert!(matches!(outcome, V2GateOutcome::Reject { .. }));
    }

    proptest::proptest! {
        /// The raw-body ingress classifier NEVER panics over arbitrary bytes, and
        /// a non-`server/discover` method NEVER classifies as Discover (T-112-13).
        #[test]
        fn classify_http_ingress_never_panics(
            raw in proptest::collection::vec(proptest::num::u8::ANY, 0..512),
            method in "[a-z/]{0,24}",
            oversized in proptest::bool::ANY,
        ) {
            // Arbitrary bytes: must not panic.
            let _ = classify_http_ingress(&raw);

            // A structured request with an arbitrary method: only server/discover
            // may ever classify as Discover.
            let meta_val = if oversized { "x".repeat(20_000) } else { "2026-07-28".to_string() };
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": meta_val } }
            });
            let bytes = serde_json::to_vec(&body).unwrap();
            let classified = classify_http_ingress(&bytes);
            if method != "server/discover" {
                proptest::prop_assert!(
                    !matches!(classified, Some(HttpIngress::Discover { .. })),
                    "non-discover method {} must never classify as Discover",
                    method
                );
            }
        }
    }
}
