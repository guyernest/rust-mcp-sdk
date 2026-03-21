//! Security response headers layer for MCP servers.
//!
//! Adds OWASP-recommended security headers to all HTTP responses:
//! - `X-Content-Type-Options: nosniff` -- prevents MIME-type sniffing
//! - `X-Frame-Options: DENY` -- prevents clickjacking via iframes
//! - `Cache-Control: no-store` -- prevents caching of sensitive responses
//!
//! Each header can be individually disabled via builder methods.
//! No HSTS header is added (per design decision D-12: transport-layer
//! concern handled by reverse proxy).

use axum::body::Body;
use http::{HeaderValue, Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower Layer that adds OWASP security headers to HTTP responses.
///
/// All three headers are enabled by default. Individual headers can be
/// disabled with the `without_*` builder methods.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::tower_layers::SecurityHeadersLayer;
///
/// let layer = SecurityHeadersLayer::new()
///     .without_cache_control(); // Keep X-Content-Type-Options and X-Frame-Options
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SecurityHeadersLayer {
    x_content_type_options: bool,
    x_frame_options: bool,
    cache_control: bool,
}

impl Default for SecurityHeadersLayer {
    fn default() -> Self {
        Self {
            x_content_type_options: true,
            x_frame_options: true,
            cache_control: true,
        }
    }
}

impl SecurityHeadersLayer {
    /// Create a new security headers layer with all headers enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable the `X-Content-Type-Options: nosniff` header.
    #[must_use]
    pub fn without_x_content_type_options(mut self) -> Self {
        self.x_content_type_options = false;
        self
    }

    /// Disable the `X-Frame-Options: DENY` header.
    #[must_use]
    pub fn without_x_frame_options(mut self) -> Self {
        self.x_frame_options = false;
        self
    }

    /// Disable the `Cache-Control: no-store` header.
    #[must_use]
    pub fn without_cache_control(mut self) -> Self {
        self.cache_control = false;
        self
    }
}

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService {
            inner,
            config: self.clone(),
        }
    }
}

/// Tower Service that adds security headers to HTTP responses.
///
/// Created by [`SecurityHeadersLayer`]. Adds configured headers to every
/// response returned by the inner service.
#[derive(Debug, Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
    config: SecurityHeadersLayer,
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
        let config = self.config.clone();
        let mut inner = self.inner.clone();
        std::mem::swap(&mut inner, &mut self.inner);

        Box::pin(async move {
            let mut response = inner.call(req).await?;
            let headers = response.headers_mut();

            if config.x_content_type_options {
                headers.insert(
                    "x-content-type-options",
                    HeaderValue::from_static("nosniff"),
                );
            }
            if config.x_frame_options {
                headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
            }
            if config.cache_control {
                // Use entry() to avoid overwriting handler-specific Cache-Control
                // (e.g., SSE responses use "no-cache, no-transform").
                headers
                    .entry("cache-control")
                    .or_insert(HeaderValue::from_static("no-store"));
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_util::ok_service;
    use http::StatusCode;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::Pin;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_default_headers() {
        let layer = SecurityHeadersLayer::new();
        let svc = layer.layer(ok_service());

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn test_without_x_content_type_options() {
        let layer = SecurityHeadersLayer::new().without_x_content_type_options();
        let svc = layer.layer(ok_service());

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        assert!(resp.headers().get("x-content-type-options").is_none());
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn test_without_x_frame_options() {
        let layer = SecurityHeadersLayer::new().without_x_frame_options();
        let svc = layer.layer(ok_service());

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert!(resp.headers().get("x-frame-options").is_none());
        assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn test_without_cache_control() {
        let layer = SecurityHeadersLayer::new().without_cache_control();
        let svc = layer.layer(ok_service());

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert!(resp.headers().get("cache-control").is_none());
    }

    #[tokio::test]
    async fn test_preserves_inner_response() {
        let custom_service = tower::service_fn(|_req: Request<Body>| {
            Box::pin(async {
                Ok::<_, Infallible>(
                    Response::builder()
                        .status(StatusCode::CREATED)
                        .body(Body::from("created"))
                        .unwrap(),
                )
            })
                as Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>
        });

        let layer = SecurityHeadersLayer::new();
        let svc = layer.layer(custom_service);

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        // Security headers should still be added.
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }

    #[tokio::test]
    async fn test_no_hsts() {
        let layer = SecurityHeadersLayer::new();
        let svc = layer.layer(ok_service());

        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = svc.oneshot(req).await.unwrap();

        // Per D-12: No HSTS -- that is a transport-layer concern for reverse proxies.
        assert!(resp.headers().get("strict-transport-security").is_none());
    }
}
