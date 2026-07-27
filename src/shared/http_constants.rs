//! Constants for HTTP headers, content types and transport byte ceilings used
//! in MCP.
//!
//! This module is deliberately UNGATED (`src/shared/mod.rs` declares it with no
//! `cfg`), which is why the shared SSE byte ceiling lives here rather than in
//! `crate::shared::http`: that module is gated on `feature = "http"`, and
//! `feature = "sse"` does not enable it, yet `sse_optimized` needs the same
//! number. One ceiling for every SSE read in the crate.

// Header Names
/// MCP session ID header name
pub const MCP_SESSION_ID: &str = "mcp-session-id";

/// MCP protocol version header name
pub const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";

/// MCP method header name (VERS-05, v2 `2026-07-28`).
///
/// On the v2 HTTP path this header MUST be present and MUST equal the
/// JSON-RPC body's `method` (cross-checked fail-closed; a header/body desync
/// is a smuggling signal — D-06). HTTP header names are case-insensitive; the
/// lowercase form matches the existing `mcp-*` family.
pub const MCP_METHOD: &str = "mcp-method";

/// MCP name header name (VERS-05, v2 `2026-07-28`).
///
/// On the v2 HTTP path this header MUST be present; for name-bearing methods
/// (`tools/call`, `prompts/get`, `resources/read`) its value is cross-checked
/// against the request's logical name (`params.name`) and a mismatch is
/// rejected fail-closed (same class as D-06).
pub const MCP_NAME: &str = "mcp-name";

/// SSE Last-Event-ID header name for resumption
pub const LAST_EVENT_ID: &str = "Last-Event-ID";

/// HTTP Accept header name
pub const ACCEPT: &str = "Accept";

/// HTTP Content-Type header name
pub const CONTENT_TYPE: &str = "Content-Type";

// Content Types
/// JSON content type value
pub const APPLICATION_JSON: &str = "application/json";

/// Server-Sent Events content type value
pub const TEXT_EVENT_STREAM: &str = "text/event-stream";

/// Accept header value for streamable HTTP (both JSON and SSE)
pub const ACCEPT_STREAMABLE: &str = "application/json, text/event-stream";

// Transport byte ceilings

/// Default ceiling on the SSE bytes an SSE reader may hold IN FLIGHT, in bytes
/// (16 MiB).
///
/// # Its two readers
///
/// One number for every SSE read in the crate (plan 113.1-03, D-03):
///
/// - `HttpTransport::connect_sse`'s reader task, which retains across chunks;
/// - `OptimizedSseTransport::connect_sse`, whose whole-response read is bounded
///   by a running total checked against this value before each append.
///
/// Re-exported as `crate::shared::http::DEFAULT_HTTP_SSE_BUFFERED_BYTES`, which
/// remains the documented public path. The definition lives here because this
/// module is ungated while `crate::shared::http` is behind `feature = "http"`,
/// which `feature = "sse"` does not enable.
///
/// # What breaks at this boundary
///
/// A single JSON-RPC payload whose in-flight bytes exceed the configured ceiling
/// is DISCARDED and ENDS the reader task. That is a real behaviour change: before
/// Phase 113-17 that transport's parser bounded only an unterminated line, so an
/// arbitrarily large `data:` payload accumulated without limit and was delivered
/// (T-113-85).
///
/// # Why it is configurable rather than fixed
///
/// A fixed ceiling is not defensible for a transport that carries arbitrary
/// JSON-RPC results. MCP `image`/`audio` content is unconstrained base64, and
/// base64 expands by ~4/3: a 12 MiB binary is ALREADY 16 MiB once encoded,
/// BEFORE the JSON envelope, the `data: ` prefix and the MIME type — so it does
/// NOT fit under this default. Large text, resources and `structuredContent` can
/// legitimately exceed it too. Any claim that media is "unaffected" by a 16 MiB
/// ceiling is arithmetically false.
///
/// [`crate::shared::http::HttpTransport::with_sse_buffered_bytes`] is the escape
/// hatch for that reader: raise the ceiling for a deployment whose payloads are
/// legitimately larger. `OptimizedSseTransport` has no such setter — it is
/// deprecated toward `StreamableHttpTransport`, which carries its own
/// configurable cap.
pub const DEFAULT_HTTP_SSE_BUFFERED_BYTES: usize = 16 * 1024 * 1024;
