//! MCP protocol-specific types.
//!
//! This module contains the core protocol types including initialization,
//! version negotiation, request routing, and completion types.

pub mod version;

use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};
use serde::{Deserialize, Serialize};

// Re-export version constants and negotiation function.
pub use version::*;

// Re-export domain modules' types for backward compatibility.
// Types that were previously in this file are now in their own modules
// and re-exported via types/mod.rs. These re-exports preserve the
// `crate::types::protocol::X` import paths used throughout the codebase.
pub use super::content::*;
pub use super::notifications::*;
pub use super::prompts::*;
pub use super::resources::*;
pub use super::sampling::*;
pub use super::tasks::*;
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

/// Icon information for entities (MCP 2025-11-25).
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::IconInfo;
///
/// let icon = IconInfo::new("https://example.com/icon.png")
///     .with_mime_type("image/png")
///     .with_sizes(vec!["32x32".to_string()]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct IconInfo {
    /// Icon source URL
    pub src: String,
    /// Icon MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Icon sizes (e.g., `["16x16", "32x32"]`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
    /// Icon theme preference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<IconTheme>,
}

impl IconInfo {
    /// Create an `IconInfo` with just the source URL.
    ///
    /// Optional fields (`mime_type`, sizes, theme) default to `None`.
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            mime_type: None,
            sizes: None,
            theme: None,
        }
    }

    /// Set the MIME type for the icon.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the icon sizes (e.g., \["16x16", "32x32"\]).
    pub fn with_sizes(mut self, sizes: Vec<String>) -> Self {
        self.sizes = Some(sizes);
        self
    }

    /// Set the icon theme preference.
    pub fn with_theme(mut self, theme: IconTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

/// Icon theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconTheme {
    /// Light theme icon
    Light,
    /// Dark theme icon
    Dark,
}

/// MCP-specific JSON-RPC error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorCode {
    /// Invalid request
    InvalidRequest = -32600,
    /// Method not found
    MethodNotFound = -32601,
    /// Invalid parameters
    InvalidParams = -32602,
    /// Internal error
    InternalError = -32603,
}

/// Implementation information.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor and fluent
/// methods to remain forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::Implementation;
///
/// let info = Implementation::new("my-server", "1.0.0")
///     .with_title("My Server")
///     .with_description("A great server");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name (e.g., "mcp-sdk-rust")
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Implementation version
    pub version: String,
    /// Optional website URL (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    /// Optional description (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<IconInfo>>,
}

impl Implementation {
    /// Create an `Implementation` with just name and version.
    ///
    /// The optional 2025-11-25 fields (title, website\_url, description, icons)
    /// default to `None`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            version: version.into(),
            website_url: None,
            description: None,
            icons: None,
        }
    }

    /// Set a human-readable title (MCP 2025-11-25).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the website URL (MCP 2025-11-25).
    pub fn with_website_url(mut self, url: impl Into<String>) -> Self {
        self.website_url = Some(url.into());
        self
    }

    /// Set a human-readable description (MCP 2025-11-25).
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set icons for the implementation (MCP 2025-11-25).
    pub fn with_icons(mut self, icons: Vec<IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }
}

/// Initialize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version the client wants to use
    pub protocol_version: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Client implementation info
    pub client_info: Implementation,
}

impl InitializeRequest {
    /// Create an initialize request with the latest protocol version.
    pub fn new(client_info: Implementation, capabilities: ClientCapabilities) -> Self {
        Self {
            protocol_version: crate::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info,
        }
    }
}

/// Initialize response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::{InitializeResult, Implementation};
/// use pmcp::ServerCapabilities;
///
/// let result = InitializeResult::new(
///     Implementation::new("my-server", "1.0.0"),
///     ServerCapabilities::tools_only(),
/// ).with_instructions("Use this server for ...");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
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

