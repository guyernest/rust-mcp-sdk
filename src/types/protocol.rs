//! MCP protocol-specific types.
//!
//! This module contains all the protocol-specific request, response, and
//! notification types defined by the MCP specification.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};

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

/// List tools request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List tools params (legacy name).
pub type ListToolsParams = ListToolsRequest;

/// Tool annotations for metadata hints.
///
/// Standard MCP annotations plus PMCP extensions for type-safe composition.
/// Clients SHOULD ignore annotations they don't understand (per MCP spec).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable title for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// If true, the tool does not modify any state (read-only operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    /// If true, the tool may perform destructive operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    /// If true, calling the tool multiple times with same args has same effect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    /// If true, the tool interacts with external systems (network, filesystem, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,

    // =========================================================================
    // PMCP Extensions for Type-Safe Composition
    // =========================================================================
    /// JSON Schema for the tool's output type (PMCP extension).
    ///
    /// When present, code generators can create typed return structs instead of
    /// falling back to `serde_json::Value`. This enables full type safety in
    /// MCP server composition workflows.
    ///
    /// Example:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "columns": { "type": "array", "items": { "type": "string" } },
    ///     "rows": { "type": "array" },
    ///     "row_count": { "type": "integer" }
    ///   },
    ///   "required": ["columns", "rows", "row_count"]
    /// }
    /// ```
    #[serde(rename = "pmcp:outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,

    /// Name of the output type for code generation (PMCP extension).
    ///
    /// Used by code generators to name the generated struct.
    /// Example: `"QueryResult"` generates `pub struct QueryResult { ... }`
    #[serde(
        rename = "pmcp:outputTypeName",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_type_name: Option<String>,
}

impl ToolAnnotations {
    /// Create empty annotations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set human-readable title for the tool.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set read-only hint (tool does not modify any state).
    ///
    /// When `true`, the tool only reads data and never modifies it.
    /// Useful for clients that want to allow read operations without confirmation.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only_hint = Some(read_only);
        self
    }

    /// Set destructive hint (tool may perform destructive operations).
    ///
    /// When `true`, the tool may permanently delete or modify data.
    /// Clients should warn users before executing destructive tools.
    pub fn with_destructive(mut self, destructive: bool) -> Self {
        self.destructive_hint = Some(destructive);
        self
    }

    /// Set idempotent hint (multiple calls with same args have same effect).
    ///
    /// When `true`, calling the tool multiple times with identical arguments
    /// produces the same result as calling it once. Safe to retry on failure.
    pub fn with_idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent_hint = Some(idempotent);
        self
    }

    /// Set open-world hint (tool interacts with external systems).
    ///
    /// When `true`, the tool may make network requests, access filesystem,
    /// or interact with other external services. Results may vary based on
    /// external state.
    pub fn with_open_world(mut self, open_world: bool) -> Self {
        self.open_world_hint = Some(open_world);
        self
    }

    /// Set output schema (PMCP extension for type-safe composition).
    ///
    /// This enables code generators to create typed return structs for
    /// server-to-server composition. The `type_name` is used as the
    /// generated struct name (e.g., `"QueryResult"` becomes `struct QueryResult`).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ToolAnnotations;
    /// use serde_json::json;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_schema(
    ///         json!({
    ///             "type": "object",
    ///             "properties": {
    ///                 "count": { "type": "integer" },
    ///                 "items": { "type": "array" }
    ///             },
    ///             "required": ["count", "items"]
    ///         }),
    ///         "SearchResult"
    ///     );
    /// ```
    pub fn with_output_schema(mut self, schema: Value, type_name: impl Into<String>) -> Self {
        self.output_schema = Some(schema);
        self.output_type_name = Some(type_name.into());
        self
    }
}

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for tool parameters
    pub input_schema: Value,
    /// Tool annotations (hints and PMCP extensions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    /// Optional metadata (e.g., for UI resource association in MCP Apps Extension)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
}

impl ToolInfo {
    /// Create a new `ToolInfo` without metadata or annotations.
    pub fn new(name: impl Into<String>, description: Option<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            description,
            input_schema,
            annotations: None,
            _meta: None,
        }
    }

    /// Create a new `ToolInfo` with annotations (including PMCP output schema).
    ///
    /// Use this constructor when your tool has output type information for
    /// type-safe composition workflows.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::{ToolInfo, ToolAnnotations};
    /// use serde_json::json;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_schema(
    ///         json!({
    ///             "type": "object",
    ///             "properties": {
    ///                 "result": { "type": "string" }
    ///             }
    ///         }),
    ///         "MyResult"
    ///     );
    ///
    /// let tool = ToolInfo::with_annotations(
    ///     "my_tool",
    ///     Some("My tool description".to_string()),
    ///     json!({"type": "object"}),
    ///     annotations,
    /// );
    /// ```
    pub fn with_annotations(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        annotations: ToolAnnotations,
    ) -> Self {
        Self {
            name: name.into(),
            description,
            input_schema,
            annotations: Some(annotations),
            _meta: None,
        }
    }

    /// Create a new `ToolInfo` with UI resource metadata.
    pub fn with_ui(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        ui_resource_uri: impl Into<String>,
    ) -> Self {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui/resourceUri".to_string(),
            Value::String(ui_resource_uri.into()),
        );

        Self {
            name: name.into(),
            description,
            input_schema,
            annotations: None,
            _meta: Some(meta),
        }
    }
}

