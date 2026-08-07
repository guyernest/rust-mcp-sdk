//! Shared components used by both client and server.

pub mod batch;
pub mod context;
/// The DEFAULT on-disk credential store — the gated I/O counterpart to
/// [`credential_store`] below.
///
/// Gated on `not(wasm32)` AND `feature = "oauth"` because every item in it needs
/// a filesystem, and `default_credential_path` needs the `oauth` feature's
/// `dirs` dependency. It is a SEPARATE module rather than a gated half of
/// `credential_store` so that the pure tier keeps its "no `#[cfg]` other than
/// `cfg(test)`" property, which is what makes its wasm32 cleanliness reviewable
/// at a glance. It knows nothing about the credential document's shape: the
/// format, the schema migration and the migration report all stay next door.
#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
pub mod credential_file;
/// Target-agnostic OAuth credential storage: the three-part key, the record,
/// the document format, the schema 1 to 2 migration and the platform seam.
///
/// Ungated on purpose — a file under the user's home directory is unusable on
/// AWS Lambda and per-container on Cloudflare Workers and Cloud Run, so
/// credential storage lands behind a trait and everything a platform needs in
/// order to implement that trait must compile where the `oauth` feature does
/// not exist, on host AND wasm32. Its only imports are this crate's error type,
/// `serde`, `async_trait`, `parking_lot` and the non-optional `url` crate. Do
/// NOT "tidy" a target or feature gate onto it: a second copy of the document
/// format and its migration is how a platform store and the CLI come to
/// disagree about what a stored credential means. A gated FILE implementation
/// is the deliberate counterpart and belongs in its own module. (Contrast the
/// `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries elsewhere in this
/// file; `oauth_validation` and `pkce` below carry the same rationale.)
pub mod credential_store;
pub mod event_store;
/// Hardened HTTP plumbing for this crate's OAuth/OIDC surfaces: the streaming
/// bounded whole-body read every auth response is read through, and the
/// discovery HTTP client whose redirect policy cannot be steered off the
/// issuer's origin.
///
/// Gated on `feature = "http-client"` because every item in it takes or returns
/// a `reqwest` type; the wasm32 build does not enable that feature and must not
/// see this module. `pub(crate)` on purpose — the four auth files that consume
/// it are all in-crate, and this hardening adds no public surface it does not
/// need.
#[cfg(feature = "http-client")]
pub(crate) mod http_body_cap;
pub mod http_utils;
pub mod logging;
pub mod middleware;
pub mod middleware_presets;
/// Target-agnostic OAuth authorization-RESPONSE validation (RFC 9207 `iss`,
/// CSRF `state`).
///
/// Ungated on purpose — it must be callable from a Cloudflare Workers or
/// Lambda redirect handler, where the `oauth` feature (and its `webbrowser` /
/// `dirs` / `rand` dependencies) does not exist and does not build. Its only
/// imports are this crate's error type and the non-optional `url` crate, so it
/// compiles on host AND wasm32. Do NOT "tidy" a `cfg` onto it: a second copy of
/// the RFC 9207 decision table is how a platform handler and the CLI come to
/// disagree about what "valid" means. (Contrast the
/// `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries elsewhere in this
/// file, and note `pkce` below carries the same rationale for the same reason.)
pub mod oauth_validation;
/// Peer back-channel trait for server-to-client RPCs from inside request handlers.
#[cfg(not(target_arch = "wasm32"))]
pub mod peer;
/// Target-agnostic one-slot pending-response buffer for one-shot transports.
///
/// Internal plumbing (`pub(crate)`) backing the `WasmHttpTransport`
/// send→receive correlation; ungated so it host-tests under plain `cargo test`.
pub(crate) mod pending_slot;
/// Target-agnostic PKCE (RFC 7636) crypto helper (verifier/challenge/state).
///
/// Ungated on purpose — compiles on host AND wasm32 via `getrandom::fill`
/// (contrast the `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries).
pub mod pkce;
pub mod protocol;
pub mod protocol_helpers;
#[cfg(not(target_arch = "wasm32"))]
pub mod reconnect;
pub mod session;
pub mod simd_parsing;
// Dead on wasm32 by CONFIGURATION, not by disuse: this module's consumers are the
// native server/client tier (`src/server/core.rs`, `src/server/task_dispatch.rs`,
// `src/client/mod.rs`), all of which are `#[cfg(not(target_arch = "wasm32"))]`. The
// items are `pub(crate)` and very much alive natively, so they must NOT be deleted;
// the wasm build simply has no callers for them. Scoped to wasm32 so genuine dead
// code is still caught on every other target.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod sse_parser;

