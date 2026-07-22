//! Constants for HTTP headers and content types used in MCP.

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