/// List tools response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Available tools
    pub tools: Vec<ToolInfo>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments (must match input schema)
    #[serde(default)]
    pub arguments: Value,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// Tool call parameters (legacy name).
pub type CallToolParams = CallToolRequest;

/// Tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result
    #[serde(default)]
    pub content: Vec<Content>,
    /// Whether the tool call represents an error
    #[serde(default)]
    pub is_error: bool,
}

/// Message content type alias.
pub type MessageContent = Content;

/// Content item in responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    #[serde(rename_all = "camelCase")]
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },
    /// Resource reference
    #[serde(rename_all = "camelCase")]
    Resource {
        /// Resource URI
        uri: String,
        /// Optional resource content
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// MIME type
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts
    pub prompts: Vec<PromptInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
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

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// System message
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
        }
    }
}

/// List resources request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List resources params (legacy name).
pub type ListResourcesParams = ListResourcesRequest;

/// Resource information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List resources response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources
    pub resources: Vec<ResourceInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Read resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceRequest {
    /// Resource URI
    pub uri: String,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// Read resource params (legacy name).
pub type ReadResourceParams = ReadResourceRequest;

/// List resource templates request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Resource template.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    /// Template URI pattern
    pub uri_template: String,
    /// Template name
    pub name: String,
    /// Template description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type for resources created from this template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List resource templates result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesResult {
    /// Available resource templates
    pub resource_templates: Vec<ResourceTemplate>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Subscribe to resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    /// Resource URI to subscribe to
    pub uri: String,
}

/// Unsubscribe from resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeRequest {
    /// Resource URI to unsubscribe from
    pub uri: String,
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

/// Logging level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    /// Debug messages
    Debug,
    /// Informational messages
    Info,
    /// Warnings
    Warning,
    /// Errors
    Error,
    /// Critical errors
    Critical,
}

/// Read resource result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Resource contents
    pub contents: Vec<Content>,
}

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

/// Progress notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    /// Progress token from the original request
    pub progress_token: ProgressToken,
    /// Current progress value (must increase with each notification)
    ///
    /// This can represent percentage (0-100), count, or any increasing metric.
    pub progress: f64,
    /// Optional total value for the operation
    ///
    /// When combined with `progress`, allows expressing "5 of 10 items processed".
    /// Both `progress` and `total` may be floating point values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Optional human-readable progress message
    ///
    /// Should provide relevant context about the current operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ProgressNotification {
    /// Create a new progress notification with no total value.
    ///
    /// Convenience constructor to reduce boilerplate when the total is unknown.
    pub fn new(progress_token: ProgressToken, progress: f64, message: Option<String>) -> Self {
        Self {
            progress_token,
            progress,
            total: None,
            message,
        }
    }
}

/// Progress (legacy alias).
pub type Progress = ProgressNotification;

/// Progress token type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProgressToken {
    /// String token
    String(String),
    /// Numeric token
    Number(i64),
}

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
    pub progress_token: Option<ProgressToken>,
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
    ListTools(ListToolsParams),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(CallToolParams),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(ListPromptsParams),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(GetPromptParams),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(ListResourcesParams),
    /// List resource templates
    #[serde(rename = "resources/templates/list")]
    ListResourceTemplates(ListResourceTemplatesRequest),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(ReadResourceParams),
    /// Subscribe to resource updates
    #[serde(rename = "resources/subscribe")]
    Subscribe(SubscribeRequest),
    /// Unsubscribe from resource updates
    #[serde(rename = "resources/unsubscribe")]
    Unsubscribe(UnsubscribeRequest),
    /// Request completion
    #[serde(rename = "completion/complete")]
    Complete(CompleteRequest),
    /// Set logging level
    #[serde(rename = "logging/setLevel")]
    SetLoggingLevel {
        /// Logging level to set
        level: LoggingLevel,
    },
    /// Ping request
    #[serde(rename = "ping")]
    Ping,
    /// Create message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(CreateMessageRequest),
    /// Response to elicitation request
    #[serde(rename = "elicitation/response")]
    ElicitInputResponse(crate::types::elicitation::ElicitInputResponse),
}

/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to create a message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<CreateMessageParams>),
    /// List roots request
    #[serde(rename = "roots/list")]
    ListRoots,
    /// Elicit input from user
    #[serde(rename = "elicitation/elicitInput")]
    ElicitInput(Box<crate::types::elicitation::ElicitInputRequest>),
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
}

/// Create message request (for client requests).
pub type CreateMessageRequest = CreateMessageParams;

/// Create message result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    /// The content generated by the model
    pub content: Content,
    /// The model used for generation
    pub model: String,
    /// Token usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
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
    /// Message content
    pub content: Content,
}

