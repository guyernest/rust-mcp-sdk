//! Sampling types for MCP protocol.
//!
//! This module contains sampling-related types including message creation,
//! model preferences, token usage, and tool-use extensions (MCP 2025-11-25).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Role;

/// Model preferences for sampling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreferences {
    /// Hints for model selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    /// Cost priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f64>,
    /// Speed priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f64>,
    /// Intelligence priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f64>,
}

/// Model hint for sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHint {
    /// Model name/identifier hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Tool choice configuration for sampling (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolChoice {
    /// Tool choice mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ToolChoiceMode>,
}

/// Tool choice mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoiceMode {
    /// Model decides whether to use tools
    Auto,
    /// Model must use a tool
    Required,
    /// Model must not use tools
    None,
}

/// Content in a sampling message or sampling result (MCP 2025-11-25).
///
/// Represents the expanded content type that includes standard content
/// plus tool use and tool result content for multi-turn tool interactions.
/// Used in both `SamplingMessage.content` and `CreateMessageResultWithTools.content`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SamplingMessageContent {
    /// Text content
    #[serde(rename = "text", rename_all = "camelCase")]
    Text {
        /// The text content
        text: String,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Image content
    #[serde(rename = "image", rename_all = "camelCase")]
    Image {
        /// Base64-encoded image data
        data: String,
        /// Image MIME type
        mime_type: String,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Audio content
    #[serde(rename = "audio", rename_all = "camelCase")]
    Audio {
        /// Base64-encoded audio data
        data: String,
        /// Audio MIME type
        mime_type: String,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Tool use content -- model wants to call a tool
    #[serde(rename = "tool_use", rename_all = "camelCase")]
    ToolUse {
        /// Tool name
        name: String,
        /// Tool use ID for correlation
        id: String,
        /// Tool input arguments
        input: Value,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Tool result content -- result of a tool call
    #[serde(rename = "tool_result", rename_all = "camelCase")]
    ToolResult {
        /// Correlates with tool_use id
        tool_use_id: String,
        /// Result content items
        content: Vec<super::content::Content>,
        /// Structured result data
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_content: Option<Value>,
        /// Whether the tool call was an error
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
}

/// Create message parameters (for server requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageParams {
    /// Messages to sample from
    pub messages: Vec<SamplingMessage>,
    /// Optional model preferences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Include context from MCP
    #[serde(default)]
    pub include_context: IncludeContext,
    /// Temperature (0-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Additional model-specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Tool definitions available to the model (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<super::tools::ToolInfo>>,
    /// Tool choice configuration (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

/// Create message result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    /// The content generated by the model
    pub content: super::content::Content,
    /// The model used for generation
    pub model: String,
    /// Token usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Sampling result with tool use support (MCP 2025-11-25).
///
/// Extends `CreateMessageResult` with array content that can include
/// tool use and tool result items alongside standard content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResultWithTools {
    /// The model used for sampling
    pub model: String,
    /// Reason the model stopped generating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Role of the generated message
    pub role: Role,
    /// Array of content items (text, image, audio, tool_use, tool_result)
    pub content: Vec<SamplingMessageContent>,
    /// Optional metadata
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Input tokens used
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// Sampling message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingMessage {
    /// Message role
    pub role: Role,
    /// Message content (expanded to support tool use in MCP 2025-11-25)
    pub content: SamplingMessageContent,
}

/// Context to include in sampling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IncludeContext {
    /// Include context from all connected servers
    AllServers,
    /// Include no additional context
    #[default]
    None,
    /// Include context from this server only
    ThisServer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn include_context_serializes_correctly() {
        assert_eq!(
            serde_json::to_value(IncludeContext::AllServers).unwrap(),
            "allServers"
        );
        assert_eq!(serde_json::to_value(IncludeContext::None).unwrap(), "none");
        assert_eq!(
            serde_json::to_value(IncludeContext::ThisServer).unwrap(),
            "thisServer"
        );
    }

    #[test]
    fn include_context_deserializes_correctly() {
        let all: IncludeContext = serde_json::from_value(json!("allServers")).unwrap();
        assert!(matches!(all, IncludeContext::AllServers));

        let none: IncludeContext = serde_json::from_value(json!("none")).unwrap();
        assert!(matches!(none, IncludeContext::None));

        let this: IncludeContext = serde_json::from_value(json!("thisServer")).unwrap();
        assert!(matches!(this, IncludeContext::ThisServer));
    }

    #[test]
    fn tool_choice_serialization() {
        let choice = ToolChoice {
            mode: Some(ToolChoiceMode::Auto),
        };
        let json = serde_json::to_value(&choice).unwrap();
        assert_eq!(json["mode"], "auto");

        let choice2 = ToolChoice {
            mode: Some(ToolChoiceMode::Required),
        };
        let json2 = serde_json::to_value(&choice2).unwrap();
        assert_eq!(json2["mode"], "required");
    }

    #[test]
    fn create_message_result_with_tools_roundtrip() {
        let result = CreateMessageResultWithTools {
            model: "claude-3".to_string(),
            stop_reason: Some("end_turn".to_string()),
            role: Role::Assistant,
            content: vec![
                SamplingMessageContent::Text {
                    text: "I'll call the tool.".to_string(),
                    meta: None,
                },
                SamplingMessageContent::ToolUse {
                    name: "search".to_string(),
                    id: "tu-1".to_string(),
                    input: json!({"query": "rust"}),
                    meta: None,
                },
            ],
            meta: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["model"], "claude-3");
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"].as_array().unwrap().len(), 2);
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][1]["type"], "tool_use");
        assert_eq!(json["content"][1]["name"], "search");

        let roundtrip: CreateMessageResultWithTools = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.model, "claude-3");
        assert_eq!(roundtrip.content.len(), 2);
    }

    #[test]
    fn sampling_message_with_tool_use_content() {
        let msg = SamplingMessage {
            role: Role::Assistant,
            content: SamplingMessageContent::ToolUse {
                name: "calculate".to_string(),
                id: "tu-2".to_string(),
                input: json!({"expression": "2+2"}),
                meta: None,
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"]["type"], "tool_use");
        assert_eq!(json["content"]["name"], "calculate");

        let roundtrip: SamplingMessage = serde_json::from_value(json).unwrap();
        match roundtrip.content {
            SamplingMessageContent::ToolUse { name, id, .. } => {
                assert_eq!(name, "calculate");
                assert_eq!(id, "tu-2");
            },
            _ => panic!("Expected ToolUse content"),
        }
    }

    #[test]
    fn sampling_message_content_text_roundtrip() {
        let content = SamplingMessageContent::Text {
            text: "hello".to_string(),
            meta: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hello");

        let roundtrip: SamplingMessageContent = serde_json::from_value(json).unwrap();
        match roundtrip {
            SamplingMessageContent::Text { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("Expected Text"),
        }
    }
}
