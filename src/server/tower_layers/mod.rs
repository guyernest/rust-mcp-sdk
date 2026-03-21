//! Tower middleware layers for MCP server security.
//!
//! Provides composable Tower Layers for DNS rebinding protection and
//! security response headers. These layers wrap OUTSIDE the existing
//! `ServerHttpMiddleware` chain.

pub mod dns_rebinding;
pub mod security_headers;

pub use dns_rebinding::{AllowedOrigins, DnsRebindingLayer, DnsRebindingService};
pub use security_headers::{SecurityHeadersLayer, SecurityHeadersService};

/// Shared test utilities for tower layer tests.
#[cfg(test)]
pub(crate) mod test_util {
    use axum::body::Body;
    use http::{Request, Response, StatusCode};
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::Pin;
    use tower::Service;

    pub fn ok_service() -> impl Service<
        Request<Body>,
        Response = Response<Body>,
        Error = Infallible,
        Future = Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>,
    > + Clone
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
            })
                as Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>
        })
    }
}
