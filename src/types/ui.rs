// UI resources for MCP Apps Extension (SEP-1865)
//
// This module implements support for interactive user interfaces in MCP.
// UI resources are pre-declared templates that can be associated with tools
// to provide rich, interactive experiences in MCP hosts.

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UI Resource declaration
///
/// UI resources are pre-declared interface templates that can be rendered by MCP hosts.
/// They use the `ui://` URI scheme and are typically associated with tools via metadata.
///
/// # Example
///
/// ```rust
/// use pmcp::types::ui::UIResource;
///
/// let resource = UIResource {
///     uri: "ui://settings/form".to_string(),
///     name: "Settings Form".to_string(),
///     description: Some("Configure application settings".to_string()),
///     mime_type: "text/html+mcp".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIResource {
    /// URI with `ui://` scheme
    ///
    /// Must start with "ui://" followed by a path-like identifier.
    /// Example: <ui://charts/bar-chart>
    pub uri: String,

    /// Human-readable name for the UI resource
    pub name: String,

    /// Optional description of what this UI resource provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// MIME type of the UI resource
    ///
    /// Currently supported: "text/html+mcp"
    /// Future: "application/wasm+mcp", "application/x-remote-dom+mcp"
    pub mime_type: String,
}

impl UIResource {
    /// Create a new UI resource
    pub fn new(uri: impl Into<String>, name: impl Into<String>, mime_type: UIMimeType) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            mime_type: mime_type.as_str().to_string(),
        }
    }

    /// Create a new HTML UI resource with MCP mime type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ui::UIResource;
    ///
    /// let resource = UIResource::html_mcp(
    ///     "ui://settings/form",
    ///     "Settings Form",
    /// );
    /// ```
    pub fn html_mcp(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(uri, name, UIMimeType::HtmlMcp)
    }

    /// Create a new HTML UI resource with `ChatGPT` Skybridge mime type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ui::UIResource;
    ///
    /// let resource = UIResource::html_skybridge(
    ///     "ui://chess/board",
    ///     "Chess Board",
    /// );
    /// ```
    pub fn html_skybridge(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(uri, name, UIMimeType::HtmlSkybridge)
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate that the URI starts with "ui://"
    pub fn validate_uri(&self) -> crate::Result<()> {
        if !self.uri.starts_with("ui://") {
            return Err(crate::Error::validation(format!(
                "UI resource URI must start with 'ui://', got: {}",
                self.uri
            )));
        }
        Ok(())
    }
}

/// Supported MIME types for UI resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIMimeType {
    /// HTML with MCP postMessage support (`text/html+mcp`)
    ///
    /// The UI is rendered in a sandboxed iframe and communicates with the host
    /// via MCP JSON-RPC over postMessage. Used by standard MCP Apps (SEP-1865).
    HtmlMcp,

    /// HTML with `ChatGPT` Skybridge support (`text/html+skybridge`)
    ///
    /// `ChatGPT` injects the `window.openai` API for widget communication.
    /// Used exclusively by `ChatGPT` Apps (`OpenAI` Apps SDK).
    HtmlSkybridge,
    // Future MIME types (commented out for Phase 1):
    // /// WebAssembly with MCP support (`application/wasm+mcp`)
    // WasmMcp,
    //
    // /// Remote DOM with MCP support (`application/x-remote-dom+mcp`)
    // RemoteDomMcp,
}

impl UIMimeType {
    /// Get the MIME type string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HtmlMcp => "text/html+mcp",
            Self::HtmlSkybridge => "text/html+skybridge",
        }
    }

    /// Check if this is a `ChatGPT` Apps MIME type
    pub fn is_chatgpt(&self) -> bool {
        matches!(self, Self::HtmlSkybridge)
    }

    /// Check if this is a standard MCP Apps MIME type
    pub fn is_mcp_apps(&self) -> bool {
        matches!(self, Self::HtmlMcp)
    }
}

impl std::fmt::Display for UIMimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for UIMimeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text/html+mcp" => Ok(Self::HtmlMcp),
            "text/html+skybridge" => Ok(Self::HtmlSkybridge),
            _ => Err(format!("Unknown UI MIME type: {}", s)),
        }
    }
}

/// UI Resource contents for delivery to the host
///
/// This represents the actual content of a UI resource that can be rendered.
/// For HTML resources, the content is provided in the `text` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIResourceContents {
    /// The resource URI
    pub uri: String,

    /// MIME type of the content
    pub mime_type: String,

    /// Text content (for text/html+mcp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Binary content as base64 (for future WASM support)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

impl UIResourceContents {
    /// Create new HTML contents
    pub fn html(uri: impl Into<String>, html: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            mime_type: UIMimeType::HtmlMcp.as_str().to_string(),
            text: Some(html.into()),
            blob: None,
        }
    }
}

