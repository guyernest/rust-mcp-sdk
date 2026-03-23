//! Tool types for MCP protocol.
//!
//! This module contains tool-related types including tool information,
//! annotations, requests, and results.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Content;
use super::protocol::Cursor;
use super::protocol::RequestMeta;

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

    /// Set output type name (PMCP extension for code generation).
    ///
    /// Used by code generators to name the generated struct for the tool's
    /// output type (e.g., `"QueryResult"` becomes `struct QueryResult`).
    ///
    /// The actual output schema is set on [`ToolInfo::with_output_schema`]
    /// as a top-level field (MCP spec 2025-06-18).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ToolAnnotations;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_type_name("SearchResult");
    /// ```
    pub fn with_output_type_name(mut self, name: impl Into<String>) -> Self {
        self.output_type_name = Some(name.into());
        self
    }

    /// Returns `true` if all fields are `None` (no meaningful content).
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.read_only_hint.is_none()
            && self.destructive_hint.is_none()
            && self.idempotent_hint.is_none()
            && self.open_world_hint.is_none()
            && self.output_type_name.is_none()
    }
}

/// Tool execution metadata declaring task support level (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    /// Task support level for this tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_support: Option<TaskSupport>,
}

impl ToolExecution {
    /// Create empty tool execution metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the task support level.
    pub fn with_task_support(mut self, support: TaskSupport) -> Self {
        self.task_support = Some(support);
        self
    }
}

/// Task support level for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSupport {
    /// Task creation is required for this tool
    Required,
    /// Task creation is optional for this tool
    Optional,
    /// Task creation is not supported for this tool
    Forbidden,
}

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name (unique identifier)
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for tool parameters
    pub input_schema: Value,
    /// JSON Schema for the tool's output type (MCP spec 2025-06-18).
    ///
    /// When present, clients can validate and type-check the tool's structured
    /// output. Code generators can create typed return structs instead of
    /// falling back to `serde_json::Value`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Tool annotations (hints and PMCP extensions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional metadata (e.g., for UI resource association in MCP Apps Extension)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
    /// Execution metadata declaring task support level (MCP 2025-11-25).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ToolExecution>,
}

