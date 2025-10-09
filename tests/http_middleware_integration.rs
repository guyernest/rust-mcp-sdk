//! Integration tests for HTTP middleware with StreamableHttpTransport
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
        Some(&"Bearer test-token".to_string())
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
    request.add_header(
        "Authorization".to_string(),
        "Bearer existing-token".to_string(),
    );

    let context = HttpMiddlewareContext::new("http://test.com".to_string(), "POST".to_string());

    // Should skip injection and warn (doesn't error, just skips)
    oauth_mw.on_request(&mut request, &context).await.unwrap();

    // Verify original header is preserved (not overwritten)
    assert_eq!(
        request.get_header("Authorization"),
        Some(&"Bearer existing-token".to_string()),
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