/// Tool metadata for UI resource association
///
/// This extends the tool's `_meta` field to reference a UI resource.
/// The metadata is optional and backward compatible.
///
/// # Example
///
/// ```rust
/// use pmcp::types::ui::ToolUIMetadata;
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// let mut meta = HashMap::new();
/// meta.insert("ui/resourceUri".to_string(), Value::String("ui://settings/form".to_string()));
///
/// let ui_meta = ToolUIMetadata::from_metadata(&meta);
/// assert_eq!(ui_meta.ui_resource_uri, Some("ui://settings/form".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolUIMetadata {
    /// UI resource URI (corresponds to `_meta["ui/resourceUri"]`)
    #[serde(rename = "ui/resourceUri", skip_serializing_if = "Option::is_none")]
    pub ui_resource_uri: Option<String>,

    /// Additional metadata fields
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}

impl ToolUIMetadata {
    /// Create new tool UI metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the UI resource URI
    pub fn with_ui_resource(mut self, uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(uri.into());
        self
    }

    /// Extract from a metadata `HashMap`
    pub fn from_metadata(metadata: &HashMap<String, serde_json::Value>) -> Self {
        let ui_resource_uri = metadata
            .get("ui/resourceUri")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut additional = metadata.clone();
        additional.remove("ui/resourceUri");

        Self {
            ui_resource_uri,
            additional,
        }
    }

    /// Convert to metadata `HashMap`
    pub fn to_metadata(&self) -> HashMap<String, serde_json::Value> {
        let mut map = self.additional.clone();
        if let Some(uri) = &self.ui_resource_uri {
            map.insert(
                "ui/resourceUri".to_string(),
                serde_json::Value::String(uri.clone()),
            );
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_resource_creation() {
        let resource = UIResource::new("ui://test/resource", "Test Resource", UIMimeType::HtmlMcp);

        assert_eq!(resource.uri, "ui://test/resource");
        assert_eq!(resource.name, "Test Resource");
        assert_eq!(resource.mime_type, "text/html+mcp");
        assert_eq!(resource.description, None);
    }

    #[test]
    fn test_ui_resource_with_description() {
        let resource = UIResource::new("ui://test/resource", "Test", UIMimeType::HtmlMcp)
            .with_description("A test resource");

        assert_eq!(resource.description, Some("A test resource".to_string()));
    }

    #[test]
    fn test_ui_resource_validation() {
        let valid = UIResource::new("ui://valid", "Valid", UIMimeType::HtmlMcp);
        assert!(valid.validate_uri().is_ok());

        let invalid = UIResource {
            uri: "http://invalid".to_string(),
            name: "Invalid".to_string(),
            description: None,
            mime_type: "text/html+mcp".to_string(),
        };
        assert!(invalid.validate_uri().is_err());
    }

    #[test]
    fn test_mime_type_conversions() {
        use std::str::FromStr;

        assert_eq!(UIMimeType::HtmlMcp.as_str(), "text/html+mcp");
        assert_eq!(UIMimeType::HtmlSkybridge.as_str(), "text/html+skybridge");
        assert_eq!(
            UIMimeType::from_str("text/html+mcp"),
            Ok(UIMimeType::HtmlMcp)
        );
        assert_eq!(
            UIMimeType::from_str("text/html+skybridge"),
            Ok(UIMimeType::HtmlSkybridge)
        );
        assert!(UIMimeType::from_str("invalid").is_err());
    }

    #[test]
    fn test_mime_type_platform_checks() {
        assert!(UIMimeType::HtmlSkybridge.is_chatgpt());
        assert!(!UIMimeType::HtmlSkybridge.is_mcp_apps());
        assert!(UIMimeType::HtmlMcp.is_mcp_apps());
        assert!(!UIMimeType::HtmlMcp.is_chatgpt());
    }

    #[test]
    fn test_ui_resource_contents_html() {
        let contents = UIResourceContents::html("ui://test", "<html>test</html>");

        assert_eq!(contents.uri, "ui://test");
        assert_eq!(contents.mime_type, "text/html+mcp");
        assert_eq!(contents.text, Some("<html>test</html>".to_string()));
        assert_eq!(contents.blob, None);
    }

    #[test]
    fn test_tool_ui_metadata() {
        let meta = ToolUIMetadata::new().with_ui_resource("ui://test");

        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));

        let map = meta.to_metadata();
        assert_eq!(
            map.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://test".to_string()))
        );
    }

    #[test]
    fn test_tool_ui_metadata_from_hashmap() {
        let mut map = HashMap::new();
        map.insert(
            "ui/resourceUri".to_string(),
            serde_json::Value::String("ui://test".to_string()),
        );
        map.insert(
            "custom".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let meta = ToolUIMetadata::from_metadata(&map);

        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));
        assert_eq!(
            meta.additional.get("custom"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }
}