impl ToolInfo {
    /// Create a new `ToolInfo` without metadata or annotations.
    pub fn new(name: impl Into<String>, description: Option<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: None,
            execution: None,
        }
    }

    /// Create a new `ToolInfo` with annotations.
    ///
    /// Use this constructor when your tool has annotation hints. For output
    /// schema, chain [`ToolInfo::with_output_schema`] on the result.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::{ToolInfo, ToolAnnotations};
    /// use serde_json::json;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_type_name("MyResult");
    ///
    /// let tool = ToolInfo::with_annotations(
    ///     "my_tool",
    ///     Some("My tool description".to_string()),
    ///     json!({"type": "object"}),
    ///     annotations,
    /// ).with_output_schema(json!({"type": "object", "properties": {"result": {"type": "string"}}}));
    /// ```
    pub fn with_annotations(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        annotations: ToolAnnotations,
    ) -> Self {
        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: Some(annotations),
            icons: None,
            _meta: None,
            execution: None,
        }
    }

    /// Create a new `ToolInfo` with UI resource metadata.
    ///
    /// Produces nested `_meta` format compatible with both MCP standard and `ChatGPT`:
    /// - `_meta.ui.resourceUri` - MCP standard nested format
    /// - `_meta["openai/outputTemplate"]` - `ChatGPT` alias for the same URI
    pub fn with_ui(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        ui_resource_uri: impl Into<String>,
    ) -> Self {
        let uri: String = ui_resource_uri.into();
        let meta = crate::types::ui::ToolUIMetadata::build_meta_map(&uri);

        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: Some(meta),
            execution: None,
        }
    }

    /// Set the output schema for this tool (MCP spec 2025-06-18).
    ///
    /// The output schema declares the JSON Schema that the tool's structured
    /// output conforms to, enabling clients to validate and type-check results.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::new("my_tool", None, json!({"type": "object"}))
    ///     .with_output_schema(json!({
    ///         "type": "object",
    ///         "properties": { "count": { "type": "integer" } }
    ///     }));
    /// ```
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Add widget metadata, deep-merging into existing `_meta`.
    ///
    /// This merges `WidgetMeta::to_meta_map()` into the tool's `_meta`,
    /// correctly combining nested `ui` objects so that `ui.resourceUri`
    /// and widget fields like `ui.prefersBorder` coexist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use pmcp::types::mcp_apps::WidgetMeta;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/app.html")
    ///     .with_widget_meta(WidgetMeta::new().prefers_border(true));
    /// // _meta.ui = { "resourceUri": "ui://w/app.html", "prefersBorder": true }
    /// ```
    #[cfg(feature = "mcp-apps")]
    #[allow(clippy::used_underscore_binding, clippy::needless_pass_by_value)]
    pub fn with_widget_meta(mut self, widget: crate::types::mcp_apps::WidgetMeta) -> Self {
        let meta = self._meta.get_or_insert_with(serde_json::Map::new);
        let overlay = widget.to_meta_map();
        crate::types::ui::deep_merge(meta, overlay);
        self
    }

    /// Add a single key-value pair to `_meta`, merging with existing entries.
    ///
    /// If the key already exists and both values are objects, they are
    /// deep-merged. Otherwise the new value replaces the old (last-in wins).
    ///
    /// This is the composable counterpart to [`ToolInfo::with_ui`] --
    /// multiple calls can be chained without overwriting each other's keys.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::new("my_tool", None, json!({"type": "object"}))
    ///     .with_meta_entry("ui", json!({"resourceUri": "ui://x"}))
    ///     .with_meta_entry("execution", json!({"mode": "async"}));
    /// ```
    #[allow(clippy::used_underscore_binding)]
    pub fn with_meta_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let meta = self._meta.get_or_insert_with(serde_json::Map::new);
        let mut overlay = serde_json::Map::with_capacity(1);
        overlay.insert(key.into(), value);
        crate::types::ui::deep_merge(meta, overlay);
        self
    }

    /// Return a reference to `_meta` if this tool has widget metadata.
    ///
    /// Single-pass check: returns `Some` only when `_meta` contains a
    /// recognised widget key, `None` otherwise.
    #[allow(clippy::used_underscore_binding)]
    pub fn widget_meta(&self) -> Option<&serde_json::Map<String, Value>> {
        self._meta.as_ref().filter(|meta| {
            meta.contains_key("openai/outputTemplate")
                || meta.contains_key(crate::types::ui::META_KEY_UI_RESOURCE_URI)
                || meta.get("ui").and_then(|v| v.get("resourceUri")).is_some()
        })
    }
}

/// List tools request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List tools response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListToolsResult;
///
/// let result = ListToolsResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Available tools
    pub tools: Vec<ToolInfo>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListToolsResult {
    /// Create a new list tools result.
    pub fn new(tools: Vec<ToolInfo>) -> Self {
        Self {
            tools,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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
    /// Task augmentation parameters (experimental MCP Tasks).
    ///
    /// When present, the server creates a task and returns `CreateTaskResult`
    /// instead of `CallToolResult`. Uses `serde_json::Value` to avoid circular
    /// crate dependency (`pmcp-tasks` depends on `pmcp`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task: Option<Value>,
}

impl CallToolRequest {
    /// Create a tool call request.
    ///
    /// `_meta` and `task` default to `None`.
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
            _meta: None,
            task: None,
        }
    }
}