/// Context to include in sampling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IncludeContext {
    /// Include all context
    All,
    /// Include no context
    #[default]
    None,
    /// Include specific context types
    ThisServerOnly,
}

/// Client notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientNotification {
    /// Notification that client has been initialized
    #[serde(rename = "notifications/initialized")]
    Initialized,
    /// Notification that roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Notification that a request was cancelled
    #[serde(rename = "notifications/cancelled")]
    Cancelled(CancelledParams),
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
}

/// Cancelled notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotification {
    /// The request ID that was cancelled
    pub request_id: crate::types::RequestId,
    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Cancelled params (legacy alias).
pub type CancelledParams = CancelledNotification;

/// Server notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerNotification {
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
    /// Tools have changed
    #[serde(rename = "notifications/tools/list_changed")]
    ToolsChanged,
    /// Prompts have changed
    #[serde(rename = "notifications/prompts/list_changed")]
    PromptsChanged,
    /// Resources have changed
    #[serde(rename = "notifications/resources/list_changed")]
    ResourcesChanged,
    /// Roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Resource was updated
    #[serde(rename = "notifications/resources/updated")]
    ResourceUpdated(ResourceUpdatedParams),
    /// Log message
    #[serde(rename = "notifications/message")]
    LogMessage(LogMessageParams),
}

/// Resource updated notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdatedParams {
    /// Resource URI that was updated
    pub uri: String,
}

/// Log message notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogMessageParams {
    /// Log level
    pub level: LogLevel,
    /// Logger name/category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// Log message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
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

/// Combined notification types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Notification {
    /// Client notification
    Client(ClientNotification),
    /// Server notification  
    Server(ServerNotification),
    /// Progress notification
    Progress(ProgressNotification),
    /// Cancelled notification
    Cancelled(CancelledNotification),
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warning,
    /// Error level
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(ListToolsParams::default());
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/list");
    }

    #[test]
    fn serialize_content() {
        let content = Content::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn tool_info_serialization() {
        let tool = ToolInfo::new(
            "test-tool",
            Some("A test tool".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
        );

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "test-tool");
        assert_eq!(json["description"], "A test tool");
        assert_eq!(json["inputSchema"]["type"], "object");
    }

    #[test]
    fn test_all_notification_types() {
        let progress = ServerNotification::Progress(ProgressNotification {
            progress_token: ProgressToken::String("token123".to_string()),
            progress: 50.0,
            total: None,
            message: Some("Processing...".to_string()),
        });
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["method"], "notifications/progress");

        let tools_changed = ServerNotification::ToolsChanged;
        let json = serde_json::to_value(&tools_changed).unwrap();
        assert_eq!(json["method"], "notifications/tools/list_changed");

        let prompts_changed = ServerNotification::PromptsChanged;
        let json = serde_json::to_value(&prompts_changed).unwrap();
        assert_eq!(json["method"], "notifications/prompts/list_changed");

        let resources_changed = ServerNotification::ResourcesChanged;
        let json = serde_json::to_value(&resources_changed).unwrap();
        assert_eq!(json["method"], "notifications/resources/list_changed");

        let roots_changed = ServerNotification::RootsListChanged;
        let json = serde_json::to_value(&roots_changed).unwrap();
        assert_eq!(json["method"], "notifications/roots/list_changed");

        let resource_updated = ServerNotification::ResourceUpdated(ResourceUpdatedParams {
            uri: "file://test.txt".to_string(),
        });
        let json = serde_json::to_value(&resource_updated).unwrap();
        assert_eq!(json["method"], "notifications/resources/updated");

        let log_msg = ServerNotification::LogMessage(LogMessageParams {
            level: LogLevel::Info,
            logger: None,
            message: "Test log message".to_string(),
            data: Some(json!({"extra": "data"})),
        });
        let json = serde_json::to_value(&log_msg).unwrap();
        assert_eq!(json["method"], "notifications/message");
    }

    #[test]
    fn test_resource_types() {
        let resource = ResourceInfo {
            uri: "file://test.txt".to_string(),
            name: "test.txt".to_string(),
            description: Some("Test file".to_string()),
            mime_type: Some("text/plain".to_string()),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["uri"], "file://test.txt");
        assert_eq!(json["name"], "test.txt");
        assert_eq!(json["description"], "Test file");
        assert_eq!(json["mimeType"], "text/plain");
    }

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
    fn test_log_levels() {
        assert_eq!(serde_json::to_value(LogLevel::Debug).unwrap(), "debug");
        assert_eq!(serde_json::to_value(LogLevel::Info).unwrap(), "info");
        assert_eq!(serde_json::to_value(LogLevel::Warning).unwrap(), "warning");
        assert_eq!(serde_json::to_value(LogLevel::Error).unwrap(), "error");
    }

    #[test]
    fn test_cancelled_notification() {
        use crate::types::RequestId;

        let cancelled = CancelledNotification {
            request_id: RequestId::Number(123),
            reason: Some("User cancelled".to_string()),
        };

        let json = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(json["requestId"], 123);
        assert_eq!(json["reason"], "User cancelled");
    }
}
