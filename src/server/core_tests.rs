//! Comprehensive unit tests for the `ServerCore` implementation.

#[cfg(test)]
#[allow(clippy::match_wildcard_for_single_variants)]
mod tests {
    use crate::error::{Error, Result};
    use crate::server::builder::ServerCoreBuilder;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::core::{ProtocolHandler, ServerCore};
    use crate::server::{PromptHandler, ResourceHandler, ToolHandler};
    use crate::types::ResourceInfo;
    use crate::types::*;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Test fixtures

    /// Mock tool that tracks invocations
    struct MockTool {
        invocation_count: Arc<AtomicUsize>,
        should_fail: bool,
        return_value: Value,
    }

    impl MockTool {
        fn new() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: false,
                return_value: json!({"status": "ok"}),
            }
        }

        fn with_return(value: Value) -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: false,
                return_value: value,
            }
        }

        fn failing() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: true,
                return_value: json!({}),
            }
        }

        fn invocation_count(&self) -> usize {
            self.invocation_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ToolHandler for MockTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            self.invocation_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(Error::internal("Mock tool error"))
            } else {
                Ok(self.return_value.clone())
            }
        }
    }

    /// Mock prompt handler
    struct MockPromptHandler {
        invocation_count: Arc<AtomicUsize>,
    }

    impl MockPromptHandler {
        fn new() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl PromptHandler for MockPromptHandler {
        async fn handle(
            &self,
            args: HashMap<String, String>,
            _extra: RequestHandlerExtra,
        ) -> Result<GetPromptResult> {
            self.invocation_count.fetch_add(1, Ordering::SeqCst);
            Ok(GetPromptResult {
                description: Some("Test prompt".to_string()),
                messages: vec![PromptMessage::user(Content::text(format!(
                    "Prompt with args: {:?}",
                    args
                )))],
                _meta: None,
            })
        }
    }

    /// Mock resource handler
    struct MockResourceHandler {
        resources: Vec<ResourceInfo>,
    }

    impl MockResourceHandler {
        fn new() -> Self {
            Self {
                resources: vec![ResourceInfo::new("test://resource1", "Resource 1")
                    .with_description("Test resource 1")
                    .with_mime_type("text/plain")],
            }
        }
    }

    #[async_trait]
    impl ResourceHandler for MockResourceHandler {
        async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
            if uri == "test://resource1" {
                Ok(ReadResourceResult::new(vec![Content::text(
                    "Resource content",
                )]))
            } else {
                Err(Error::internal(format!("Resource not found: {}", uri)))
            }
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<ListResourcesResult> {
            Ok(ListResourcesResult {
                resources: self.resources.clone(),
                next_cursor: None,
            })
        }
    }

    // Helper functions

    fn create_test_server() -> ServerCore {
        ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap()
    }

    fn create_init_request() -> Request {
        Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2025-06-18".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })))
    }

    // Test cases

    #[tokio::test]
    async fn test_server_initialization() {
        let server = create_test_server();

        // Server should not be initialized initially
        assert!(!server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_none());

        // Send initialization request
        let response = server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Verify successful response
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let init_result: InitializeResult = serde_json::from_value(result).unwrap();
                assert_eq!(
                    init_result.protocol_version,
                    ProtocolVersion("2025-06-18".to_string())
                );
                assert_eq!(init_result.server_info.name, "test-server");
                assert_eq!(init_result.server_info.version, "1.0.0");
            },
            _ => panic!("Expected successful initialization"),
        }

        // Server should be initialized now
        assert!(server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_some());
    }

    #[tokio::test]
    async fn test_request_before_initialization() {
        let server = create_test_server();

        // Try to call a tool before initialization
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Should get an error
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert_eq!(error.code, -32002);
                assert!(error.message.contains("not initialized"));
            },
            _ => panic!("Expected error for uninitialized server"),
        }
    }

    #[tokio::test]
    async fn test_tool_listing() {
        let tool1 = MockTool::new();
        let tool2 = MockTool::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("tool1", tool1)
            .tool("tool2", tool2)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List tools
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 2);

                let tool_names: Vec<&str> =
                    tools_result.tools.iter().map(|t| t.name.as_str()).collect();
                assert!(tool_names.contains(&"tool1"));
                assert!(tool_names.contains(&"tool2"));
            },
            _ => panic!("Expected successful tools list"),
        }
    }

    #[tokio::test]
    async fn test_tool_schema_in_list() {
        use crate::server::simple_tool::SyncTool;

        let tool_with_schema = SyncTool::new("math_tool", |args| {
            let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!({ "result": a + b }))
        })
        .with_description("Adds two numbers")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "First number" },
                "b": { "type": "number", "description": "Second number" }
            },
            "required": ["a", "b"]
        }));

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("math_tool", tool_with_schema)
            .tool("plain_tool", MockTool::new())
            .build()
            .unwrap();

        // Initialize
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List tools
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 2);

                // Check tool with schema
                let math_tool = tools_result
                    .tools
                    .iter()
                    .find(|t| t.name == "math_tool")
                    .expect("math_tool not found");

                assert_eq!(math_tool.description.as_deref(), Some("Adds two numbers"));
                assert_eq!(math_tool.input_schema["type"], "object");
                assert_eq!(math_tool.input_schema["required"], json!(["a", "b"]));
                assert_eq!(math_tool.input_schema["properties"]["a"]["type"], "number");

                // Check plain tool has default empty schema
                let plain_tool = tools_result
                    .tools
                    .iter()
                    .find(|t| t.name == "plain_tool")
                    .expect("plain_tool not found");

                assert_eq!(plain_tool.description, None);
                assert_eq!(plain_tool.input_schema, json!({}));
            },
            _ => panic!("Expected successful tools list"),
        }
    }

    #[tokio::test]
    async fn test_tool_invocation() {
        let tool = Arc::new(MockTool::with_return(json!({
            "result": "computed",
            "value": 42
        })));
        let tool_clone = tool.clone();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool_arc("calculator", tool_clone)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "calculator".to_string(),
            arguments: json!({
                "operation": "add",
                "a": 5,
                "b": 3
            }),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(!call_result.is_error);
                assert_eq!(call_result.content.len(), 1);
            },
            _ => panic!("Expected successful tool call"),
        }

        // Verify tool was invoked
        assert_eq!(tool.invocation_count(), 1);
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let server = create_test_server();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call non-existent tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "nonexistent".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            _ => panic!("Expected error for non-existent tool"),
        }
    }

    #[tokio::test]
    async fn test_tool_error_handling() {
        let tool = MockTool::failing();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("failing_tool", tool)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call the failing tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "failing_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("Mock tool error"));
            },
            _ => panic!("Expected error from failing tool"),
        }
    }

    #[tokio::test]
    async fn test_prompt_handling() {
        let prompt = MockPromptHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .prompt("test_prompt", prompt)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List prompts
        let list_request =
            Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
                cursor: None,
            })));

        let list_response = server
            .handle_request(RequestId::from(2i64), list_request, None)
            .await;

        match list_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let prompts_result: ListPromptsResult = serde_json::from_value(result).unwrap();
                assert_eq!(prompts_result.prompts.len(), 1);
                assert_eq!(prompts_result.prompts[0].name, "test_prompt");
            },
            _ => panic!("Expected successful prompts list"),
        }

        // Get prompt
        let get_request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name: "test_prompt".to_string(),
            arguments: HashMap::from([("key".to_string(), "value".to_string())]),
            _meta: None,
        })));

        let get_response = server
            .handle_request(RequestId::from(3i64), get_request, None)
            .await;

        match get_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let prompt_result: GetPromptResult = serde_json::from_value(result).unwrap();
                assert_eq!(prompt_result.description, Some("Test prompt".to_string()));
                assert_eq!(prompt_result.messages.len(), 1);
            },
            _ => panic!("Expected successful prompt get"),
        }
    }

    #[tokio::test]
    async fn test_resource_handling() {
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .resources(resources)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List resources
        let list_request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor: None },
        )));

        let list_response = server
            .handle_request(RequestId::from(2i64), list_request, None)
            .await;

        match list_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let resources_result: ListResourcesResult = serde_json::from_value(result).unwrap();
                assert_eq!(resources_result.resources.len(), 1);
                assert_eq!(resources_result.resources[0].uri, "test://resource1");
            },
            _ => panic!("Expected successful resources list"),
        }

        // Read resource
        let read_request =
            Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
                uri: "test://resource1".to_string(),
                _meta: None,
            })));

        let read_response = server
            .handle_request(RequestId::from(3i64), read_request, None)
            .await;

        match read_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let read_result: ReadResourceResult = serde_json::from_value(result).unwrap();
                assert_eq!(read_result.contents.len(), 1);
                // Check that we got content back
                assert_eq!(read_result.contents.len(), 1);
            },
            _ => panic!("Expected successful resource read"),
        }
    }

    #[tokio::test]
    async fn test_resource_not_found() {
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .resources(resources)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Read non-existent resource
        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "test://nonexistent".to_string(),
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("Resource not found"));
            },
            _ => panic!("Expected error for non-existent resource"),
        }
    }

    #[tokio::test]
    async fn test_capabilities_reporting() {
        let tool = MockTool::new();
        let prompt = MockPromptHandler::new();
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("tool1", tool)
            .prompt("prompt1", prompt)
            .resources(resources)
            .build()
            .unwrap();

        // Check capabilities through ProtocolHandler trait
        let caps = server.capabilities();
        assert!(caps.tools.is_some());
        assert!(caps.prompts.is_some());
        assert!(caps.resources.is_some());

        // Check info through ProtocolHandler trait
        let info = server.info();
        assert_eq!(info.name, "test-server");
        assert_eq!(info.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_notification_handling() {
        let server = create_test_server();

        // Send a notification
        let notification = Notification::Progress(ProgressNotification::new(
            ProgressToken::String("test".to_string()),
            50.0,
            Some("Processing".to_string()),
        ));

        // Should handle without error
        let result = server.handle_notification(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        use futures::future::join_all;

        let tool = Arc::new(MockTool::new());
        let tool_clone = tool.clone();

        let server = Arc::new(
            ServerCoreBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .tool_arc("concurrent_tool", tool_clone)
                .build()
                .unwrap(),
        );

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Create multiple concurrent requests
        let mut futures = Vec::new();
        for i in 1..=10 {
            let server_clone = server.clone();
            let future = async move {
                let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
                    name: "concurrent_tool".to_string(),
                    arguments: json!({ "id": i }),
                    _meta: None,
                    task: None,
                })));
                server_clone
                    .handle_request(RequestId::from(i as i64), request, None)
                    .await
            };
            futures.push(future);
        }

        // Execute all requests concurrently
        let results = join_all(futures).await;

        // All should succeed
        for response in results {
            match response.payload {
                crate::types::jsonrpc::ResponsePayload::Result(_) => {
                    // Success
                },
                _ => panic!("Expected successful concurrent tool calls"),
            }
        }

        // Verify all invocations
        assert_eq!(tool.invocation_count(), 10);
    }

    #[tokio::test]
    async fn test_builder_validation() {
        // Missing name
        let result = ServerCoreBuilder::new().version("1.0.0").build();
        assert!(result.is_err());

        // Missing version
        let result = ServerCoreBuilder::new().name("test").build();
        assert!(result.is_err());

        // Valid configuration
        let result = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_custom_capabilities() {
        let custom_caps = ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            prompts: None,
            resources: None,
            logging: None,
            completions: None,
            sampling: None,
            tasks: None,
            experimental: None,
        };

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .capabilities(custom_caps.clone())
            .build()
            .unwrap();

        assert_eq!(server.capabilities().tools, custom_caps.tools);
        assert_eq!(server.capabilities().prompts, custom_caps.prompts);
    }

    #[tokio::test]
    async fn test_call_tool_with_task_support_returns_create_task_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::tasks::RELATED_TASK_META_KEY;
        use crate::types::{TaskSupport, ToolExecution};

        // Create a TypedTool that returns Task-shaped JSON and declares taskSupport
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-test-123",
                        "status": "working",
                        "ttl": 60000,
                        "createdAt": "2026-03-22T00:00:00Z",
                        "lastUpdatedAt": "2026-03-22T00:00:00Z",
                        "pollInterval": 5000
                    }))
                })
            },
        )
        .with_description("A task-creating tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the task tool WITH task field (client requests task-augmented response)
        let mut call_req = CallToolRequest::new("task_tool", json!({}));
        call_req.task = Some(json!({})); // Client signals task-augmented mode
        let request = Request::Client(Box::new(ClientRequest::CallTool(call_req)));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Verify it's a CreateTaskResult (has "task" key, no "content" key)
                assert!(
                    result.get("task").is_some(),
                    "Response should have 'task' field for CreateTaskResult, got: {}",
                    serde_json::to_string_pretty(&result).unwrap()
                );
                assert!(
                    result.get("content").is_none(),
                    "Response should NOT have 'content' field (that would be CallToolResult)"
                );

                // Verify task fields
                assert_eq!(result["task"]["taskId"], "t-test-123");
                assert_eq!(result["task"]["status"], "working");

                // Verify _meta with related-task reference (D-08, D-09)
                assert!(
                    result.get("_meta").is_some(),
                    "Response should have '_meta' with related-task"
                );
                let related = &result["_meta"][RELATED_TASK_META_KEY];
                assert_eq!(related["taskId"], "t-test-123");
            },
            _ => panic!("Expected successful tool call with CreateTaskResult"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_without_task_support_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;

        // Tool returns Task-shaped JSON but does NOT declare taskSupport
        let normal_tool = MockTool::with_return(json!({
            "taskId": "t-sneaky",
            "status": "working"
        }));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("normal_tool", normal_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "normal_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Should be CallToolResult with content, NOT CreateTaskResult with task
                assert!(
                    result.get("content").is_some(),
                    "Should be CallToolResult with 'content' field"
                );
                assert!(
                    result.get("task").is_none(),
                    "Should NOT be CreateTaskResult -- tool doesn't declare taskSupport"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_task_support_but_no_task_field_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::{TaskSupport, ToolExecution};

        // Tool declares taskSupport=Required but client does NOT send task field
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-test-456",
                        "status": "working"
                    }))
                })
            },
        )
        .with_description("A task tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call WITHOUT task field — client doesn't support task-augmented calls
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "task_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Should be CallToolResult (backward compat for non-task-aware clients)
                assert!(
                    result.get("content").is_some(),
                    "Should be CallToolResult when client doesn't send task field"
                );
                assert!(
                    result.get("task").is_none(),
                    "Should NOT be CreateTaskResult — client didn't request task mode"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_forbidden_task_support_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::{TaskSupport, ToolExecution};

        // Create TypedTool with Forbidden task support
        let forbidden_tool = TypedTool::new_with_schema(
            "forbidden_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-should-not-detect",
                        "status": "working"
                    }))
                })
            },
        )
        .with_description("Forbidden task support")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("forbidden_tool", forbidden_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "forbidden_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                assert!(result.get("content").is_some(), "Should be CallToolResult");
                assert!(
                    result.get("task").is_none(),
                    "Should NOT detect task -- Forbidden"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }
}
