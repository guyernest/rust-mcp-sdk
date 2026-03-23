//! Tower middleware layers for MCP server security.
//!
//! Provides composable Tower Layers for DNS rebinding protection and
//! security response headers. These layers wrap OUTSIDE the existing
//! `ServerHttpMiddleware` chain.

pub mod dns_rebinding;
pub mod security_headers;

pub use dns_rebinding::{AllowedOrigins, DnsRebindingLayer, DnsRebindingService};
pub use security_headers::{SecurityHeadersLayer, SecurityHeadersService};

use http::Method;
use std::time::Duration;
use tower_http::cors::CorsLayer;

/// Build the standard MCP CORS layer for the given allowed origins.
///
/// Single source of truth for the CORS configuration used by both
/// [`StreamableHttpServer::start()`] and [`pmcp::axum::router()`].
pub(crate) fn build_mcp_cors_layer(allowed: &AllowedOrigins) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(allowed.to_cors_allow_origin())
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            http::header::CONTENT_TYPE,
            http::header::ACCEPT,
            http::HeaderName::from_static("mcp-session-id"),
            http::HeaderName::from_static("mcp-protocol-version"),
            http::HeaderName::from_static("last-event-id"),
        ])
        .expose_headers([
            http::HeaderName::from_static("mcp-session-id"),
            http::HeaderName::from_static("mcp-protocol-version"),
        ])
        .max_age(Duration::from_secs(86400))
}

/// Shared test utilities for tower layer tests.
#[cfg(test)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) mod test_util {
    use axum::body::Body;
    use http::{Request, Response, StatusCode};
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::Pin;
    use tower::Service;

    /// Pinned boxed future for service responses.
    type BoxFut = Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>;

    pub(crate) fn ok_service(
    ) -> impl Service<Request<Body>, Response = Response<Body>, Error = Infallible, Future = BoxFut>
           + Clone
           + Send
           + 'static {
        tower::service_fn(|_req: Request<Body>| {
            Box::pin(async {
                Ok::<_, Infallible>(
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from("ok"))
                        .unwrap(),
                )
            }) as BoxFut
        })
    }
}
