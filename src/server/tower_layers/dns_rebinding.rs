//! DNS rebinding protection layer for MCP servers.
//!
//! Validates `Host` and `Origin` request headers against an allowlist derived
//! from the server's bind address. Rejects requests with disallowed headers
//! with HTTP 403, protecting against DNS rebinding attacks (CVE-2025-66414).

use axum::body::Body;
use http::{header, HeaderMap, HeaderValue, Request, Response, StatusCode};
use std::collections::HashSet;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::Layer;
use tower::Service;
use tower_http::cors::AllowOrigin;
use tracing::warn;

/// Allowed origins configuration for Host and Origin header validation.
///
/// Single source of truth for which hostnames and origins are permitted.
/// Constructed from the server's bind address (auto-detecting localhost aliases)
/// or from an explicit list of origins.
#[derive(Debug, Clone)]
pub struct AllowedOrigins {
    /// Full origin strings (e.g., `http://localhost:8080`).
    origins: Vec<String>,
    /// Extracted hostnames for Host header validation (e.g., `localhost`).
    hostnames: HashSet<String>,
}

impl AllowedOrigins {
    /// Create allowed origins from a bind address.
    ///
    /// If the address is loopback or unspecified (0.0.0.0), automatically
    /// includes `localhost`, `127.0.0.1`, and `[::1]` as allowed hostnames.
    /// Otherwise, only the bound IP address is allowed.
    pub fn from_bind_addr(addr: SocketAddr) -> Self {
        let ip = addr.ip();
        let port = addr.port();

        if ip.is_loopback() || ip.is_unspecified() {
            let localhost_hosts = ["localhost", "127.0.0.1", "[::1]"];
            let hostnames: HashSet<String> =
                localhost_hosts.iter().map(|h| (*h).to_string()).collect();
            let origins: Vec<String> = localhost_hosts
                .iter()
                .flat_map(|h| {
                    vec![
                        format!("http://{}:{}", h, port),
                        format!("https://{}:{}", h, port),
                    ]
                })
                .collect();
            Self { origins, hostnames }
        } else {
            let ip_str = match ip {
                IpAddr::V6(v6) => format!("[{}]", v6),
                IpAddr::V4(v4) => v4.to_string(),
            };
            let hostnames: HashSet<String> = std::iter::once(ip_str.clone()).collect();
            let origins = vec![
                format!("http://{}:{}", ip_str, port),
                format!("https://{}:{}", ip_str, port),
            ];
            Self { origins, hostnames }
        }
    }

    /// Create allowed origins from an explicit list of origin URLs.
    ///
    /// Parses each origin to extract the hostname for Host header validation.
    /// Origins should include the scheme (e.g., `https://myapp.com`).
    pub fn explicit(origins: Vec<String>) -> Self {
        let mut hostnames = HashSet::new();
        for origin in &origins {
            if let Some(hostname) = extract_hostname(origin) {
                hostnames.insert(hostname);
            }
        }
        Self { origins, hostnames }
    }

