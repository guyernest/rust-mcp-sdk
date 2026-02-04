// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! MCP Apps Extension types for interactive UI support.
//!
//! This module provides types for building interactive widgets that work across
//! multiple MCP host platforms:
//!
//! - **ChatGPT Apps** (OpenAI Apps SDK) - Uses `text/html+skybridge` and `window.openai`
//! - **MCP Apps (SEP-1865)** - Standard MCP extension using `text/html+mcp`
//! - **MCP-UI** - Community standard supporting HTML, URLs, and Remote DOM
//!
//! # Architecture
//!
//! The types in this module follow a layered architecture:
//!
//! 1. **Core Types** - Host-agnostic abstractions (`UIAction`, `WidgetCSP`, etc.)
//! 2. **Platform-Specific Metadata** - ChatGPT-specific fields (`ChatGptToolMeta`, etc.)
//! 3. **Adapters** - Transform core types for specific hosts (see `server::mcp_apps`)
//!
//! # Example
//!
//! ```rust
//! use pmcp::types::mcp_apps::{WidgetCSP, ChatGptToolMeta, ToolVisibility};
//!
//! // Configure Content Security Policy
//! let csp = WidgetCSP::new()
//!     .connect("https://api.example.com")
//!     .resources("https://cdn.example.com");
//!
//! // Configure ChatGPT-specific tool metadata
//! let meta = ChatGptToolMeta::new()
//!     .output_template("ui://widget/my-widget.html")
//!     .invoking("Loading...")
//!     .invoked("Ready!")
//!     .widget_accessible(true);
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `mcp-apps` feature:
//!
//! ```toml
//! [dependencies]
//! pmcp = { version = "1.9", features = ["mcp-apps"] }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;

// =============================================================================
// Content Security Policy
// =============================================================================

/// Widget Content Security Policy for ChatGPT Apps.
///
/// Defines which domains the widget can interact with. ChatGPT enforces these
/// restrictions in the sandboxed iframe environment.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::WidgetCSP;
///
/// let csp = WidgetCSP::new()
///     .connect("https://api.example.com")      // Allow fetch/XHR
///     .resources("https://cdn.example.com")    // Allow images, scripts
///     .resources("https://*.oaistatic.com")    // OpenAI's CDN
///     .redirect("https://checkout.example.com"); // Allow external redirects
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetCSP {
    /// Domains the widget can fetch from (connect-src).
    ///
    /// Use for API endpoints that the widget calls directly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connect_domains: Vec<String>,

    /// Domains for static assets like images, fonts, and scripts.
    ///
    /// Use for CDN-hosted widget bundles and media.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_domains: Vec<String>,

    /// Domains for external redirects via `openExternal`.
    ///
    /// ChatGPT appends a `redirectUrl` query parameter to help external
    /// flows return to the conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_domains: Option<Vec<String>>,

    /// Domains allowed for iframes within the widget.
    ///
    /// **Warning:** Using `frame_domains` is discouraged and triggers
    /// extra scrutiny during ChatGPT App review. Only use when embedding
    /// iframes is essential to your experience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,
}

impl WidgetCSP {
    /// Create an empty CSP configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connect domain (for fetch/XHR requests).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::mcp_apps::WidgetCSP;
    ///
    /// let csp = WidgetCSP::new()
    ///     .connect("https://api.example.com")
    ///     .connect("https://api2.example.com");
    /// ```
    pub fn connect(mut self, domain: impl Into<String>) -> Self {
        self.connect_domains.push(domain.into());
        self
    }

    /// Add a resource domain (for images, scripts, fonts, etc.).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::mcp_apps::WidgetCSP;
    ///
    /// let csp = WidgetCSP::new()
    ///     .resources("https://cdn.example.com")
    ///     .resources("https://*.cloudfront.net");
    /// ```
    pub fn resources(mut self, domain: impl Into<String>) -> Self {
        self.resource_domains.push(domain.into());
        self
    }