impl InitializeResult {
    /// Create an initialize result with the default protocol version.
    ///
    /// Instructions default to `None`.
    pub fn new(server_info: Implementation, capabilities: ServerCapabilities) -> Self {
        Self {
            protocol_version: ProtocolVersion::default(),
            capabilities,
            server_info,
            instructions: None,
        }
    }

    /// Set optional instructions for the client.
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
}

/// Pagination cursor.
pub type Cursor = Option<String>;

/// Request metadata that can be attached to any request.
///
/// This follows the MCP protocol's `_meta` field specification.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::RequestMeta;
/// use pmcp::types::notifications::ProgressToken;
///
/// let meta = RequestMeta::new()
///     .with_progress_token(ProgressToken::String("tok-1".to_string()))
///     .with_task_id("task-abc");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
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

impl RequestMeta {
    /// Create an empty `RequestMeta`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the progress token.
    pub fn with_progress_token(mut self, token: super::notifications::ProgressToken) -> Self {
        self.progress_token = Some(token);
        self
    }

    /// Set the task ID for workflow continuation (PMCP extension).
    #[allow(clippy::used_underscore_binding)]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self._task_id = Some(task_id.into());
        self
    }
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

/// Completion result wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    /// Completion options
    pub completion: CompletionResult,
}

/// Completion result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
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

impl CompletionResult {
    /// Create a completion result with the given values.
    ///
    /// `has_more` defaults to `false`, `total` defaults to `None`.
    pub fn new(values: Vec<String>) -> Self {
        Self {
            values,
            total: None,
            has_more: false,
        }
    }

    /// Set the total number of completions available.
    pub fn with_total(mut self, total: usize) -> Self {
        self.total = Some(total);
        self
    }

    /// Set whether there are more completions available.
    pub fn with_has_more(mut self, has_more: bool) -> Self {
        self.has_more = has_more;
        self
    }
}

/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum ClientRequest {
    /// Initialize the connection
    #[serde(rename = "initialize")]
    Initialize(InitializeRequest),
    /// List available tools
    #[serde(rename = "tools/list")]
    ListTools(super::tools::ListToolsRequest),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(super::tools::CallToolRequest),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(super::prompts::ListPromptsRequest),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(super::prompts::GetPromptRequest),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(super::resources::ListResourcesRequest),
    /// List resource templates
    #[serde(rename = "resources/templates/list")]
    ListResourceTemplates(super::resources::ListResourceTemplatesRequest),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(super::resources::ReadResourceRequest),
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
    /// Create message (sampling).
    /// Boxed to match `ServerRequest::CreateMessage` and avoid inflating the enum.
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<super::sampling::CreateMessageParams>),
    /// Get task status (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/get")]
    TasksGet(crate::types::tasks::GetTaskRequest),
    /// Get task result (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/result")]
    TasksResult(crate::types::tasks::GetTaskPayloadRequest),
    /// List tasks (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/list")]
    TasksList(crate::types::tasks::ListTasksRequest),
    /// Cancel a task (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/cancel")]
    TasksCancel(crate::types::tasks::CancelTaskRequest),
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
    /// Request to elicit user input (spec method: elicitation/create)
    #[serde(rename = "elicitation/create")]
    ElicitationCreate(Box<crate::types::elicitation::ElicitRequestParams>),
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

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(super::super::tools::ListToolsRequest::default());
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
        let req = ClientRequest::TasksGet(crate::types::tasks::GetTaskRequest {
            task_id: "t-123".to_string(),
        });
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tasks/get");
        assert_eq!(json["params"]["taskId"], "t-123");

        let deserialized: ClientRequest = serde_json::from_value(json).unwrap();
        assert!(matches!(deserialized, ClientRequest::TasksGet(_)));
    }

    #[test]
    fn request_meta_task_id_serializes_as_underscore() {
        let meta = RequestMeta::new().with_task_id("abc");
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["_task_id"], "abc");
        assert!(
            json.get("_taskId").is_none(),
            "_task_id must not be camelCased"
        );
    }

    #[test]
    fn request_meta_task_id_omitted_when_none() {
        let meta = RequestMeta::new();
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
