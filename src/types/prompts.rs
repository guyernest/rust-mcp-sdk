//! Prompt types for MCP protocol.
//!
//! This module contains prompt-related types including prompt information,
//! arguments, requests, and results.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::content::{MessageContent, Role};
use super::protocol::Cursor;
use super::protocol::RequestMeta;

/// List prompts request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List prompts params (legacy name).
pub type ListPromptsParams = ListPromptsRequest;

/// Prompt information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    /// Prompt name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt arguments schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Type hint for prompt arguments.
///
/// This is a PMCP extension to the MCP protocol that helps:
/// - MCP clients display appropriate input widgets (number spinner vs text field)
/// - Validate user input before sending to the server
/// - Enable workflow tool chaining with properly typed parameters
/// - Future-proof for when the MCP protocol adds native type support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptArgumentType {
    /// String value (default)
    #[default]
    String,
    /// Floating-point number
    Number,
    /// Integer number
    Integer,
    /// Boolean true/false
    Boolean,
}

impl PromptArgumentType {
    /// Parse a string value according to this type hint.
    /// Returns a properly typed `serde_json::Value`.
    pub fn parse_value(&self, s: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::String => Ok(serde_json::Value::String(s.to_string())),
            Self::Number => s
                .parse::<f64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid number", s)),
            Self::Integer => s
                .parse::<i64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid integer", s)),
            Self::Boolean => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(serde_json::json!(true)),
                "false" | "0" | "no" => Ok(serde_json::json!(false)),
                _ => Err(format!("'{}' is not a valid boolean (use true/false)", s)),
            },
        }
    }
}

/// Prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
    /// Completion configuration for this argument
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<crate::types::completable::CompletionConfig>,
    /// Type hint for the argument value (PMCP extension).
    ///
    /// When set, the SDK will:
    /// - Validate that string arguments can be parsed to this type
    /// - Convert string arguments to the appropriate JSON type for tool calls
    ///
    /// This field is optional and defaults to "string" behavior if not specified.
    /// MCP clients that don't understand this field will safely ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg_type: Option<PromptArgumentType>,
}

/// List prompts response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListPromptsResult;
///
/// let result = ListPromptsResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts
    pub prompts: Vec<PromptInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListPromptsResult {
    /// Create a new list prompts result.
    pub fn new(prompts: Vec<PromptInfo>) -> Self {
        Self {
            prompts,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Get prompt request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptRequest {
    /// Prompt name
    pub name: String,
    /// Prompt arguments
    #[serde(default)]
    pub arguments: HashMap<String, String>,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// Get prompt params (legacy name).
pub type GetPromptParams = GetPromptRequest;

/// Get prompt result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::GetPromptResult;
///
/// let result = GetPromptResult::new(vec![], Some("A prompt".to_string()));
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
    /// Optional metadata for task-aware workflows (PMCP extension).
    ///
    /// When a workflow prompt is backed by a task, this field contains
    /// task state information (`task_id`, status, step plan) that
    /// task-aware MCP clients can use for structured continuation.
    /// Omitted from serialized JSON when `None`.
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl GetPromptResult {
    /// Create a new get prompt result.
    pub fn new(messages: Vec<PromptMessage>, description: Option<String>) -> Self {
        Self {
            description,
            messages,
            _meta: None,
        }
    }

    /// Add metadata to the prompt result.
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_meta(mut self, meta: serde_json::Map<String, serde_json::Value>) -> Self {
        self._meta = Some(meta);
        self
    }
}

/// Message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: MessageContent,
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;
    use crate::types::content::Content;
    use serde_json::json;

    #[test]
    fn test_prompt_types() {
        let prompt = PromptInfo {
            name: "test_prompt".to_string(),
            description: Some("A test prompt".to_string()),
            arguments: Some(vec![PromptArgument {
                name: "arg1".to_string(),
                description: Some("First argument".to_string()),
                required: true,
                completion: None,
                arg_type: None,
            }]),
        };

        let json = serde_json::to_value(&prompt).unwrap();
        assert_eq!(json["name"], "test_prompt");
        assert_eq!(json["arguments"][0]["name"], "arg1");
        assert_eq!(json["arguments"][0]["required"], true);
    }

    #[test]
    fn get_prompt_result_without_meta_omits_field() {
        let result = GetPromptResult {
            description: Some("Test".to_string()),
            messages: vec![],
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("_meta").is_none(),
            "_meta should be omitted when None"
        );
        assert_eq!(json["description"], "Test");
    }

    #[test]
    fn get_prompt_result_with_meta_includes_field() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-123".to_string()),
        );

        let result = GetPromptResult {
            description: None,
            messages: vec![],
            _meta: Some(meta),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("_meta").is_some(), "_meta should be present");
        assert_eq!(json["_meta"]["taskId"], "task-123");
    }

    #[test]
    fn get_prompt_result_deserialize_without_meta_backward_compat() {
        let json_str = r#"{"messages": [], "description": "Test"}"#;
        let result: GetPromptResult = serde_json::from_str(json_str).unwrap();
        assert!(
            result._meta.is_none(),
            "Missing _meta should deserialize as None"
        );
        assert_eq!(result.description.as_deref(), Some("Test"));
    }

    #[test]
    fn get_prompt_result_serde_round_trip_with_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-456".to_string()),
        );
        meta.insert(
            "status".to_string(),
            serde_json::Value::String("working".to_string()),
        );

        let result = GetPromptResult {
            description: Some("Workflow result".to_string()),
            messages: vec![PromptMessage {
                role: Role::User,
                content: Content::Text {
                    text: "Hello".to_string(),
                },
            }],
            _meta: Some(meta),
        };

        let json = serde_json::to_value(&result).unwrap();
        let round_trip: GetPromptResult = serde_json::from_value(json).unwrap();

        assert_eq!(round_trip.description.as_deref(), Some("Workflow result"));
        assert_eq!(round_trip.messages.len(), 1);
        assert!(round_trip._meta.is_some());
        let rt_meta = round_trip._meta.unwrap();
        assert_eq!(
            rt_meta.get("taskId").unwrap(),
            &serde_json::Value::String("task-456".to_string())
        );
    }
}
