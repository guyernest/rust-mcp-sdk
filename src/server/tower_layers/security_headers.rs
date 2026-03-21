//! Security response headers layer for MCP servers.
//!
//! Adds OWASP-recommended security headers to all HTTP responses.
//! Placeholder -- implementation in Task 2.

use axum::body::Body;
use http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower Layer that adds OWASP security headers to responses.
#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer;

impl SecurityHeadersLayer {
    /// Create a new security headers layer with all headers enabled.
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService { inner }
    }
}

/// Tower Service that adds security headers to responses.
#[derive(Debug, Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
}

impl<S, ReqBody> Service<Request<ReqBody>> for SecurityHeadersService<S>
where
    S: Service<Request<ReqBody>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send,
    ReqBody: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        std::mem::swap(&mut inner, &mut self.inner);
        Box::pin(async move { inner.call(req).await })
    }
}