    /// Convenience constructor for localhost development.
    ///
    /// Equivalent to `from_bind_addr(127.0.0.1:0)` -- includes all
    /// localhost aliases with port 0 (matching any port in Host checks).
    pub fn localhost() -> Self {
        Self::from_bind_addr(SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    /// Allow all origins — disables Host and Origin validation.
    ///
    /// Use this for servers behind a reverse proxy that handles CORS at
    /// the edge (e.g., API Gateway + Lambda, `CloudFront`, nginx). In these
    /// deployments, DNS rebinding protection adds no security value because
    /// the MCP server is only reachable via loopback within the sandbox.
    ///
    /// `StreamableHttpServerConfig::stateless()` uses this by default.
    ///
    /// # Security Note
    ///
    /// Only use this when the server is behind a proxy that enforces its
    /// own origin policy. Do NOT use for servers directly exposed to the
    /// internet or accessible from a browser.
    pub fn any() -> Self {
        Self {
            origins: Vec::new(),
            hostnames: HashSet::new(),
        }
    }

    /// Returns `true` if this is an `any()` configuration (all origins allowed).
    pub fn is_any(&self) -> bool {
        self.origins.is_empty() && self.hostnames.is_empty()
    }

    /// Check if the Host header value is in the allowed set.
    ///
    /// Strips the port from the Host value before checking.
    /// Returns `false` for missing or non-ASCII Host headers.
    /// Always returns `true` for [`AllowedOrigins::any()`].
    pub fn is_allowed_host(&self, headers: &HeaderMap) -> bool {
        if self.is_any() {
            return true;
        }
        let Some(host_value) = headers.get(header::HOST) else {
            return false;
        };
        let Ok(host_str) = host_value.to_str() else {
            return false;
        };
        let hostname = strip_port(host_str);
        self.hostnames.contains(hostname)
    }

    /// Check if an Origin header value is in the allowed set.
    ///
    /// Checks the full origin string first, then falls back to hostname
    /// extraction for port mismatch tolerance.
    /// Always returns `true` for [`AllowedOrigins::any()`].
    pub fn is_allowed_origin(&self, origin: &HeaderValue) -> bool {
        if self.is_any() {
            return true;
        }
        let Ok(origin_str) = origin.to_str() else {
            return false;
        };
        // Check full origin match first (no allocation — compare &str directly).
        if self.origins.iter().any(|o| o.as_str() == origin_str) {
            return true;
        }
        // Fallback: extract hostname and check against allowed hostnames.
        if let Some(hostname) = extract_hostname(origin_str) {
            return self.hostnames.contains(&hostname);
        }
        false
    }

    /// Convert to a `tower_http` CORS `AllowOrigin` for use with `CorsLayer`.
    ///
    /// For [`AllowedOrigins::any()`], returns `AllowOrigin::any()` (wildcard `*`).
    /// Otherwise, uses a predicate that delegates to [`is_allowed_origin`](Self::is_allowed_origin),
    /// which includes hostname-fallback matching. This handles port mismatches
    /// (e.g., `AllowedOrigins::localhost()` with port 0 still accepts
    /// `Origin: http://localhost:8080`).
    pub fn to_cors_allow_origin(&self) -> AllowOrigin {
        if self.is_any() {
            return AllowOrigin::any();
        }
        let this = self.clone();
        AllowOrigin::predicate(move |origin: &HeaderValue, _parts: &http::request::Parts| {
            this.is_allowed_origin(origin)
        })
    }

    /// Get the list of allowed origin URLs.
    pub fn origins(&self) -> &[String] {
        &self.origins
    }

    /// Access the set of allowed hostnames (for testing and diagnostics).
    #[cfg(test)]
    pub(crate) fn hostnames(&self) -> &HashSet<String> {
        &self.hostnames
    }
}

/// Extract hostname from an origin URL string.
///
/// Strips the scheme (`http://`, `https://`) and port (`:NNNN`), returning
/// only the hostname component.
fn extract_hostname(origin: &str) -> Option<String> {
    let without_scheme = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))?;
    // Strip path if present.
    let without_path = without_scheme.split('/').next().unwrap_or(without_scheme);
    // Strip port.
    Some(strip_port(without_path).to_string())
}

/// Strip the port suffix from a `host:port` string.
///
/// For IPv6 bracket notation (`[::1]:8080`), strips after the closing bracket.
fn strip_port(host: &str) -> &str {
    // IPv6 bracket notation: [::1]:8080
    if let Some(bracket_end) = host.rfind(']') {
        // Return everything up to and including the bracket.
        &host[..=bracket_end]
    } else {
        // IPv4 / hostname: split on last colon.
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    }
}

/// Tower Layer that validates Host and Origin headers for DNS rebinding protection.
///
/// Wraps an inner service, rejecting requests whose `Host` header is not in
/// the allowed set with HTTP 403. If an `Origin` header is present and not
/// allowed, also returns 403. Missing `Origin` is permitted (non-browser clients
/// like curl do not send it).
#[derive(Debug, Clone)]
pub struct DnsRebindingLayer {
    allowed_origins: AllowedOrigins,
}