/// Tool call result.
///
/// Supports three-tier response model for MCP Apps:
/// - `content`: Model-focused narration (goes to model, optionally to widget)
/// - `structured_content`: Structured data for both model and widget
/// - `_meta`: Widget-only metadata (never sent to model)
///
/// # `ChatGPT` Apps Example
///
/// ```rust
/// use pmcp::types::CallToolResult;
/// use serde_json::json;
///
/// let result = CallToolResult::new(vec![])
///     .with_structured_content(json!({
///         "boardState": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR",
///         "lastMove": { "from": "e2", "to": "e4" }
///     }))
///     .with_meta(json!({
///         "widgetState": { "selectedSquare": null }
///     }).as_object().unwrap().clone());
/// ```
///
/// # Backward Compatibility
///
/// Use constructors for clean, future-proof initialization:
///
/// ```rust
/// use pmcp::types::{CallToolResult, Content};
///
/// let result = CallToolResult::new(vec![Content::text("Hello")]);
/// assert!(!result.is_error);
///
/// let error = CallToolResult::error(vec![Content::text("Something went wrong")]);
/// assert!(error.is_error);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result (model-focused narration).
    ///
    /// This content is primarily for the model to understand the result.
    /// In `ChatGPT` Apps, this appears as text below the widget.
    #[serde(default)]
    pub content: Vec<Content>,

    /// Whether the tool call represents an error.
    #[serde(default)]
    pub is_error: bool,

    /// Structured data for both model and widget (`ChatGPT` Apps / MCP Apps Extension).
    ///
    /// Use this for data that should be accessible to both the AI model
    /// (for reasoning) and the widget (for display). Examples:
    /// - Game board state (chess position, game score)
    /// - Query results (database rows, search results)
    /// - Form data (user selections, validated input)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,

    /// Widget-only metadata (`ChatGPT` Apps / MCP Apps Extension).
    ///
    /// Metadata that goes only to the widget, never to the model.
    /// Use for widget display hints, UI state, and internal widget data.
    /// Examples:
    /// - `widgetState`: Persisted widget state (`ChatGPT` manages this)
    /// - Display hints: colors, animations, layout preferences
    /// - Internal IDs that the model doesn't need
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
}

