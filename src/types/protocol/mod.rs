//! MCP protocol-specific types.
//!
//! This module contains the core protocol types including initialization,
//! version negotiation, request routing, and completion types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};

// Re-export domain modules' types for backward compatibility.
// Types that were previously in this file are now in their own modules
// and re-exported via types/mod.rs. These re-exports preserve the
// `crate::types::protocol::X` import paths used throughout the codebase.
pub use super::content::*;
pub use super::notifications::*;
pub use super::prompts::*;
pub use super::resources::*;
pub use super::sampling::*;
pub use super::tools::*;

/// Protocol version identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub String);

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self(crate::DEFAULT_PROTOCOL_VERSION.to_string())
    }
}

impl ProtocolVersion {
    /// Get the version as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implementation information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name (e.g., "mcp-sdk-rust")
    pub name: String,
    /// Implementation version
    pub version: String,
}

/// Initialize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version the client wants to use
    pub protocol_version: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Client implementation info
    pub client_info: Implementation,
}

/// Initialize request parameters (legacy name).
pub type InitializeParams = InitializeRequest;

/// Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Negotiated protocol version
    pub protocol_version: ProtocolVersion,
    /// Server capabilities
    pub capabilities: ServerCapabilities,
    /// Server implementation info
    pub server_info: Implementation,
    /// Optional instructions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Pagination cursor.
pub type Cursor = Option<String>;

/// Request metadata that can be attached to any request.
///
/// This follows the MCP protocol's `_meta` field specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMeta {
    /// Progress token for out-of-band progress notifications.
    ///
    /// If specified, the caller is requesting progress notifications for this request.
    /// The value is an opaque token that will be attached to subsequent progress notifications.
    /// The receiver is not obligated to provide these notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_token: Option<super::notifications::ProgressToken>,

    /// Task ID for workflow continuation (PMCP extension).
    ///
    /// When present on a `tools/call` request, the server records the tool
    /// result against the referenced workflow task after normal execution.
    /// The tool call itself proceeds as normal; the recording is best-effort.
    #[serde(skip_serializing_if = "Option::is_none", rename = "_task_id")]
    #[allow(clippy::pub_underscore_fields)]
    pub _task_id: Option<String>,
}

/// Completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteRequest {
    /// The reference to complete from
    pub r#ref: CompletionReference,
    /// The argument to complete
    pub argument: CompletionArgument,
}

/// Completion reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CompletionReference {
    /// Complete from a resource
    #[serde(rename = "ref/resource")]
    Resource {
        /// Resource URI
        uri: String,
    },
    /// Complete from a prompt
    #[serde(rename = "ref/prompt")]
    Prompt {
        /// Prompt name
        name: String,
    },
}

/// Completion argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionArgument {
    /// Argument name
    pub name: String,
    /// Argument value
    pub value: String,
}

/// Completion result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    /// Completion options
    pub completion: CompletionResult,
}

/// Completion result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionResult {
    /// Suggested values
    pub values: Vec<String>,
    /// Total number of completions available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    /// Whether there are more completions available
    #[serde(default)]
    pub has_more: bool,
}

/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientRequest {
    /// Initialize the connection
    #[serde(rename = "initialize")]
    Initialize(InitializeParams),
    /// List available tools
    #[serde(rename = "tools/list")]
    ListTools(super::tools::ListToolsParams),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(super::tools::CallToolParams),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(super::prompts::ListPromptsParams),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(super::prompts::GetPromptParams),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(super::resources::ListResourcesParams),
    /// List resource templates
    #[serde(rename = "resources/templates/list")]
    ListResourceTemplates(super::resources::ListResourceTemplatesRequest),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(super::resources::ReadResourceParams),
    /// Subscribe to resource updates
    #[serde(rename = "resources/subscribe")]
    Subscribe(super::resources::SubscribeRequest),
    /// Unsubscribe from resource updates
    #[serde(rename = "resources/unsubscribe")]
    Unsubscribe(super::resources::UnsubscribeRequest),
    /// Request completion
    #[serde(rename = "completion/complete")]
    Complete(CompleteRequest),
    /// Set logging level
    #[serde(rename = "logging/setLevel")]
    SetLoggingLevel {
        /// Logging level to set
        level: super::notifications::LoggingLevel,
    },
    /// Ping request
    #[serde(rename = "ping")]
    Ping,
    /// Create message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(super::sampling::CreateMessageRequest),
    /// Response to elicitation request
    #[serde(rename = "elicitation/response")]
    ElicitInputResponse(crate::types::elicitation::ElicitInputResponse),
    /// Get task status (experimental MCP Tasks).
    #[serde(rename = "tasks/get")]
    TasksGet(Value),
    /// Get task result (experimental MCP Tasks).
    #[serde(rename = "tasks/result")]
    TasksResult(Value),
    /// List tasks (experimental MCP Tasks).
    #[serde(rename = "tasks/list")]
    TasksList(Value),
    /// Cancel a task (experimental MCP Tasks).
    #[serde(rename = "tasks/cancel")]
    TasksCancel(Value),
}

/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to create a message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<super::sampling::CreateMessageParams>),
    /// List roots request
    #[serde(rename = "roots/list")]
    ListRoots,
    /// Elicit input from user
    #[serde(rename = "elicitation/elicitInput")]
    ElicitInput(Box<crate::types::elicitation::ElicitInputRequest>),
}

/// Combined request types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Request {
    /// Client request
    Client(Box<ClientRequest>),
    /// Server request
    Server(Box<ServerRequest>),
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(super::super::tools::ListToolsParams::default());
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/list");
    }

    #[test]
    fn test_task_client_request_variants() {
        let json_str = r#"{"method": "tasks/get", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksGet(_)));

        let json_str = r#"{"method": "tasks/result", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksResult(_)));

        let json_str = r#"{"method": "tasks/list", "params": {}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksList(_)));

        let json_str = r#"{"method": "tasks/cancel", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksCancel(_)));
    }

    #[test]
    fn test_task_client_request_roundtrip() {
        let req = ClientRequest::TasksGet(json!({"taskId": "t-123"}));
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tasks/get");
        assert_eq!(json["params"]["taskId"], "t-123");

        let deserialized: ClientRequest = serde_json::from_value(json).unwrap();
        assert!(matches!(deserialized, ClientRequest::TasksGet(_)));
    }

    #[test]
    fn request_meta_task_id_serializes_as_underscore() {
        let meta = RequestMeta {
            progress_token: None,
            _task_id: Some("abc".to_string()),
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["_task_id"], "abc");
        assert!(
            json.get("_taskId").is_none(),
            "_task_id must not be camelCased"
        );
    }

    #[test]
    fn request_meta_task_id_omitted_when_none() {
        let meta = RequestMeta {
            progress_token: None,
            _task_id: None,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert!(
            json.get("_task_id").is_none(),
            "_task_id should be omitted when None"
        );
    }

    #[test]
    fn request_meta_task_id_deserialization() {
        let json_str = r#"{"_task_id": "task-xyz"}"#;
        let meta: RequestMeta = serde_json::from_str(json_str).unwrap();
        assert_eq!(meta._task_id.as_deref(), Some("task-xyz"));
        assert!(meta.progress_token.is_none());
    }
}