impl DnsRebindingLayer {
    /// Create a new DNS rebinding protection layer.
    pub fn new(allowed_origins: AllowedOrigins) -> Self {
        Self { allowed_origins }
    }
}

impl<S> Layer<S> for DnsRebindingLayer {
    type Service = DnsRebindingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DnsRebindingService {
            inner,
            allowed_origins: self.allowed_origins.clone(),
        }
    }
}

/// Tower Service that performs DNS rebinding header validation.
///
/// Created by [`DnsRebindingLayer`]. Validates Host (always) and Origin
/// (when present) headers against the configured [`AllowedOrigins`].
#[derive(Clone)]
pub struct DnsRebindingService<S> {
    inner: S,
    allowed_origins: AllowedOrigins,
}

impl<S> std::fmt::Debug for DnsRebindingService<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DnsRebindingService")
            .field("allowed_origins", &self.allowed_origins)
            .finish_non_exhaustive()
    }
}

impl<S, ReqBody> Service<Request<ReqBody>> for DnsRebindingService<S>
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
        // Check Host header.
        if !self.allowed_origins.is_allowed_host(req.headers()) {
            let host_val = req
                .headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<missing>");
            warn!(
                host = %host_val,
                "DNS rebinding protection: rejected request with disallowed Host header"
            );
            return Box::pin(async {
                Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("content-type", "text/plain")
                    .body(Body::from(
                        "Forbidden: Host header not in allowed origins\n",
                    ))
                    .expect("static response"))
            });
        }

        // Check Origin header (only when present -- non-browser clients omit it).
        if let Some(origin) = req.headers().get(header::ORIGIN) {
            if !self.allowed_origins.is_allowed_origin(origin) {
                let origin_val = origin.to_str().unwrap_or("<non-ascii>");
                warn!(
                    origin = %origin_val,
                    "DNS rebinding protection: rejected request with disallowed Origin header"
                );
                return Box::pin(async {
                    Ok(Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("content-type", "text/plain")
                        .body(Body::from("Forbidden: Origin not in allowed origins\n"))
                        .expect("static response"))
                });
            }
        }

        // Both checks passed -- forward to inner service.
        let mut inner = self.inner.clone();
        std::mem::swap(&mut inner, &mut self.inner);
        Box::pin(async move { inner.call(req).await })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::ok_service;
    use super::*;
    use tower::ServiceExt;

    // -- AllowedOrigins unit tests --

    #[test]
    fn test_auto_detect_localhost() {
        let ao = AllowedOrigins::from_bind_addr("127.0.0.1:8080".parse().unwrap());
        assert!(ao.hostnames().contains("localhost"));
        assert!(ao.hostnames().contains("127.0.0.1"));
        assert!(ao.hostnames().contains("[::1]"));
    }

    #[test]
    fn test_auto_detect_unspecified() {
        let ao = AllowedOrigins::from_bind_addr("0.0.0.0:8080".parse().unwrap());
        assert!(ao.hostnames().contains("localhost"));
        assert!(ao.hostnames().contains("127.0.0.1"));
        assert!(ao.hostnames().contains("[::1]"));
    }

    #[test]
    fn test_auto_detect_public_ip() {
        let ao = AllowedOrigins::from_bind_addr("192.168.1.5:8080".parse().unwrap());
        assert!(ao.hostnames().contains("192.168.1.5"));
        assert!(!ao.hostnames().contains("localhost"));
        assert_eq!(ao.hostnames().len(), 1);
    }

    #[test]
    fn test_explicit_origins() {
        let ao = AllowedOrigins::explicit(vec!["https://myapp.com".to_string()]);
        assert!(ao.hostnames().contains("myapp.com"));
    }

    #[test]
    fn test_any_allows_all_hosts() {
        let ao = AllowedOrigins::any();
        assert!(ao.is_any());
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("evil.com"));
        assert!(ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_any_allows_all_origins() {
        let ao = AllowedOrigins::any();
        let origin = HeaderValue::from_static("https://anything.example.com");
        assert!(ao.is_allowed_origin(&origin));
    }

    #[test]
    fn test_any_allows_missing_host() {
        let ao = AllowedOrigins::any();
        let headers = HeaderMap::new();
        assert!(ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_localhost_is_not_any() {
        let ao = AllowedOrigins::localhost();
        assert!(!ao.is_any());
    }

    #[test]
    fn test_is_allowed_host_good() {
        let ao = AllowedOrigins::localhost();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost"));
        assert!(ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_is_allowed_host_with_port() {
        let ao = AllowedOrigins::localhost();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost:8080"));
        assert!(ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_is_allowed_host_bad() {
        let ao = AllowedOrigins::localhost();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("evil.com"));
        assert!(!ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_is_allowed_host_missing() {
        let ao = AllowedOrigins::localhost();
        let headers = HeaderMap::new();
        assert!(!ao.is_allowed_host(&headers));
    }

    #[test]
    fn test_is_allowed_origin_good() {
        let ao = AllowedOrigins::from_bind_addr("127.0.0.1:8080".parse().unwrap());
        let origin = HeaderValue::from_static("http://localhost:8080");
        assert!(ao.is_allowed_origin(&origin));
    }

    #[test]
    fn test_is_allowed_origin_bad() {
        let ao = AllowedOrigins::localhost();
        let origin = HeaderValue::from_static("http://evil.com");
        assert!(!ao.is_allowed_origin(&origin));
    }

    // -- DnsRebindingService integration tests --

    #[tokio::test]
    async fn test_reject_bad_host() {
        let layer = DnsRebindingLayer::new(AllowedOrigins::localhost());
        let svc = layer.layer(ok_service());

        let req = Request::builder()
            .header(header::HOST, "evil.com")
            .body(Body::empty())
            .unwrap();

        let resp = svc.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_accept_good_host_no_origin() {
        let layer = DnsRebindingLayer::new(AllowedOrigins::localhost());
        let svc = layer.layer(ok_service());

        let req = Request::builder()
            .header(header::HOST, "localhost")
            .body(Body::empty())
            .unwrap();

        let resp = svc.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_reject_bad_origin() {
        let ao = AllowedOrigins::from_bind_addr("127.0.0.1:8080".parse().unwrap());
        let layer = DnsRebindingLayer::new(ao);
        let svc = layer.layer(ok_service());

        let req = Request::builder()
            .header(header::HOST, "localhost")
            .header(header::ORIGIN, "http://evil.com")
            .body(Body::empty())
            .unwrap();

        let resp = svc.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_accept_good_host_good_origin() {
        let ao = AllowedOrigins::from_bind_addr("127.0.0.1:8080".parse().unwrap());
        let layer = DnsRebindingLayer::new(ao);
        let svc = layer.layer(ok_service());

        let req = Request::builder()
            .header(header::HOST, "localhost")
            .header(header::ORIGIN, "http://localhost:8080")
            .body(Body::empty())
            .unwrap();

        let resp = svc.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_to_cors_allow_origin() {
        let ao = AllowedOrigins::from_bind_addr("127.0.0.1:8080".parse().unwrap());
        // Verify it returns an AllowOrigin (type check -- the inner value is opaque).
        let _cors: AllowOrigin = ao.to_cors_allow_origin();
    }

    #[tokio::test]
    async fn test_any_passes_through_layer() {
        let layer = DnsRebindingLayer::new(AllowedOrigins::any());
        let svc = layer.layer(ok_service());

        // External host + external origin — would fail with localhost()
        let req = Request::builder()
            .header(header::HOST, "api.example.com")
            .header(header::ORIGIN, "https://myapp.example.com")
            .body(Body::empty())
            .unwrap();

        let resp = svc.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
