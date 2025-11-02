//! Protocol middleware ordering tests for server.
//!
//! These tests lock in the execution order contract:
//! - **Request**: lower priority runs first (Critical → ... → Lowest)
//! - **Response**: reverse order (Lowest → ... → Critical)
//! - **Notification**: same as request (lower priority first)

use pmcp::error::Result;
use pmcp::runtime::RwLock;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::shared::middleware::{
    AdvancedMiddleware, EnhancedMiddlewareChain, MiddlewareContext, MiddlewarePriority,
};
use pmcp::types::{
    ClientRequest, InitializeParams, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
    Notification, ProgressNotification, ProgressToken, Request, RequestId,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Tracking middleware that records execution order
#[derive(Debug)]
struct OrderTrackingMiddleware {
    name: String,
    priority: MiddlewarePriority,
    request_order: Arc<Mutex<Vec<String>>>,
    response_order: Arc<Mutex<Vec<String>>>,
    notification_order: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl AdvancedMiddleware for OrderTrackingMiddleware {
    fn priority(&self) -> MiddlewarePriority {
        self.priority
    }

    async fn on_request_with_context(
        &self,
        _request: &mut JSONRPCRequest,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        self.request_order.lock().await.push(self.name.clone());
        Ok(())
    }

    async fn on_response_with_context(
        &self,
        _response: &mut JSONRPCResponse,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        self.response_order.lock().await.push(self.name.clone());
        Ok(())
    }

    async fn on_notification_with_context(
        &self,
        _notification: &mut JSONRPCNotification,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        self.notification_order.lock().await.push(self.name.clone());
        Ok(())
    }
}

#[tokio::test]
async fn test_protocol_middleware_request_ordering() {
    // Request order: lower priority runs first (Critical → ... → Lowest)
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));
    let notification_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = EnhancedMiddlewareChain::new();

    // Add middleware in random order, verify they execute by priority
    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Normal".to_string(),
        priority: MiddlewarePriority::Normal,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Critical".to_string(),
        priority: MiddlewarePriority::Critical,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Lowest".to_string(),
        priority: MiddlewarePriority::Lowest,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "High".to_string(),
        priority: MiddlewarePriority::High,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    let context = MiddlewareContext::default();
    let mut request = JSONRPCRequest::new(
        RequestId::from(1i64),
        "test/method",
        Some(serde_json::json!({})),
    );

    // Process request
    chain
        .process_request_with_context(&mut request, &context)
        .await
        .unwrap();

    // Verify request order: Critical → High → Normal → Lowest
    let order = request_order.lock().await;
    assert_eq!(
        *order,
        vec!["Critical", "High", "Normal", "Lowest"],
        "Request middleware must execute in priority order (lower priority value first)"
    );
}

#[tokio::test]
async fn test_protocol_middleware_response_ordering() {
    // Response order: reverse of request (Lowest → ... → Critical)
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));
    let notification_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = EnhancedMiddlewareChain::new();

    // Add middleware with different priorities
    for (name, priority) in [
        ("Critical", MiddlewarePriority::Critical),
        ("High", MiddlewarePriority::High),
        ("Normal", MiddlewarePriority::Normal),
        ("Lowest", MiddlewarePriority::Lowest),
    ] {
        chain.add(Arc::new(OrderTrackingMiddleware {
            name: name.to_string(),
            priority,
            request_order: request_order.clone(),
            response_order: response_order.clone(),
            notification_order: notification_order.clone(),
        }));
    }

    let context = MiddlewareContext::default();
    let mut response =
        JSONRPCResponse::success(RequestId::from(1i64), serde_json::json!({"result": "test"}));

    // Process response
    chain
        .process_response_with_context(&mut response, &context)
        .await
        .unwrap();

    // Verify response order: Lowest → Normal → High → Critical (reverse of request)
    let order = response_order.lock().await;
    assert_eq!(
        *order,
        vec!["Lowest", "Normal", "High", "Critical"],
        "Response middleware must execute in reverse priority order (highest priority value first)"
    );
}