    /// Add a redirect domain (for `openExternal` navigation).
    pub fn redirect(mut self, domain: impl Into<String>) -> Self {
        self.redirect_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Add a frame domain (for iframes - use with caution).
    ///
    /// **Warning:** Widgets with `frame_domains` are subject to higher
    /// scrutiny during review and may be rejected.
    pub fn frame(mut self, domain: impl Into<String>) -> Self {
        self.frame_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Check if the CSP has any domains configured.
    pub fn is_empty(&self) -> bool {
        self.connect_domains.is_empty()
            && self.resource_domains.is_empty()
            && self.redirect_domains.as_ref().is_none_or(Vec::is_empty)
            && self.frame_domains.as_ref().is_none_or(Vec::is_empty)
    }
}

// =============================================================================
// Widget Metadata (Resource-level)
// =============================================================================

/// Widget configuration metadata for ChatGPT Apps.
///
/// These fields are added to the resource's `_meta` field and control
/// how ChatGPT renders and configures the widget.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::{WidgetMeta, WidgetCSP};
///
/// let meta = WidgetMeta::new()
///     .prefers_border(true)
///     .domain("https://chatgpt.com")
///     .description("Interactive chess board")
///     .csp(WidgetCSP::new().connect("https://api.chess.com"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetMeta {
    /// Whether widget prefers a border around it.
    #[serde(
        rename = "openai/widgetPrefersBorder",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefers_border: Option<bool>,

    /// Dedicated origin for the widget.
    ///
    /// When set, ChatGPT renders the widget under
    /// `<domain>.web-sandbox.oaiusercontent.com`, enabling features
    /// like API key allowlists and fullscreen punch-out.
    #[serde(
        rename = "openai/widgetDomain",
        skip_serializing_if = "Option::is_none"
    )]
    pub domain: Option<String>,

    /// Content Security Policy configuration.
    #[serde(rename = "openai/widgetCSP", skip_serializing_if = "Option::is_none")]
    pub csp: Option<WidgetCSP>,

    /// Widget self-description.
    ///
    /// Reduces redundant text beneath the widget by letting it describe itself.
    #[serde(
        rename = "openai/widgetDescription",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
}

impl WidgetMeta {
    /// Create empty widget metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set border preference.
    pub fn prefers_border(mut self, prefers: bool) -> Self {
        self.prefers_border = Some(prefers);
        self
    }

    /// Set widget domain for dedicated origin.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set widget description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set Content Security Policy.
    pub fn csp(mut self, csp: WidgetCSP) -> Self {
        self.csp = Some(csp);
        self
    }

    /// Convert to a serde_json::Map for merging into resource `_meta`.
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.prefers_border.is_none()
            && self.domain.is_none()
            && self.csp.is_none()
            && self.description.is_none()
    }
}

// =============================================================================
// Tool Visibility
// =============================================================================

/// Tool visibility setting for ChatGPT Apps.
///
/// Controls whether a tool is visible to the model or only callable from widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ToolVisibility {
    /// Tool is visible to the model (default).
    ///
    /// The model can decide to call this tool based on user prompts.
    #[default]
    Public,

    /// Tool is hidden from the model.
    ///
    /// Only callable from widgets via `window.openai.callTool()`.
    /// Useful for internal widget operations that shouldn't be
    /// triggered by user prompts.
    Private,
}

// =============================================================================
// Tool Metadata (Tool-level)
// =============================================================================

/// ChatGPT-specific tool metadata.
///
/// These fields are added to the tool's `_meta` field and control
/// how ChatGPT presents and executes the tool.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::{ChatGptToolMeta, ToolVisibility};
///
/// let meta = ChatGptToolMeta::new()
///     .output_template("ui://widget/kanban-board.html")
///     .invoking("Preparing the board...")
///     .invoked("Board ready!")
///     .widget_accessible(true)
///     .visibility(ToolVisibility::Public);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct ChatGptToolMeta {
    /// UI template URI that ChatGPT loads when this tool is called.
    ///
    /// Must point to a resource with `text/html+skybridge` MIME type.
    #[serde(
        rename = "openai/outputTemplate",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_template: Option<String>,

    /// Message shown while the tool is running.
    ///
    /// Example: "Loading data from server..."
    #[serde(
        rename = "openai/toolInvocation/invoking",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoking: Option<String>,

    /// Message shown when the tool completes.
    ///
    /// Example: "Data loaded successfully!"
    #[serde(
        rename = "openai/toolInvocation/invoked",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoked: Option<String>,

    /// Whether the widget can call this tool via `window.openai.callTool()`.
    ///
    /// Set to `true` to enable widget-initiated tool calls (e.g., refresh button).
    #[serde(
        rename = "openai/widgetAccessible",
        skip_serializing_if = "Option::is_none"
    )]
    pub widget_accessible: Option<bool>,

    /// Tool visibility to the model.
    #[serde(rename = "openai/visibility", skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ToolVisibility>,

    /// Names of parameters that accept file uploads.
    ///
    /// Each named parameter must be an object with `download_url` and `file_id` fields.
    #[serde(rename = "openai/fileParams", skip_serializing_if = "Option::is_none")]
    pub file_params: Option<Vec<String>>,
}

