//! Comprehensive unit tests for `TransportAdapter` implementations.

#[cfg(test)]
#[allow(clippy::match_wildcard_for_single_variants)]
#[allow(clippy::significant_drop_in_scrutinee)]
mod tests {
    use crate::error::Result;
    use crate::server::adapters::{GenericTransportAdapter, TransportAdapter};
    use crate::server::builder::ServerCoreBuilder;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::core::ProtocolHandler;
    use crate::server::ToolHandler;
    use crate::shared::{Transport as TransportTrait, TransportMessage};
    use crate::types::*;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::fmt::Debug;
    use std::sync::Arc;

    #[cfg(target_arch = "wasm32")]
    use futures::lock::Mutex;
    #[cfg(not(target_arch = "wasm32"))]
    use tokio::sync::Mutex;

    // Mock transport for testing

    #[derive(Debug, Clone)]
    struct MockTransport {
        messages_to_receive: Arc<Mutex<VecDeque<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
        is_connected: Arc<Mutex<bool>>,
        close_called: Arc<Mutex<bool>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                messages_to_receive: Arc::new(Mutex::new(VecDeque::new())),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
                is_connected: Arc::new(Mutex::new(true)),
                close_called: Arc::new(Mutex::new(false)),
            }
        }

        async fn add_message_to_receive(&self, message: TransportMessage) {
            self.messages_to_receive.lock().await.push_back(message);
        }

        async fn get_sent_messages(&self) -> Vec<TransportMessage> {
            self.sent_messages.lock().await.clone()
        }

        async fn was_closed(&self) -> bool {
            *self.close_called.lock().await
        }

        #[allow(dead_code)]
        async fn disconnect(&self) {
            *self.is_connected.lock().await = false;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[async_trait]
    impl TransportTrait for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().await.push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            if let Some(message) = self.messages_to_receive.lock().await.pop_front() {
                Ok(message)
            } else {
                // Simulate disconnection when no more messages
                *self.is_connected.lock().await = false;
                Err(crate::error::Error::internal("No more messages"))
            }
        }

        async fn close(&mut self) -> Result<()> {
            *self.close_called.lock().await = true;
            *self.is_connected.lock().await = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            futures::executor::block_on(async { *self.is_connected.lock().await })
        }

        fn transport_type(&self) -> &'static str {
            "mock"
        }
    }

    // Test tool for handler
    struct EchoTool;

    #[async_trait]
    impl ToolHandler for EchoTool {
        async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(json!({ "echo": args }))
        }
    }

    // Helper functions

    fn create_test_handler() -> Arc<dyn ProtocolHandler> {
        Arc::new(
            ServerCoreBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .tool("echo", EchoTool)
                .build()
                .unwrap(),
        )
    }

    fn create_init_request() -> TransportMessage {
        TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "test-client".to_string(),
                    version: "1.0.0".to_string(),
                },
            }))),
        }
    }

    fn create_tool_call_request(id: i64, tool_name: &str) -> TransportMessage {
        TransportMessage::Request {
            id: RequestId::from(id),
            request: Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
                name: tool_name.to_string(),
                arguments: json!({ "test": "data" }),
                _meta: None,
                task: None,
            }))),
        }
    }

    // Test cases

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_request_response() {
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Add messages for the transport to receive
        transport
            .add_message_to_receive(create_init_request())
            .await;
        transport
            .add_message_to_receive(create_tool_call_request(2, "echo"))
            .await;

        let adapter = GenericTransportAdapter::new(transport);
        let handler = create_test_handler();

        // Run the adapter (will process messages and then close)
        let result = adapter.serve(handler).await;
        assert!(result.is_ok());

        // Verify responses were sent
        let sent_messages = transport_clone.get_sent_messages().await;
        assert_eq!(sent_messages.len(), 2);

        // Verify first response (initialization)
        match &sent_messages[0] {
            TransportMessage::Response(response) => {
                assert_eq!(response.id, RequestId::from(1i64));
                match &response.payload {
                    crate::types::jsonrpc::ResponsePayload::Result(_) => {
                        // Success
                    },
                    _ => panic!("Expected successful initialization response"),
                }
            },
            _ => panic!("Expected response message"),
        }

        // Verify second response (tool call)
        match &sent_messages[1] {
            TransportMessage::Response(response) => {
                assert_eq!(response.id, RequestId::from(2i64));
            },
            _ => panic!("Expected response message"),
        }

        // Verify transport was closed
        assert!(transport_clone.was_closed().await);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_notification_handling() {
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Add a notification message
        let notification =
            TransportMessage::Notification(Notification::Progress(ProgressNotification {
                progress_token: ProgressToken::String("test".to_string()),
                progress: 50.0,
                total: None,
                message: Some("Processing".to_string()),
            }));
        transport.add_message_to_receive(notification).await;

        let adapter = GenericTransportAdapter::new(transport);
        let handler = create_test_handler();

        // Run the adapter
        let result = adapter.serve(handler).await;
        assert!(result.is_ok());

        // Notifications shouldn't generate responses
        let sent_messages = transport_clone.get_sent_messages().await;
        assert_eq!(sent_messages.len(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_error_handling() {
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Add a request for a non-existent tool
        transport
            .add_message_to_receive(create_init_request())
            .await;
        transport
            .add_message_to_receive(create_tool_call_request(2, "nonexistent"))
            .await;

        let adapter = GenericTransportAdapter::new(transport);
        let handler = create_test_handler();

        // Run the adapter
        let result = adapter.serve(handler).await;
        assert!(result.is_ok());

        // Verify error response was sent
        let sent_messages = transport_clone.get_sent_messages().await;
        assert_eq!(sent_messages.len(), 2);

        // Check the error response
        match &sent_messages[1] {
            TransportMessage::Response(response) => {
                assert_eq!(response.id, RequestId::from(2i64));
                match &response.payload {
                    crate::types::jsonrpc::ResponsePayload::Error(error) => {
                        assert!(error.message.contains("not found"));
                    },
                    _ => panic!("Expected error response for non-existent tool"),
                }
            },
            _ => panic!("Expected response message"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_transport_type() {
        let transport = MockTransport::new();
        let adapter = GenericTransportAdapter::new(transport);
        assert_eq!(adapter.transport_type(), "generic");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_concurrent_messages() {
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Initialize first
        transport
            .add_message_to_receive(create_init_request())
            .await;

        // Add multiple tool call requests
        for i in 2..=10 {
            transport
                .add_message_to_receive(create_tool_call_request(i, "echo"))
                .await;
        }

        let adapter = GenericTransportAdapter::new(transport);
        let handler = create_test_handler();

        // Run the adapter
        let result = adapter.serve(handler).await;
        assert!(result.is_ok());

        // Verify all responses were sent
        let sent_messages = transport_clone.get_sent_messages().await;
        assert_eq!(sent_messages.len(), 10); // 1 init + 9 tool calls (2..=10)

        // Verify all responses have correct IDs
        for (i, message) in sent_messages.iter().enumerate() {
            match message {
                TransportMessage::Response(response) => {
                    assert_eq!(
                        response.id,
                        RequestId::from(i64::try_from(i + 1).expect("index fits in i64"))
                    );
                },
                _ => panic!("Expected response message"),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_generic_adapter_invalid_response_ignored() {
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Add a response message (servers shouldn't receive these)
        let response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: crate::types::jsonrpc::ResponsePayload::Result(json!({})),
        });
        transport.add_message_to_receive(response).await;

        let adapter = GenericTransportAdapter::new(transport);
        let handler = create_test_handler();

        // Run the adapter
        let result = adapter.serve(handler).await;
        assert!(result.is_ok());

        // No response should be sent for a received response
        let sent_messages = transport_clone.get_sent_messages().await;
        assert_eq!(sent_messages.len(), 0);
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn test_http_adapter_single_request() {
        use crate::server::adapters::HttpAdapter;

        let adapter = HttpAdapter::new();
        let handler = create_test_handler();

        // Create a request body
        let init_request = serde_json::to_string(&create_init_request()).unwrap();

        // Handle the request
        let response = adapter
            .handle_http_request(handler.clone(), init_request)
            .await
            .unwrap();

        // Parse the response
        let response_message: TransportMessage = serde_json::from_str(&response).unwrap();
        match response_message {
            TransportMessage::Response(resp) => {
                assert_eq!(resp.id, RequestId::from(1i64));
                match resp.payload {
                    crate::types::jsonrpc::ResponsePayload::Result(_) => {
                        // Success
                    },
                    _ => panic!("Expected successful response"),
                }
            },
            _ => panic!("Expected response message"),
        }
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn test_http_adapter_notification() {
        use crate::server::adapters::HttpAdapter;

        let adapter = HttpAdapter::new();
        let handler = create_test_handler();

        // Create a notification body
        let notification =
            TransportMessage::Notification(Notification::Progress(ProgressNotification {
                progress_token: ProgressToken::String("test".to_string()),
                progress: 50.0,
                total: None,
                message: Some("Processing".to_string()),
            }));
        let notification_body = serde_json::to_string(&notification).unwrap();

        // Handle the notification
        let response = adapter
            .handle_http_request(handler, notification_body)
            .await
            .unwrap();

        // Notifications should return empty response
        assert_eq!(response, "");
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn test_http_adapter_invalid_message() {
        use crate::server::adapters::HttpAdapter;

        let adapter = HttpAdapter::new();
        let handler = create_test_handler();

        // Try to send a response (invalid for server to receive)
        let response_msg = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: crate::types::jsonrpc::ResponsePayload::Result(json!({})),
        });
        let body = serde_json::to_string(&response_msg).unwrap();

        // Should get an error
        let result = adapter.handle_http_request(handler, body).await;
        assert!(result.is_err());
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn test_http_adapter_serve_not_implemented() {
        use crate::server::adapters::HttpAdapter;

        let adapter = HttpAdapter::new();
        let handler = create_test_handler();

        // The serve method should return an error for HTTP adapter
        let result = adapter.serve(handler).await;
        assert!(result.is_err());
    }

    // Mock adapter tests are already in the adapters.rs file
    // but we can add more comprehensive ones here

    #[allow(clippy::fallible_impl_from)]
    impl From<TransportMessage> for Request {
        fn from(msg: TransportMessage) -> Self {
            match msg {
                TransportMessage::Request { request, .. } => request,
                _ => panic!("Cannot convert non-request TransportMessage to Request"),
            }
        }
    }

    #[cfg(test)]
    mod mock_adapter_tests {
        use super::*;
        use crate::server::adapters::MockAdapter;

        #[tokio::test]
        async fn test_mock_adapter_multiple_requests() {
            let adapter = MockAdapter::new();
            let handler = create_test_handler();

            // Add multiple requests
            adapter
                .add_request(RequestId::from(1i64), create_init_request().into())
                .await;

            for i in 2..=5 {
                let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
                    name: "echo".to_string(),
                    arguments: json!({ "id": i }),
                    _meta: None,
                    task: None,
                })));
                adapter
                    .add_request(RequestId::from(i as i64), request)
                    .await;
            }

            // Serve all requests
            adapter.serve(handler).await.unwrap();

            // Get all responses
            let responses = adapter.get_responses().await;
            assert_eq!(responses.len(), 5);

            // Verify all responses have correct IDs
            for (i, response) in responses.iter().enumerate() {
                assert_eq!(
                    response.id,
                    RequestId::from(i64::try_from(i + 1).expect("index fits in i64"))
                );
            }
        }

        #[tokio::test]
        async fn test_mock_adapter_preserves_order() {
            let adapter = MockAdapter::new();
            let handler = create_test_handler();

            // Add requests in specific order
            let ids = vec![5i64, 3, 1, 4, 2];

            // Initialize first
            adapter
                .add_request(RequestId::from(0i64), create_init_request().into())
                .await;

            for id in &ids {
                let request =
                    Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
                        cursor: None,
                    })));
                adapter.add_request(RequestId::from(*id), request).await;
            }

            // Serve requests
            adapter.serve(handler).await.unwrap();

            // Get responses
            let responses = adapter.get_responses().await;
            assert_eq!(responses.len(), 6); // init + 5 requests

            // Verify order is preserved (excluding init)
            for (i, expected_id) in ids.iter().enumerate() {
                assert_eq!(responses[i + 1].id, RequestId::from(*expected_id));
            }
        }
    }
}