#[tokio::test]
async fn test_protocol_middleware_notification_ordering() {
    // Notification order: same as request (lower priority first)
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));
    let notification_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = EnhancedMiddlewareChain::new();

    // Add middleware in non-sequential priority order
    for (name, priority) in [
        ("Normal", MiddlewarePriority::Normal),
        ("Critical", MiddlewarePriority::Critical),
        ("Lowest", MiddlewarePriority::Lowest),
        ("High", MiddlewarePriority::High),
    ] {
        chain.add(Arc::new(OrderTrackingMiddleware {
            name: name.to_string(),
            priority,
            request_order: request_order.clone(),
            response_order: response_order.clone(),
            notification_order: notification_order.clone(),
        }));
    }

    let context = MiddlewareContext::default();
    let mut notification = JSONRPCNotification::new(
        "notifications/progress",
        Some(serde_json::json!({"progress": 50})),
    );

    // Process notification
    chain
        .process_notification_with_context(&mut notification, &context)
        .await
        .unwrap();

    // Verify notification order: Critical → High → Normal → Lowest (same as request)
    let order = notification_order.lock().await;
    assert_eq!(
        *order,
        vec!["Critical", "High", "Normal", "Lowest"],
        "Notification middleware must execute in priority order (same as request)"
    );
}

#[tokio::test]
async fn test_server_protocol_middleware_integration() {
    // Verify protocol middleware integrates correctly with ServerCore
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));
    let notification_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = EnhancedMiddlewareChain::new();

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Auth".to_string(),
        priority: MiddlewarePriority::Critical, // Auth runs first
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Logging".to_string(),
        priority: MiddlewarePriority::High, // Logging runs after auth
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Metrics".to_string(),
        priority: MiddlewarePriority::Normal, // Metrics runs after logging
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    let server = ServerCoreBuilder::new()
        .name("test-server")
        .version("1.0.0")
        .protocol_middleware(Arc::new(RwLock::new(chain)))
        .build()
        .unwrap();

    // Send a request through the server (will trigger middleware)
    let init_request = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
        protocol_version: pmcp::DEFAULT_PROTOCOL_VERSION.to_string(),
        capabilities: pmcp::types::ClientCapabilities::default(),
        client_info: pmcp::types::Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
        },
    })));

    let _response = server
        .handle_request(RequestId::from(1i64), init_request, None)
        .await;

    // Verify request order: Auth → Logging → Metrics
    let req_order = request_order.lock().await;
    assert_eq!(
        *req_order,
        vec!["Auth", "Logging", "Metrics"],
        "Server request middleware must execute in priority order"
    );

    // Verify response order: Metrics → Logging → Auth (reverse)
    let resp_order = response_order.lock().await;
    assert_eq!(
        *resp_order,
        vec!["Metrics", "Logging", "Auth"],
        "Server response middleware must execute in reverse priority order"
    );
}

#[tokio::test]
async fn test_server_notification_middleware_integration() {
    // Verify notification middleware integrates correctly
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));
    let notification_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = EnhancedMiddlewareChain::new();

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Validation".to_string(),
        priority: MiddlewarePriority::Critical,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    chain.add(Arc::new(OrderTrackingMiddleware {
        name: "Logging".to_string(),
        priority: MiddlewarePriority::High,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
        notification_order: notification_order.clone(),
    }));

    let server = ServerCoreBuilder::new()
        .name("test-server")
        .version("1.0.0")
        .protocol_middleware(Arc::new(RwLock::new(chain)))
        .build()
        .unwrap();

    // Send a notification through the server
    let notification = Notification::Progress(ProgressNotification {
        progress_token: ProgressToken::String("test-123".to_string()),
        progress: 50.0,
        total: None,
        message: Some("Test progress".to_string()),
    });

    let _ = server.handle_notification(notification).await;

    // Verify notification order: Validation → Logging (same as request)
    let notif_order = notification_order.lock().await;
    assert_eq!(
        *notif_order,
        vec!["Validation", "Logging"],
        "Server notification middleware must execute in priority order (same as request)"
    );
}
