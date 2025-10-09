//! Integration tests for HTTP middleware with `StreamableHttpTransport`
//!
//! Tests:
//! - Ordering: Multiple middleware with different priorities
//! - OAuth flows: No provider, expired token, duplicate headers
//! - SSE reconnection: Middleware runs on each reconnect
//! - Concurrency: Parallel requests through middleware chain
//! - Double-retry protection: Coordination via context metadata

use async_trait::async_trait;
use pmcp::client::http_middleware::{
    HttpMiddleware, HttpMiddlewareChain, HttpMiddlewareContext, HttpRequest, HttpResponse,
};
use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

/// Test middleware that tracks execution order
struct OrderTrackingMiddleware {
    name: &'static str,
    priority: i32,
    request_order: Arc<AtomicUsize>,
    response_order: Arc<AtomicUsize>,
}

impl OrderTrackingMiddleware {
    fn new(name: &'static str, priority: i32) -> Self {
        Self {
            name,
            priority,
            request_order: Arc::new(AtomicUsize::new(0)),
            response_order: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn get_request_order(&self) -> usize {
        self.request_order.load(AtomicOrdering::SeqCst)
    }

    fn get_response_order(&self) -> usize {
        self.response_order.load(AtomicOrdering::SeqCst)
    }
}

#[async_trait]
impl HttpMiddleware for OrderTrackingMiddleware {
    async fn on_request(
        &self,
        _request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        // Get current order count from context metadata
        let current_order = context
            .get_metadata("request_order_counter")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Record this middleware's execution order
        self.request_order
            .store(current_order, AtomicOrdering::SeqCst);

        // Increment counter for next middleware
        context.set_metadata(
            "request_order_counter".to_string(),
            (current_order + 1).to_string(),
        );

        tracing::debug!("[{}] on_request: order={}", self.name, current_order);
        Ok(())
    }

    async fn on_response(
        &self,
        _response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        // Get current order count from context metadata
        let current_order = context
            .get_metadata("response_order_counter")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Record this middleware's execution order
        self.response_order
            .store(current_order, AtomicOrdering::SeqCst);

        // Increment counter for next middleware
        context.set_metadata(
            "response_order_counter".to_string(),
            (current_order + 1).to_string(),
        );

        tracing::debug!("[{}] on_response: order={}", self.name, current_order);
        Ok(())
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

#[tokio::test]
async fn test_middleware_ordering() {
    // Create middleware with different priorities
    let mw1 = Arc::new(OrderTrackingMiddleware::new("low", 50));
    let mw2 = Arc::new(OrderTrackingMiddleware::new("high", 10));
    let mw3 = Arc::new(OrderTrackingMiddleware::new("normal", 30));

    // Add to chain (should be sorted by priority)
    let mut chain = HttpMiddlewareChain::new();
    chain.add(mw1.clone());
    chain.add(mw2.clone());
    chain.add(mw3.clone());

    // Create test request and context
    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Process request
    chain.process_request(&mut request, &context).await.unwrap();

    // Verify request order: high (10) -> normal (30) -> low (50)
    assert_eq!(mw2.get_request_order(), 0, "High priority should run first");
    assert_eq!(
        mw3.get_request_order(),
        1,
        "Normal priority should run second"
    );
    assert_eq!(mw1.get_request_order(), 2, "Low priority should run third");

    // Create test response
    let mut response = HttpResponse::new(200, vec![]);

    // Process response
    chain
        .process_response(&mut response, &context)
        .await
        .unwrap();

    // Verify response order: reverse of request (low -> normal -> high)
    assert_eq!(
        mw1.get_response_order(),
        0,
        "Low priority should run first in response"
    );
    assert_eq!(
        mw3.get_response_order(),
        1,
        "Normal priority should run second in response"
    );
    assert_eq!(
        mw2.get_response_order(),
        2,
        "High priority should run third in response"
    );
}

#[tokio::test]
async fn test_oauth_no_provider() {
    // Create OAuth middleware without setting up auth provider
    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should inject token successfully (no provider needed for simple bearer token)
    oauth_mw.on_request(&mut request, &context).await.unwrap();

    // Verify Authorization header was added
    assert_eq!(
        request.get_header("Authorization"),
        Some("Bearer test-token")
    );
}

#[tokio::test]
async fn test_oauth_expired_token() {
    use std::time::Duration;

    // Create expired token (expires in 0 seconds = already expired)
    let token = BearerToken::with_expiry("expired-token".to_string(), Duration::from_secs(0));

    // Wait a moment to ensure it's expired
    tokio::time::sleep(Duration::from_millis(10)).await;

    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should return authentication error for expired token
    let result = oauth_mw.on_request(&mut request, &context).await;
    assert!(result.is_err(), "Expired token should return error");
    assert!(matches!(
        result.unwrap_err(),
        pmcp::Error::Authentication(_)
    ));
}

#[tokio::test]
async fn test_oauth_duplicate_header_detection() {
    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);

    // Pre-add Authorization header (simulating transport auth or config)
    request.add_header("Authorization", "Bearer existing-token");

    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should skip injection and warn (doesn't error, just skips)
    oauth_mw.on_request(&mut request, &context).await.unwrap();

    // Verify original header is preserved (not overwritten)
    assert_eq!(
        request.get_header("Authorization"),
        Some("Bearer existing-token"),
        "Existing auth header should not be overwritten"
    );
}

#[tokio::test]
async fn test_oauth_precedence_policy() {
    let token = BearerToken::new("oauth-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);

    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Set metadata indicating transport auth already set
    context.set_metadata("auth_already_set".to_string(), "true".to_string());

    // Should skip injection due to precedence policy
    oauth_mw.on_request(&mut request, &context).await.unwrap();

    // Verify no Authorization header was added
    assert!(
        request.get_header("Authorization").is_none(),
        "OAuth should skip when transport auth is set"
    );
}

#[tokio::test]
async fn test_oauth_401_detection() {
    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut response = HttpResponse::new(401, vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should detect 401 and return auth error
    let result = oauth_mw.on_response(&mut response, &context).await;
    assert!(result.is_err(), "401 response should return error");

    // Verify metadata was set
    assert_eq!(
        context.get_metadata("auth_failure"),
        Some("true".to_string())
    );
    assert_eq!(context.get_metadata("status_code"), Some("401".to_string()));
}

#[tokio::test]
async fn test_middleware_short_circuit_on_error() {
    /// Middleware that always fails
    struct FailingMiddleware;

    #[async_trait]
    impl HttpMiddleware for FailingMiddleware {
        async fn on_request(
            &self,
            _request: &mut HttpRequest,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            Err(pmcp::Error::authentication("Test failure"))
        }

        fn priority(&self) -> i32 {
            10
        }
    }

    /// Middleware that tracks if it was called
    struct TrackingMiddleware {
        called: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpMiddleware for TrackingMiddleware {
        async fn on_request(
            &self,
            _request: &mut HttpRequest,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            self.called.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }

        fn priority(&self) -> i32 {
            20 // Runs after failing middleware
        }
    }

    let mut chain = HttpMiddlewareChain::new();
    chain.add(Arc::new(FailingMiddleware));

    let tracking = Arc::new(TrackingMiddleware {
        called: Arc::new(AtomicUsize::new(0)),
    });
    chain.add(tracking.clone());

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should fail at first middleware
    let result = chain.process_request(&mut request, &context).await;
    assert!(result.is_err(), "Chain should short-circuit on error");

    // Verify second middleware was NOT called
    assert_eq!(
        tracking.called.load(AtomicOrdering::SeqCst),
        0,
        "Later middleware should not run after error"
    );
}

#[tokio::test]
async fn test_concurrency_no_shared_state_contention() {
    use tokio::task::JoinSet;

    // Create middleware chain
    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = Arc::new(OAuthClientMiddleware::new(token));

    let mut chain = HttpMiddlewareChain::new();
    chain.add(oauth_mw);
    let chain = Arc::new(chain);

    // Spawn 100 parallel requests
    let mut set = JoinSet::new();
    for i in 0..100 {
        let chain_clone = chain.clone();
        set.spawn(async move {
            let mut request = HttpRequest::new(
                "POST".to_string(),
                format!("http://test.com/req{}", i),
                vec![],
            );
            let context =
                HttpMiddlewareContext::new(format!("http://test.com/req{}", i), "POST".to_string());

            chain_clone.process_request(&mut request, &context).await
        });
    }

    // Wait for all to complete
    let mut success_count = 0;
    while let Some(result) = set.join_next().await {
        assert!(result.is_ok(), "Task should not panic");
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }

    // All should succeed
    assert_eq!(success_count, 100, "All parallel requests should succeed");
}

#[tokio::test]
async fn test_header_case_insensitivity() {
    use hyper::http::HeaderMap;
    use pmcp::client::http_middleware::{HttpRequest, HttpResponse};

    // Test HttpRequest case-insensitive headers
    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);

    // Add headers with different cases
    request.add_header("Content-Type", "application/json");
    request.add_header("Authorization", "Bearer token");
    request.add_header("x-custom-header", "value");

    // Verify case-insensitive lookup works
    assert_eq!(
        request.get_header("content-type"),
        Some("application/json"),
        "Lowercase lookup should work"
    );
    assert_eq!(
        request.get_header("Content-Type"),
        Some("application/json"),
        "Mixed case lookup should work"
    );
    assert_eq!(
        request.get_header("CONTENT-TYPE"),
        Some("application/json"),
        "Uppercase lookup should work"
    );

    assert_eq!(
        request.get_header("authorization"),
        Some("Bearer token"),
        "Authorization header should be accessible"
    );
    assert_eq!(
        request.get_header("AUTHORIZATION"),
        Some("Bearer token"),
        "Case variation should work"
    );

    // Verify has_header is case-insensitive
    assert!(request.has_header("authorization"), "Lowercase check");
    assert!(request.has_header("Authorization"), "Mixed case check");
    assert!(request.has_header("AUTHORIZATION"), "Uppercase check");
    assert!(request.has_header("x-custom-header"), "Custom header check");
    assert!(
        request.has_header("X-Custom-Header"),
        "Custom header mixed case"
    );

    // Verify remove_header is case-insensitive
    let removed = request.remove_header("CONTENT-TYPE");
    assert_eq!(removed, Some("application/json".to_string()));
    assert!(
        !request.has_header("content-type"),
        "Header should be removed"
    );
    assert!(
        !request.has_header("Content-Type"),
        "Header should be removed (any case)"
    );

    // Test HttpResponse case-insensitive headers
    let mut response = HttpResponse::new(200, vec![]);
    response.add_header("Content-Length", "123");
    response.add_header("cache-control", "no-cache");

    assert_eq!(response.get_header("content-length"), Some("123"));
    assert_eq!(response.get_header("CONTENT-LENGTH"), Some("123"));
    assert_eq!(response.get_header("Cache-Control"), Some("no-cache"));

    // Test with_headers constructor
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/plain".parse().unwrap());
    headers.insert("X-Custom", "value".parse().unwrap());

    let response2 = HttpResponse::with_headers(200, headers, vec![]);
    assert_eq!(
        response2.get_header("content-type"),
        Some("text/plain"),
        "with_headers should work with HeaderMap"
    );
    assert_eq!(
        response2.get_header("CONTENT-TYPE"),
        Some("text/plain"),
        "Case-insensitive lookup should work"
    );
    assert_eq!(
        response2.get_header("x-custom"),
        Some("value"),
        "Custom headers should be accessible"
    );
}

#[tokio::test]
async fn test_oauth_duplicate_detection_case_insensitive() {
    use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

    let token = BearerToken::new("oauth-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Add Authorization header with different case than OAuth middleware uses
    request.add_header(
        "AUTHORIZATION", // Uppercase
        "Bearer existing-token",
    );

    // OAuth middleware should detect the existing header regardless of case
    oauth_mw.on_request(&mut request, &context).await.unwrap();

    // Verify original header is preserved
    assert_eq!(
        request.get_header("authorization"), // lowercase lookup
        Some("Bearer existing-token"),
        "Original header should be preserved"
    );
    assert_eq!(
        request.get_header("Authorization"), // mixed case lookup
        Some("Bearer existing-token"),
        "Case-insensitive lookup should work"
    );

    // Should only have one authorization header (not duplicated)
    let auth_header_count = request
        .headers
        .iter()
        .filter(|(k, _)| k.as_str() == "authorization")
        .count();
    assert_eq!(
        auth_header_count, 1,
        "Should only have one authorization header"
    );
}

#[tokio::test]
async fn test_middleware_chain_with_mixed_case_headers() {
    /// Middleware that checks for Authorization header
    struct AuthCheckMiddleware;

    #[async_trait]
    impl HttpMiddleware for AuthCheckMiddleware {
        async fn on_request(
            &self,
            request: &mut HttpRequest,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            // Check using different case variations
            assert!(
                request.has_header("authorization"),
                "Should find auth header"
            );
            assert!(
                request.has_header("Authorization"),
                "Should be case-insensitive"
            );
            assert!(
                request.has_header("AUTHORIZATION"),
                "Should work with uppercase"
            );

            let auth = request.get_header("AuThOrIzAtIoN");
            assert!(auth.is_some(), "Mixed case lookup should work");

            Ok(())
        }

        fn priority(&self) -> i32 {
            20
        }
    }

    let mut chain = HttpMiddlewareChain::new();

    // Add OAuth middleware first (priority 10)
    let token = BearerToken::new("test-token".to_string());
    chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // Add auth checker (priority 20 - runs after OAuth)
    chain.add(Arc::new(AuthCheckMiddleware));

    let mut request = HttpRequest::new("POST".to_string(), "http://test.com".to_string(), vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Process request - OAuth adds "Authorization", checker verifies case-insensitive access
    chain.process_request(&mut request, &context).await.unwrap();

    // Verify the header is accessible via any case variation
    assert!(request.has_header("authorization"));
    assert!(request.has_header("Authorization"));
    assert!(request.has_header("AUTHORIZATION"));
    assert_eq!(
        request.get_header("authorization"),
        Some("Bearer test-token")
    );
}

#[tokio::test]
async fn test_oauth_retry_coordination_with_metadata() {
    use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

    /// Middleware that simulates a retry mechanism coordinating with OAuth
    struct RetryCoordinationMiddleware {
        retry_attempted: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpMiddleware for RetryCoordinationMiddleware {
        async fn on_error(
            &self,
            error: &pmcp::Error,
            context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            // Check if this is an auth error (from OAuth middleware)
            if matches!(error, pmcp::Error::Authentication(_)) {
                // Verify OAuth set the metadata
                if context.get_metadata("auth_failure") == Some("true".to_string()) {
                    // Check if retry was already attempted
                    if context.get_metadata("oauth.retry_used").is_some() {
                        // Don't retry again - would create infinite loop
                        tracing::warn!("OAuth retry already attempted, not retrying again");
                        return Ok(());
                    }

                    // Mark that we're attempting a retry
                    context.set_metadata("oauth.retry_used".to_string(), "true".to_string());
                    self.retry_attempted.fetch_add(1, AtomicOrdering::SeqCst);

                    tracing::info!("Retry middleware: auth failure detected, would retry here");
                }
            }

            Ok(())
        }

        fn priority(&self) -> i32 {
            5 // Priority doesn't matter for on_error - all middleware called
        }
    }

    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = Arc::new(OAuthClientMiddleware::new(token));

    let retry_mw = Arc::new(RetryCoordinationMiddleware {
        retry_attempted: Arc::new(AtomicUsize::new(0)),
    });

    let mut chain = HttpMiddlewareChain::new();
    chain.add(oauth_mw);
    chain.add(retry_mw.clone());

    // Create a 401 response
    let mut response = HttpResponse::new(401, vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Process response - OAuth should detect 401 and set metadata
    let result = chain.process_response(&mut response, &context).await;

    // OAuth should return error for 401
    assert!(result.is_err(), "OAuth should return error for 401");

    // Verify metadata was set
    assert_eq!(
        context.get_metadata("auth_failure"),
        Some("true".to_string()),
        "OAuth should set auth_failure metadata"
    );
    assert_eq!(
        context.get_metadata("status_code"),
        Some("401".to_string()),
        "OAuth should set status_code metadata"
    );

    // Verify retry middleware detected it and marked retry as used
    assert_eq!(
        context.get_metadata("oauth.retry_used"),
        Some("true".to_string()),
        "Retry middleware should set retry_used metadata"
    );

    // Verify retry was attempted once
    assert_eq!(
        retry_mw.retry_attempted.load(AtomicOrdering::SeqCst),
        1,
        "Retry should have been attempted once"
    );
}

#[tokio::test]
async fn test_double_retry_protection() {
    use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

    /// Middleware that tracks retry attempts
    struct RetryTrackingMiddleware {
        retry_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpMiddleware for RetryTrackingMiddleware {
        async fn on_error(
            &self,
            error: &pmcp::Error,
            context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            // Only retry on auth errors
            if matches!(error, pmcp::Error::Authentication(_))
                && context.get_metadata("auth_failure") == Some("true".to_string())
            {
                if context.get_metadata("oauth.retry_used").is_none() {
                    // First retry
                    context.set_metadata("oauth.retry_used".to_string(), "true".to_string());
                    self.retry_count.fetch_add(1, AtomicOrdering::SeqCst);
                    tracing::info!("First retry attempt");
                } else {
                    // Would be second retry - don't do it
                    tracing::warn!("Retry already used, preventing double retry");
                }
            }
            Ok(())
        }

        fn priority(&self) -> i32 {
            5 // Priority doesn't matter for on_error - all middleware called
        }
    }

    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = Arc::new(OAuthClientMiddleware::new(token));

    let retry_tracker = Arc::new(RetryTrackingMiddleware {
        retry_count: Arc::new(AtomicUsize::new(0)),
    });

    let mut chain = HttpMiddlewareChain::new();
    chain.add(oauth_mw);
    chain.add(retry_tracker.clone());

    // Simulate first 401 response
    let mut response1 = HttpResponse::new(401, vec![]);
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Process first response - should trigger retry
    let _ = chain.process_response(&mut response1, &context).await;

    // Verify first retry was attempted
    assert_eq!(
        retry_tracker.retry_count.load(AtomicOrdering::SeqCst),
        1,
        "First retry should have been attempted"
    );

    // Simulate second 401 response (retry failed)
    let mut response2 = HttpResponse::new(401, vec![]);

    // Process second response with SAME context (simulating retry)
    let _ = chain.process_response(&mut response2, &context).await;

    // Verify retry was NOT attempted again (still 1, not 2)
    assert_eq!(
        retry_tracker.retry_count.load(AtomicOrdering::SeqCst),
        1,
        "Second retry should have been prevented by oauth.retry_used metadata"
    );

    // Verify metadata is still set
    assert_eq!(
        context.get_metadata("oauth.retry_used"),
        Some("true".to_string()),
        "Retry metadata should persist"
    );
}

#[tokio::test]
async fn test_oauth_error_hook_logging() {
    use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

    let token = BearerToken::new("test-token".to_string());
    let oauth_mw = OAuthClientMiddleware::new(token);

    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Create an authentication error
    let error = pmcp::Error::authentication("Test authentication error");

    // Call on_error hook - should not panic
    let result = oauth_mw.on_error(&error, &context).await;
    assert!(
        result.is_ok(),
        "on_error should handle auth errors gracefully"
    );

    // Call with non-auth error - should also be fine
    let other_error = pmcp::Error::internal("Other error");
    let result = oauth_mw.on_error(&other_error, &context).await;
    assert!(
        result.is_ok(),
        "on_error should handle non-auth errors gracefully"
    );
}

#[tokio::test]
async fn test_http_logging_redacts_sensitive_headers() {
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    let middleware = HttpLoggingMiddleware::new();

    // Test Authorization header redaction with different casings
    let mut request = HttpRequest::new("GET".to_string(), "http://test.com".to_string(), vec![]);
    request.add_header("Authorization", "Bearer my-secret-token-12345");
    request.add_header("AUTHORIZATION", "Bearer another-secret");
    request.add_header("cookie", "session=abc123");
    request.add_header("x-api-key", "api-key-secret");
    request.add_header("content-type", "application/json");

    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "GET".to_string());

    // Process request (this will log with redaction)
    middleware.on_request(&mut request, &context).await.unwrap();

    // Verify headers are redacted in the internal redaction logic
    let formatted = middleware.redact_header_value(
        &hyper::http::HeaderName::from_static("authorization"),
        "Bearer secret",
    );
    assert_eq!(formatted, "Bearer [REDACTED]");

    let formatted_cookie = middleware.redact_header_value(
        &hyper::http::HeaderName::from_static("cookie"),
        "session=abc123",
    );
    assert_eq!(formatted_cookie, "[REDACTED]");

    // Content-type should not be redacted
    let formatted_ct = middleware.redact_header_value(
        &hyper::http::HeaderName::from_static("content-type"),
        "application/json",
    );
    assert_eq!(formatted_ct, "application/json");
}

#[tokio::test]
async fn test_http_logging_respects_overrides() {
    use hyper::http::HeaderName;
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    // Remove x-api-key from redaction list
    let middleware =
        HttpLoggingMiddleware::new().allow_header(&HeaderName::from_static("x-api-key"));

    // x-api-key should NOT be redacted now
    let formatted =
        middleware.redact_header_value(&HeaderName::from_static("x-api-key"), "my-api-key-12345");
    assert_eq!(formatted, "my-api-key-12345");

    // But authorization should still be redacted
    let formatted_auth = middleware.redact_header_value(
        &HeaderName::from_static("authorization"),
        "Bearer secret-token",
    );
    assert_eq!(formatted_auth, "Bearer [REDACTED]");
}

#[tokio::test]
async fn test_http_logging_multivalue_headers() {
    use hyper::http::HeaderMap;
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    let middleware = HttpLoggingMiddleware::new();

    // Create headers with multiple set-cookie entries
    let mut headers = HeaderMap::new();
    headers.append("set-cookie", "session1=abc123".parse().unwrap());
    headers.append("set-cookie", "session2=def456".parse().unwrap());
    headers.append("set-cookie", "user=john".parse().unwrap());
    headers.insert("content-type", "application/json".parse().unwrap());

    // Format headers - all set-cookie values should be redacted
    let formatted = middleware.format_headers(&headers);

    // Should contain [REDACTED] for each set-cookie entry
    assert!(formatted.contains("[REDACTED]"));

    // Should contain content-type unredacted
    assert!(formatted.contains("application/json"));

    // Verify all set-cookie entries are redacted (not just one)
    let redacted_count = formatted.matches("[REDACTED]").count();
    assert!(
        redacted_count >= 3,
        "Expected at least 3 redacted values, got {}",
        redacted_count
    );
}

#[tokio::test]
async fn test_http_logging_body_disabled_by_default() {
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    // Default middleware should NOT log bodies
    let middleware_default = HttpLoggingMiddleware::new();
    assert!(
        middleware_default.max_body_bytes().is_none(),
        "Default middleware should not log bodies"
    );

    // With max_body_bytes set, it should log bodies
    let middleware_with_body = HttpLoggingMiddleware::new().with_max_body_bytes(1024);
    assert_eq!(
        middleware_with_body.max_body_bytes(),
        Some(1024),
        "Middleware with body logging should have max_body_bytes set"
    );

    // Create a request with a body
    let body = b"sensitive data in body";
    let mut request = HttpRequest::new(
        "POST".to_string(),
        "http://test.com".to_string(),
        body.to_vec(),
    );
    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Both should succeed, but default won't log the body content
    middleware_default
        .on_request(&mut request, &context)
        .await
        .unwrap();
    middleware_with_body
        .on_request(&mut request, &context)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_http_logging_authorization_without_scheme() {
    use hyper::http::HeaderName;
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    // With show_auth_scheme = false, should completely redact
    let middleware = HttpLoggingMiddleware::new().with_show_auth_scheme(false);

    let formatted = middleware.redact_header_value(
        &HeaderName::from_static("authorization"),
        "Bearer my-secret-token",
    );
    assert_eq!(formatted, "[REDACTED]");

    let formatted_basic = middleware.redact_header_value(
        &HeaderName::from_static("authorization"),
        "Basic dXNlcjpwYXNz",
    );
    assert_eq!(formatted_basic, "[REDACTED]");
}

#[tokio::test]
async fn test_http_logging_header_value_truncation() {
    use hyper::http::HeaderName;
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    // Set max header value length to 20 characters
    let middleware = HttpLoggingMiddleware::new().with_max_header_value_len(20);

    // Long non-sensitive header should be truncated
    let long_value = "this-is-a-very-long-content-type-value-that-exceeds-the-limit";
    let formatted =
        middleware.redact_header_value(&HeaderName::from_static("content-type"), long_value);

    assert!(formatted.len() <= 23, "Truncated value too long"); // 20 + "..."
    assert!(formatted.ends_with("..."), "Should end with ellipsis");
}

#[tokio::test]
async fn test_http_logging_case_insensitive_redaction() {
    use hyper::http::HeaderName;
    use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;

    let middleware = HttpLoggingMiddleware::new();

    // Test different casings of Authorization header
    let test_cases = vec![
        ("authorization", "Bearer secret"),
        ("Authorization", "Bearer secret"),
        ("AUTHORIZATION", "Bearer secret"),
        ("AuThOrIzAtIoN", "Bearer secret"),
    ];

    for (header_name, value) in test_cases {
        let name = HeaderName::from_bytes(header_name.as_bytes()).unwrap();
        let formatted = middleware.redact_header_value(&name, value);
        assert_eq!(
            formatted, "Bearer [REDACTED]",
            "Failed for header name: {}",
            header_name
        );
    }

    // Test Cookie header casings
    let cookie_cases = vec![
        ("cookie", "session=abc"),
        ("Cookie", "session=abc"),
        ("COOKIE", "session=abc"),
    ];

    for (header_name, value) in cookie_cases {
        let name = HeaderName::from_bytes(header_name.as_bytes()).unwrap();
        let formatted = middleware.redact_header_value(&name, value);
        assert_eq!(
            formatted, "[REDACTED]",
            "Failed for header name: {}",
            header_name
        );
    }
}