impl ChatGptToolMeta {
    /// Create empty tool metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the UI template URI.
    pub fn output_template(mut self, uri: impl Into<String>) -> Self {
        self.output_template = Some(uri.into());
        self
    }

    /// Set the message shown while the tool is running.
    pub fn invoking(mut self, msg: impl Into<String>) -> Self {
        self.invoking = Some(msg.into());
        self
    }

    /// Set the message shown when the tool completes.
    pub fn invoked(mut self, msg: impl Into<String>) -> Self {
        self.invoked = Some(msg.into());
        self
    }

    /// Set whether the widget can call this tool.
    pub fn widget_accessible(mut self, accessible: bool) -> Self {
        self.widget_accessible = Some(accessible);
        self
    }

    /// Set tool visibility.
    pub fn visibility(mut self, visibility: ToolVisibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Set file parameter names.
    pub fn file_params(mut self, params: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.file_params = Some(params.into_iter().map(Into::into).collect());
        self
    }

    /// Convert to a serde_json::Map for merging into tool `_meta`.
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.output_template.is_none()
            && self.invoking.is_none()
            && self.invoked.is_none()
            && self.widget_accessible.is_none()
            && self.visibility.is_none()
            && self.file_params.is_none()
    }
}

// =============================================================================
// Widget Response Metadata (Server-to-Widget Communication)
// =============================================================================

/// Metadata for tool responses in ChatGPT Apps.
///
/// These fields control widget behavior after a tool response is received.
/// They are included in the `_meta` field of `CallToolResult`.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::WidgetResponseMeta;
///
/// // Close the widget after this response
/// let meta = WidgetResponseMeta::new()
///     .close_widget(true)
///     .widget_session_id("session-abc123");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetResponseMeta {
    /// Close the widget when this response arrives.
    ///
    /// When set to `true`, ChatGPT will hide the widget after processing
    /// this tool response. Useful for "done" or "cancel" actions.
    #[serde(rename = "openai/closeWidget", skip_serializing_if = "Option::is_none")]
    pub close_widget: Option<bool>,

    /// Widget session ID for correlating tool calls.
    ///
    /// This ID is unique per widget instance and can be used to correlate
    /// multiple tool calls from the same widget session for logging or analytics.
    #[serde(
        rename = "openai/widgetSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub widget_session_id: Option<String>,
}

impl WidgetResponseMeta {
    /// Create empty response metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to close the widget.
    pub fn close_widget(mut self, close: bool) -> Self {
        self.close_widget = Some(close);
        self
    }

    /// Set the widget session ID.
    pub fn widget_session_id(mut self, id: impl Into<String>) -> Self {
        self.widget_session_id = Some(id.into());
        self
    }

    /// Convert to a serde_json::Map for merging into tool result `_meta`.
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.close_widget.is_none() && self.widget_session_id.is_none()
    }
}

// =============================================================================
// UI Actions (Widget-to-Host Communication)
// =============================================================================

/// Notification severity level for UI notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum NotifyLevel {
    /// Informational message.
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