impl CallToolResult {
    /// Create a new tool result with content.
    pub fn new(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: false,
            structured_content: None,
            _meta: None,
        }
    }

    /// Create an error result.
    pub fn error(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: true,
            structured_content: None,
            _meta: None,
        }
    }

    /// Add structured content for both model and widget.
    pub fn with_structured_content(mut self, content: Value) -> Self {
        self.structured_content = Some(content);
        self
    }

    /// Add widget-only metadata.
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self._meta = Some(meta);
        self
    }

    /// Enrich with widget metadata from a [`ToolInfo`] if it has widget meta.
    ///
    /// Sets `structured_content` and `_meta` so widgets can access tool
    /// output data. No-op for non-widget tools. Only clones `_meta` when
    /// the tool actually has widget metadata.
    pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
        if let Some(meta) = info.widget_meta() {
            let enriched = self.with_structured_content(structured_value);
            // Copy all openai/* descriptor keys from the tool's _meta to the
            // CallToolResult._meta so ChatGPT can match the result to its widget.
            let filtered: serde_json::Map<String, Value> = meta
                .iter()
                .filter(|(k, _)| k.starts_with("openai/toolInvocation/"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !filtered.is_empty() {
                enriched.with_meta(filtered)
            } else {
                enriched
            }
        } else {
            self
        }
    }
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)] // MCP protocol fields use underscore prefix (_meta, _task_id)
mod tests {
    use super::*;
    use serde_json::json;

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
    fn test_call_tool_result_basic() {
        let result = CallToolResult::new(vec![Content::Text {
            text: "Move accepted".to_string(),
        }]);

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["content"][0]["text"], "Move accepted");
        assert_eq!(json["isError"], false);
        assert!(json.get("structuredContent").is_none());
        assert!(json.get("_meta").is_none());
    }

    #[test]
    fn test_call_tool_result_with_structured_content() {
        let result = CallToolResult::new(vec![Content::Text {
            text: "Move e2-e4 played".to_string(),
        }])
        .with_structured_content(json!({
            "boardState": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR",
            "lastMove": { "from": "e2", "to": "e4" }
        }));

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json["structuredContent"]["boardState"],
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR"
        );
        assert_eq!(json["structuredContent"]["lastMove"]["from"], "e2");
        assert_eq!(json["structuredContent"]["lastMove"]["to"], "e4");
    }

    #[test]
    fn test_call_tool_result_with_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert("widgetState".to_string(), json!({ "selectedSquare": "e4" }));
        meta.insert("displayHints".to_string(), json!({ "animate": true }));

        let result = CallToolResult::new(vec![]).with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["_meta"]["widgetState"]["selectedSquare"], "e4");
        assert_eq!(json["_meta"]["displayHints"]["animate"], true);
    }

    #[test]
    fn test_call_tool_result_full_three_tier() {
        let mut meta = serde_json::Map::new();
        meta.insert("widgetState".to_string(), json!({ "theme": "dark" }));

        let result = CallToolResult::new(vec![Content::Text {
            text: "Chess game started. White to move.".to_string(),
        }])
        .with_structured_content(json!({
            "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "turn": "white",
            "legalMoves": ["e2e4", "d2d4", "Nf3", "Nc3"]
        }))
        .with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        assert!(json["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Chess game started"));
        assert_eq!(json["structuredContent"]["turn"], "white");
        assert_eq!(json["_meta"]["widgetState"]["theme"], "dark");
    }

    #[test]
    fn test_call_tool_result_error() {
        let result = CallToolResult::error(vec![Content::Text {
            text: "Invalid move: e2-e5 is not legal".to_string(),
        }]);

        assert!(result.is_error);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["isError"], true);
    }

    #[test]
    fn test_call_tool_result_deserialization() {
        let json_str = r#"{
            "content": [{"type": "text", "text": "Move played"}],
            "isError": false,
            "structuredContent": {"position": "e4"},
            "_meta": {"widgetState": {"selected": true}}
        }"#;

        let result: CallToolResult = serde_json::from_str(json_str).unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert!(result.structured_content.is_some());
        assert!(result._meta.is_some());

        let meta_value = result._meta.unwrap();
        assert_eq!(meta_value["widgetState"]["selected"], true);
    }

    #[test]
    fn test_call_tool_request_with_task() {
        let json_str = r#"{"name": "my_tool", "arguments": {}, "task": {"ttl": 60000}}"#;
        let req: CallToolRequest = serde_json::from_str(json_str).unwrap();
        assert!(req.task.is_some());
        assert_eq!(req.task.unwrap()["ttl"], 60000);
    }

    #[test]
    fn test_call_tool_request_without_task_backward_compat() {
        let json_str = r#"{"name": "my_tool", "arguments": {}}"#;
        let req: CallToolRequest = serde_json::from_str(json_str).unwrap();
        assert!(req.task.is_none());
        assert_eq!(req.name, "my_tool");
    }

    #[test]
    fn test_tool_info_with_execution() {
        let mut tool = ToolInfo::new(
            "task-tool",
            Some("A task-enabled tool".to_string()),
            json!({"type": "object"}),
        );
        tool.execution = Some(ToolExecution::new().with_task_support(TaskSupport::Required));

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "task-tool");
        assert_eq!(json["execution"]["taskSupport"], "required");
    }

    #[test]
    fn test_tool_execution_serialization() {
        let exec = ToolExecution::new().with_task_support(TaskSupport::Required);
        let json = serde_json::to_value(&exec).unwrap();
        assert_eq!(json["taskSupport"], "required");

        let exec2 = ToolExecution::new().with_task_support(TaskSupport::Optional);
        let json2 = serde_json::to_value(&exec2).unwrap();
        assert_eq!(json2["taskSupport"], "optional");

        let exec3 = ToolExecution::new().with_task_support(TaskSupport::Forbidden);
        let json3 = serde_json::to_value(&exec3).unwrap();
        assert_eq!(json3["taskSupport"], "forbidden");
    }

    #[test]
    fn test_tool_info_without_execution_omits_field() {
        let tool = ToolInfo::new(
            "normal-tool",
            Some("A normal tool".to_string()),
            json!({"type": "object"}),
        );

        let json = serde_json::to_value(&tool).unwrap();
        assert!(json.get("execution").is_none());
    }

    #[test]
    fn test_tool_info_with_ui_dual_format() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://w/x.html");
        assert_eq!(
            meta.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://w/x.html".to_string())),
            "must have legacy flat ui/resourceUri key for Claude Desktop/ChatGPT"
        );
    }

    #[test]
    fn test_tool_info_with_ui_no_openai_keys() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );
        assert_eq!(
            meta.len(),
            2,
            "_meta should have exactly 2 keys (ui + ui/resourceUri)"
        );
    }

    #[test]
    fn test_with_meta_entry_on_empty_meta() {
        let tool = ToolInfo::new("t", None, json!({"type": "object"}))
            .with_meta_entry("ui", json!({"resourceUri": "ui://x"}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
    }

    #[test]
    fn test_with_meta_entry_merges_with_existing() {
        let mut initial = serde_json::Map::new();
        initial.insert("ui".into(), json!({"resourceUri": "ui://x"}));
        let tool = ToolInfo::new("t", None, json!({"type": "object"}));
        let tool = ToolInfo {
            _meta: Some(initial),
            ..tool
        };
        let tool = tool.with_meta_entry("execution", json!({"mode": "async"}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
        assert_eq!(meta["execution"]["mode"], "async");
    }

    #[test]
    fn test_with_meta_entry_deep_merges_nested() {
        let mut initial = serde_json::Map::new();
        initial.insert("ui".into(), json!({"resourceUri": "ui://x"}));
        let tool = ToolInfo::new("t", None, json!({"type": "object"}));
        let tool = ToolInfo {
            _meta: Some(initial),
            ..tool
        };
        let tool = tool.with_meta_entry("ui", json!({"prefersBorder": true}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
        assert_eq!(meta["ui"]["prefersBorder"], true);
    }

    #[test]
    fn test_with_meta_entry_chained() {
        let tool = ToolInfo::new("t", None, json!({"type": "object"}))
            .with_meta_entry("a", json!(1))
            .with_meta_entry("b", json!(2));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["a"], 1);
        assert_eq!(meta["b"], 2);
    }

    #[test]
    fn test_existing_with_meta_replace_all_unchanged() {
        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://y");
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://y");
        assert!(!meta.contains_key("openai/outputTemplate"));
    }

    #[test]
    #[cfg(feature = "mcp-apps")]
    fn test_with_widget_meta_merges_with_ui() {
        use crate::types::mcp_apps::WidgetMeta;

        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://w/app.html")
            .with_widget_meta(WidgetMeta::new().prefers_border(true).domain("x.com"));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert!(!meta.contains_key("openai/outputTemplate"));
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui"]["domain"], "x.com");
        assert_eq!(meta["openai/widgetPrefersBorder"], true);
        assert_eq!(meta["openai/widgetDomain"], "x.com");
    }

    #[test]
    #[cfg(feature = "mcp-apps")]
    fn test_with_widget_meta_on_empty_meta() {
        use crate::types::mcp_apps::WidgetMeta;

        let tool = ToolInfo::new("t", None, json!({"type": "object"})).with_widget_meta(
            WidgetMeta::new()
                .resource_uri("ui://w/app.html")
                .prefers_border(true),
        );
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert!(!meta.contains_key("openai/outputTemplate"));
        assert_eq!(meta["openai/widgetPrefersBorder"], true);
    }
}
