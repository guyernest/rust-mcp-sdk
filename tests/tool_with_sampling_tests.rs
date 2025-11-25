//! Tests for tool with sampling functionality
//! Following TDD approach - tests written first to define expected behavior

use async_trait::async_trait;
use pmcp::{RequestHandlerExtra, Result, Server, ServerCapabilities, ToolHandler, ToolInfo};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use tokio_util::sync::CancellationToken;

/// Mock sampling handler for testing
struct MockSamplingHandler {
    responses: Arc<Mutex<Vec<String>>>,
}

impl MockSamplingHandler {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

#[async_trait]
impl ToolHandler for MockSamplingHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");

        // Mock the sampling behavior - return a summary
        let mut responses = self.responses.lock().unwrap();
        let summary = if responses.is_empty() {
            if text.is_empty() {
                "Summary of empty text".to_string()
            } else {
                format!("Summary of: {}", text.chars().take(50).collect::<String>())
            }
        } else {
            responses.remove(0)
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": summary
            }],
            "isError": false
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_sampling_handler_creation() {
        let responses = vec!["Mock summary".to_string()];
        let handler = MockSamplingHandler::new(responses);
        assert!(!handler.responses.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sampling_tool_basic_functionality() {
        let responses = vec!["This is a test summary".to_string()];
        let handler = MockSamplingHandler::new(responses);

        let args = json!({
            "text": "This is a long piece of text that should be summarized by the LLM"
        });

        let extra = RequestHandlerExtra::new("test-1".to_string(), CancellationToken::new());
        let result = handler.handle(args, extra).await;
        assert!(result.is_ok());

        let result_value = result.unwrap();
        assert_eq!(result_value["isError"], false);
        assert!(result_value["content"].is_array());

        let content = &result_value["content"][0];
        assert_eq!(content["type"], "text");
        assert_eq!(content["text"], "This is a test summary");
    }

    #[tokio::test]
    async fn test_sampling_tool_empty_text() {
        let responses = vec!["Empty text summary".to_string()];
        let handler = MockSamplingHandler::new(responses);

        let args = json!({"text": ""});
        let extra = RequestHandlerExtra::new("test-1".to_string(), CancellationToken::new());
        let result = handler.handle(args, extra).await;

        assert!(result.is_ok());
        let result_value = result.unwrap();
        assert_eq!(result_value["isError"], false);
        assert_eq!(result_value["content"][0]["text"], "Empty text summary");
    }

    #[tokio::test]
    async fn test_sampling_tool_missing_text_param() {
        let responses = vec![]; // Empty responses to trigger default behavior
        let handler = MockSamplingHandler::new(responses);

        let args = json!({});
        let extra = RequestHandlerExtra::new("test-1".to_string(), CancellationToken::new());
        let result = handler.handle(args, extra).await;

        assert!(result.is_ok());
        let result_value = result.unwrap();
        assert_eq!(result_value["isError"], false);
        // Should handle missing text parameter gracefully
        assert!(result_value["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Summary"));
    }

    #[tokio::test]
    async fn test_sampling_tool_long_text() {
        let long_text = "Lorem ipsum ".repeat(1000);
        let responses = vec!["Summarized long text".to_string()];
        let handler = MockSamplingHandler::new(responses);

        let args = json!({"text": long_text});
        let extra = RequestHandlerExtra::new("test-1".to_string(), CancellationToken::new());
        let result = handler.handle(args, extra).await;

        assert!(result.is_ok());
        let result_value = result.unwrap();
        assert_eq!(result_value["isError"], false);
        assert_eq!(result_value["content"][0]["text"], "Summarized long text");
    }

    #[tokio::test]
    async fn test_tool_info_schema_validation() {
        // Test that the tool schema is properly defined
        let tool_info = ToolInfo::new(
            "summarize",
            Some("Summarize any text using an LLM".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to summarize"
                    }
                },
                "required": ["text"]
            }),
        );

        assert_eq!(tool_info.name, "summarize");
        assert!(tool_info.description.as_ref().unwrap().contains("LLM"));
        assert_eq!(tool_info.input_schema["type"], "object");
        assert!(tool_info.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("text")));
    }

    #[tokio::test]
    async fn test_server_with_sampling_tool() {
        let responses = vec!["Test server summary".to_string()];
        let handler = MockSamplingHandler::new(responses);

        let server = Server::builder()
            .name("test-sampling-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities {
                tools: Some(pmcp::ToolCapabilities {
                    list_changed: Some(true),
                }),
                ..Default::default()
            })
            .tool("summarize", handler);

        // Verify server builds successfully
        assert!(server.build().is_ok());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_sampling_handler_always_returns_valid_response(
            text in any::<String>(),
            summary_response in any::<String>()
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let handler = MockSamplingHandler::new(vec![summary_response.clone()]);
                let args = json!({"text": text});

                let extra = RequestHandlerExtra::new("test-prop".to_string(), CancellationToken::new());
                let result = handler.handle(args, extra).await;

                // Property: Should always return a valid result
                prop_assert!(result.is_ok());

                let result_value = result.unwrap();
                // Property: Should always have correct structure
                prop_assert!(result_value.is_object());
                prop_assert!(result_value["content"].is_array());
                prop_assert_eq!(&result_value["isError"], &false);

                // Property: Content should have text type
                let content = &result_value["content"][0];
                prop_assert_eq!(&content["type"], "text");
                prop_assert!(content["text"].is_string());

                Ok::<(), proptest::test_runner::TestCaseError>(())
            }).unwrap();
        }

        #[test]
        fn test_tool_info_schema_properties(
            name in "[a-zA-Z_][a-zA-Z0-9_]*",
            description in any::<String>()
        ) {
            let tool_info = ToolInfo::new(
                name.clone(),
                Some(description.clone()),
                json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to summarize"
                        }
                    },
                    "required": ["text"]
                }),
            );

            // Property: Tool info should maintain its structure
            prop_assert_eq!(tool_info.name, name);
            prop_assert_eq!(tool_info.description, Some(description));
            prop_assert_eq!(&tool_info.input_schema["type"], "object");
        }

        #[test]
        fn test_sampling_response_serialization_roundtrip(
            text_content in any::<String>()
        ) {
            let response = json!({
                "content": [{
                    "type": "text",
                    "text": text_content
                }],
                "isError": false
            });

            // Property: Serialization should be reversible
            let serialized = serde_json::to_string(&response).unwrap();
            let deserialized: Value = serde_json::from_str(&serialized).unwrap();

            prop_assert_eq!(response, deserialized.clone());
            prop_assert_eq!(&deserialized["content"][0]["text"], &text_content);
        }
    }
}