/// UI actions that widgets can emit to communicate with the host.
///
/// This is a superset of actions supported across all platforms:
/// - **MCP Apps**: `Tool` only
/// - **ChatGPT Apps**: `Tool`, `SetState`, `SendMessage`
/// - **MCP-UI**: `Tool`, `Prompt`, `Intent`, `Notify`, `Link`
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::UIAction;
/// use serde_json::json;
///
/// // Call an MCP tool
/// let action = UIAction::Tool {
///     name: "chess_move".to_string(),
///     arguments: json!({ "from": "e2", "to": "e4" }),
///     message_id: Some("req-123".to_string()),
/// };
///
/// // Update widget state (ChatGPT)
/// let action = UIAction::SetState {
///     state: json!({ "selectedPiece": "e2" }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIAction {
    /// Call an MCP tool.
    ///
    /// Supported by: MCP Apps, ChatGPT Apps, MCP-UI
    #[serde(rename_all = "camelCase")]
    Tool {
        /// Tool name to invoke.
        name: String,
        /// Tool arguments.
        arguments: serde_json::Value,
        /// Optional message ID for async response correlation.
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },

    /// Send a message to the AI (MCP-UI).
    ///
    /// Supported by: MCP-UI, ChatGPT Apps (as `sendFollowUpMessage`)
    Prompt {
        /// Text to send to the AI.
        text: String,
    },

    /// High-level intent action (MCP-UI).
    ///
    /// Supported by: MCP-UI only
    Intent {
        /// Intent action name.
        action: String,
        /// Intent data.
        data: serde_json::Value,
    },

    /// Notification to the user (MCP-UI).
    ///
    /// Supported by: MCP-UI only
    Notify {
        /// Notification severity.
        level: NotifyLevel,
        /// Notification message.
        message: String,
    },

    /// Navigation link (MCP-UI, limited support).
    ///
    /// Supported by: MCP-UI (limited), ChatGPT Apps (via `openExternal`)
    Link {
        /// URL to navigate to.
        url: String,
    },

    /// Update widget state (ChatGPT Apps).
    ///
    /// Supported by: ChatGPT Apps only (via `setWidgetState`)
    SetState {
        /// New state object.
        state: serde_json::Value,
    },

    /// Send follow-up message (ChatGPT Apps).
    ///
    /// Supported by: ChatGPT Apps only
    SendMessage {
        /// Message text.
        message: String,
    },
}

// =============================================================================
// Extended UI MIME Types
// =============================================================================

/// Extended MIME types for MCP Apps.
///
/// Includes support for ChatGPT's Skybridge format and MCP-UI's Remote DOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendedUIMimeType {
    /// Standard MCP Apps HTML (`text/html+mcp`).
    ///
    /// Widgets communicate via postMessage with JSON-RPC protocol.
    HtmlMcp,

    /// ChatGPT Apps Skybridge HTML (`text/html+skybridge`).
    ///
    /// ChatGPT injects `window.openai` API for widget communication.
    HtmlSkybridge,

    /// Plain HTML for MCP-UI hosts (`text/html`).
    HtmlPlain,

    /// URL list for MCP-UI (`text/uri-list`).
    ///
    /// First valid HTTP(S) URL is loaded in an iframe.
    UriList,

    /// Remote DOM for MCP-UI (`application/vnd.mcp-ui.remote-dom+javascript`).
    ///
    /// JavaScript-based UI using Shopify's Remote DOM library.
    RemoteDom,

    /// Remote DOM with React framework.
    RemoteDomReact,
}

impl ExtendedUIMimeType {
    /// Get the MIME type string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HtmlMcp => "text/html+mcp",
            Self::HtmlSkybridge => "text/html+skybridge",
            Self::HtmlPlain => "text/html",
            Self::UriList => "text/uri-list",
            Self::RemoteDom => "application/vnd.mcp-ui.remote-dom+javascript",
            Self::RemoteDomReact => "application/vnd.mcp-ui.remote-dom+javascript; framework=react",
        }
    }

    /// Check if this MIME type is for ChatGPT Apps.
    pub fn is_chatgpt(&self) -> bool {
        matches!(self, Self::HtmlSkybridge)
    }

    /// Check if this MIME type is for standard MCP Apps.
    pub fn is_mcp_apps(&self) -> bool {
        matches!(self, Self::HtmlMcp)
    }

    /// Check if this MIME type is for MCP-UI.
    pub fn is_mcp_ui(&self) -> bool {
        matches!(
            self,
            Self::HtmlPlain | Self::UriList | Self::RemoteDom | Self::RemoteDomReact
        )
    }
}

impl std::fmt::Display for ExtendedUIMimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ExtendedUIMimeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text/html+mcp" => Ok(Self::HtmlMcp),
            "text/html+skybridge" => Ok(Self::HtmlSkybridge),
            "text/html" => Ok(Self::HtmlPlain),
            "text/uri-list" => Ok(Self::UriList),
            "application/vnd.mcp-ui.remote-dom+javascript" => Ok(Self::RemoteDom),
            s if s.starts_with("application/vnd.mcp-ui.remote-dom+javascript") => {
                if s.contains("framework=react") {
                    Ok(Self::RemoteDomReact)
                } else {
                    Ok(Self::RemoteDom)
                }
            },
            _ => Err(format!("Unknown UI MIME type: {}", s)),
        }
    }
}