#[cfg(feature = "sse")]
pub mod sse_optimized;

#[cfg(not(target_arch = "wasm32"))]
pub mod connection_pool;
#[cfg(not(target_arch = "wasm32"))]
pub mod stdio;
pub mod transport;
pub mod uri_template;

// Cross-platform runtime abstraction
pub mod runtime;

// Platform-specific WebSocket modules
#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub mod websocket;

#[cfg(all(feature = "websocket-wasm", target_arch = "wasm32"))]
pub mod wasm_websocket;

#[cfg(target_arch = "wasm32")]
pub mod wasm_http;

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub mod http;
pub mod http_constants;

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
/// Streamable HTTP transport implementation for MCP.
pub mod streamable_http;

// Re-export commonly used types
pub use batch::{BatchRequest, BatchResponse};
pub use context::{ClientInfo, ContextPropagator, RequestContext};
pub use event_store::{
    EventStore, EventStoreConfig, InMemoryEventStore, MessageDirection, ResumptionManager,
    ResumptionState, ResumptionToken, StoredEvent,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
pub use logging::init_logging;
pub use logging::{CorrelatedLogger, LogConfig, LogEntry, LogFormat, LogLevel};
pub use middleware::{
    AdvancedMiddleware, AuthMiddleware, CircuitBreakerMiddleware, CompressionMiddleware,
    CompressionType, EnhancedMiddlewareChain, LoggingMiddleware, MetricsMiddleware, Middleware,
    MiddlewareChain, MiddlewareContext, MiddlewarePriority, PerformanceMetrics,
    RateLimitMiddleware, RetryMiddleware,
};
pub use protocol::{ProgressCallback, Protocol, ProtocolOptions, RequestOptions};
pub use protocol_helpers::{
    create_notification, create_request, parse_notification, parse_request,
};
#[cfg(not(target_arch = "wasm32"))]
pub use reconnect::{ReconnectConfig, ReconnectGuard, ReconnectManager};
pub use session::{Session, SessionConfig, SessionManager};
#[cfg(not(target_arch = "wasm32"))]
pub use stdio::StdioTransport;
pub use transport::{Transport, TransportMessage};
pub use uri_template::UriTemplate;

#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub use websocket::{WebSocketConfig, WebSocketTransport};

#[cfg(all(feature = "websocket-wasm", target_arch = "wasm32"))]
pub use wasm_websocket::{WasmWebSocketConfig, WasmWebSocketTransport};

#[cfg(target_arch = "wasm32")]
pub use wasm_http::{WasmHttpClient, WasmHttpConfig, WasmHttpTransport};

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use http::{HttpConfig, HttpTransport};

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub use streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};

// Why: `OptimizedSseTransport` is deprecated on purpose (plan 113.1-03, D-01)
// but is NOT removed — retiring a public item is a 3.0 action, and this
// milestone's additivity claim is "zero removed public items". The `deprecated`
// lint fires on a `pub use` re-export within the defining crate, and `make lint`
// runs with `-D warnings`, so the crate must allow it to compile its own
// retained transport. `OptimizedSseConfig` is deliberately NOT deprecated.
#[allow(deprecated)]
#[cfg(feature = "sse")]
pub use sse_optimized::{OptimizedSseConfig, OptimizedSseTransport};

#[cfg(not(target_arch = "wasm32"))]
pub use connection_pool::{
    ConnectionId, ConnectionPool, ConnectionPoolConfig, HealthStatus, LoadBalanceStrategy,
    PoolStats, PooledTransport,
};

pub use simd_parsing::{
    CpuFeatures, ParsingMetrics, SimdBase64, SimdHttpHeaderParser, SimdJsonParser, SimdSseParser,
};