// =============================================================================
// UI Content Types
// =============================================================================

/// Content types for UI resources.
///
/// Represents the different ways UI content can be delivered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIContent {
    /// Inline HTML content.
    Html {
        /// The HTML content.
        html: String,
    },

    /// External URL to load.
    Url {
        /// The URL to load in an iframe.
        url: String,
    },

    /// Remote DOM JavaScript (MCP-UI).
    #[cfg(feature = "mcp-apps")]
    RemoteDom {
        /// JavaScript code defining the UI.
        script: String,
        /// Framework to use for rendering.
        framework: RemoteDomFramework,
    },
}

/// Framework for Remote DOM rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum RemoteDomFramework {
    /// Web Components (default).
    #[default]
    WebComponents,
    /// React.
    React,
}

// =============================================================================
// Universal UI Resource
// =============================================================================

/// Widget dimensions configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct UIDimensions {
    /// Preferred width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Preferred height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Minimum width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u32>,
    /// Minimum height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<u32>,
    /// Maximum width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    /// Maximum height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
}

/// Universal UI metadata (merged from all platforms).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct UIMetadata {
    /// Widget description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Preferred dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<UIDimensions>,

    /// Initial data to pass to the widget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_data: Option<serde_json::Value>,

    /// Content Security Policy (ChatGPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<WidgetCSP>,

    /// Additional host-specific metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// =============================================================================
// Host Type Detection
// =============================================================================

/// Known MCP host types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostType {
    /// OpenAI ChatGPT.
    ChatGpt,
    /// Anthropic Claude.
    Claude,
    /// Nanobot (MCP-UI).
    Nanobot,
    /// MCPJam (MCP-UI).
    McpJam,
    /// Generic MCP host.
    Generic,
}

impl HostType {
    /// Get the preferred MIME type for this host.
    pub fn preferred_mime_type(&self) -> ExtendedUIMimeType {
        match self {
            Self::ChatGpt => ExtendedUIMimeType::HtmlSkybridge,
            Self::Claude | Self::Generic => ExtendedUIMimeType::HtmlMcp,
            Self::Nanobot | Self::McpJam => ExtendedUIMimeType::HtmlPlain,
        }
    }

    /// Check if this host supports the given MIME type.
    pub fn supports_mime_type(&self, mime_type: ExtendedUIMimeType) -> bool {
        match self {
            Self::ChatGpt => matches!(mime_type, ExtendedUIMimeType::HtmlSkybridge),
            Self::Claude | Self::Generic => matches!(mime_type, ExtendedUIMimeType::HtmlMcp),
            Self::Nanobot | Self::McpJam => mime_type.is_mcp_ui(),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_widget_csp_builder() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .connect("https://api2.example.com")
            .resources("https://cdn.example.com")
            .redirect("https://checkout.example.com")
            .frame("https://embed.example.com");

        assert_eq!(csp.connect_domains.len(), 2);
        assert_eq!(csp.resource_domains.len(), 1);
        assert_eq!(csp.redirect_domains.as_ref().unwrap().len(), 1);
        assert_eq!(csp.frame_domains.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_widget_csp_serialization() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .resources("https://cdn.example.com");

        let json = serde_json::to_value(&csp).unwrap();
        assert_eq!(json["connect_domains"][0], "https://api.example.com");
        assert_eq!(json["resource_domains"][0], "https://cdn.example.com");
        assert!(json.get("redirect_domains").is_none());
        assert!(json.get("frame_domains").is_none());
    }

    #[test]
    fn test_widget_meta_builder() {
        let meta = WidgetMeta::new()
            .prefers_border(true)
            .domain("https://chatgpt.com")
            .description("Test widget")
            .csp(WidgetCSP::new().connect("https://api.example.com"));

        assert_eq!(meta.prefers_border, Some(true));
        assert_eq!(meta.domain, Some("https://chatgpt.com".to_string()));
        assert_eq!(meta.description, Some("Test widget".to_string()));
        assert!(meta.csp.is_some());
    }

    #[test]
    fn test_widget_meta_serialization() {
        let meta = WidgetMeta::new().prefers_border(true).description("Test");

        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["openai/widgetPrefersBorder"], true);
        assert_eq!(json["openai/widgetDescription"], "Test");
    }

    #[test]
    fn test_chatgpt_tool_meta_builder() {
        let meta = ChatGptToolMeta::new()
            .output_template("ui://widget/test.html")
            .invoking("Loading...")
            .invoked("Done!")
            .widget_accessible(true)
            .visibility(ToolVisibility::Public)
            .file_params(vec!["imageFile", "documentFile"]);

        assert_eq!(
            meta.output_template,
            Some("ui://widget/test.html".to_string())
        );
        assert_eq!(meta.invoking, Some("Loading...".to_string()));
        assert_eq!(meta.invoked, Some("Done!".to_string()));
        assert_eq!(meta.widget_accessible, Some(true));
        assert_eq!(meta.visibility, Some(ToolVisibility::Public));
        assert_eq!(meta.file_params.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chatgpt_tool_meta_serialization() {
        let meta = ChatGptToolMeta::new()
            .output_template("ui://widget/test.html")
            .invoking("Loading...")
            .widget_accessible(true)
            .visibility(ToolVisibility::Private);

        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["openai/outputTemplate"], "ui://widget/test.html");
        assert_eq!(json["openai/toolInvocation/invoking"], "Loading...");
        assert_eq!(json["openai/widgetAccessible"], true);
        assert_eq!(json["openai/visibility"], "private");
    }

    #[test]
    fn test_tool_visibility_serialization() {
        assert_eq!(
            serde_json::to_value(ToolVisibility::Public).unwrap(),
            "public"
        );
        assert_eq!(
            serde_json::to_value(ToolVisibility::Private).unwrap(),
            "private"
        );
    }

    #[test]
    fn test_ui_action_tool() {
        let action = UIAction::Tool {
            name: "test_tool".to_string(),
            arguments: json!({ "param": "value" }),
            message_id: Some("123".to_string()),
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "tool");
        assert_eq!(json["name"], "test_tool");
        assert_eq!(json["arguments"]["param"], "value");
        assert_eq!(json["messageId"], "123");
    }

    #[test]
    fn test_ui_action_set_state() {
        let action = UIAction::SetState {
            state: json!({ "selected": "item1" }),
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "setState");
        assert_eq!(json["state"]["selected"], "item1");
    }

    #[test]
    fn test_extended_ui_mime_type() {
        assert_eq!(ExtendedUIMimeType::HtmlMcp.as_str(), "text/html+mcp");
        assert_eq!(
            ExtendedUIMimeType::HtmlSkybridge.as_str(),
            "text/html+skybridge"
        );
        assert_eq!(ExtendedUIMimeType::HtmlPlain.as_str(), "text/html");

        assert!(ExtendedUIMimeType::HtmlSkybridge.is_chatgpt());
        assert!(ExtendedUIMimeType::HtmlMcp.is_mcp_apps());
        assert!(ExtendedUIMimeType::HtmlPlain.is_mcp_ui());
    }

    #[test]
    fn test_extended_ui_mime_type_from_str() {
        assert_eq!(
            "text/html+mcp".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlMcp
        );
        assert_eq!(
            "text/html+skybridge".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlSkybridge
        );
        assert_eq!(
            "text/html".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlPlain
        );
        assert!("invalid".parse::<ExtendedUIMimeType>().is_err());
    }

    #[test]
    fn test_host_type_mime_type() {
        assert_eq!(
            HostType::ChatGpt.preferred_mime_type(),
            ExtendedUIMimeType::HtmlSkybridge
        );
        assert_eq!(
            HostType::Claude.preferred_mime_type(),
            ExtendedUIMimeType::HtmlMcp
        );
        assert_eq!(
            HostType::Nanobot.preferred_mime_type(),
            ExtendedUIMimeType::HtmlPlain
        );
    }

    #[test]
    fn test_to_meta_map() {
        let widget_meta = WidgetMeta::new().prefers_border(true).description("Test");

        let map = widget_meta.to_meta_map();
        assert_eq!(
            map.get("openai/widgetPrefersBorder"),
            Some(&serde_json::Value::Bool(true))
        );

        let tool_meta = ChatGptToolMeta::new()
            .output_template("ui://test")
            .widget_accessible(true);

        let map = tool_meta.to_meta_map();
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&serde_json::Value::String("ui://test".to_string()))
        );
    }
}
