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

    /// HTML with MCP App profile (`text/html;profile=mcp-app`)
    ///
    /// The profile-based MIME type used by `ChatGPT` for MCP Apps.
    /// This is the format `ChatGPT` advertises in its developer documentation.
    HtmlMcpApp,
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
            Self::HtmlMcpApp => "text/html;profile=mcp-app",
        }
    }

    /// Check if this is a `ChatGPT` Apps MIME type
    pub fn is_chatgpt(&self) -> bool {
        matches!(self, Self::HtmlSkybridge | Self::HtmlMcpApp)
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
            "text/html;profile=mcp-app" => Ok(Self::HtmlMcpApp),
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
/// Uses nested `_meta.ui.resourceUri` format for MCP standard compatibility,
/// plus `openai/outputTemplate` for `ChatGPT` compatibility.
///
/// Backward compatible: `from_metadata()` reads both nested `"ui"` object
/// and legacy flat `"ui/resourceUri"` key.
///
/// # Example
///
/// ```rust
/// use pmcp::types::ui::ToolUIMetadata;
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// // Nested format (preferred)
/// let mut meta = HashMap::new();
/// meta.insert("ui".to_string(), serde_json::json!({"resourceUri": "ui://settings/form"}));
///
/// let ui_meta = ToolUIMetadata::from_metadata(&meta);
/// assert_eq!(ui_meta.ui_resource_uri, Some("ui://settings/form".to_string()));
///
/// // Legacy flat format also works
/// let mut legacy = HashMap::new();
/// legacy.insert("ui/resourceUri".to_string(), Value::String("ui://settings/form".to_string()));
///
/// let ui_meta = ToolUIMetadata::from_metadata(&legacy);
/// assert_eq!(ui_meta.ui_resource_uri, Some("ui://settings/form".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ToolUIMetadata {
    /// UI resource URI (from `_meta.ui.resourceUri` or legacy `_meta["ui/resourceUri"]`)
    pub ui_resource_uri: Option<String>,

    /// Additional metadata fields
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
    ///
    /// Reads from nested `"ui"` object first, falling back to legacy flat
    /// `"ui/resourceUri"` key for backward compatibility.
    pub fn from_metadata(metadata: &HashMap<String, serde_json::Value>) -> Self {
        // Try nested format first: {"ui": {"resourceUri": "..."}}
        let ui_resource_uri = metadata
            .get("ui")
            .and_then(|v| v.get("resourceUri"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            // Fall back to legacy flat format: {"ui/resourceUri": "..."}
            .or_else(|| {
                metadata
                    .get("ui/resourceUri")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let mut additional = metadata.clone();
        additional.remove("ui");
        additional.remove("ui/resourceUri");
        additional.remove("openai/outputTemplate");

        Self {
            ui_resource_uri,
            additional,
        }
    }

    /// Convert to metadata `HashMap`
    ///
    /// Emits nested `"ui"` format plus `"openai/outputTemplate"` for `ChatGPT`.
    pub fn to_metadata(&self) -> HashMap<String, serde_json::Value> {
        let mut map = self.additional.clone();
        if let Some(uri) = &self.ui_resource_uri {
            map.insert(
                "ui".to_string(),
                serde_json::json!({ "resourceUri": uri }),
            );
            map.insert(
                "openai/outputTemplate".to_string(),
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
        assert_eq!(UIMimeType::HtmlMcpApp.as_str(), "text/html;profile=mcp-app");
        assert_eq!(
            UIMimeType::from_str("text/html+mcp"),
            Ok(UIMimeType::HtmlMcp)
        );
        assert_eq!(
            UIMimeType::from_str("text/html+skybridge"),
            Ok(UIMimeType::HtmlSkybridge)
        );
        assert_eq!(
            UIMimeType::from_str("text/html;profile=mcp-app"),
            Ok(UIMimeType::HtmlMcpApp)
        );
        assert!(UIMimeType::from_str("invalid").is_err());
    }

    #[test]
    fn test_mime_type_platform_checks() {
        assert!(UIMimeType::HtmlSkybridge.is_chatgpt());
        assert!(!UIMimeType::HtmlSkybridge.is_mcp_apps());
        assert!(UIMimeType::HtmlMcp.is_mcp_apps());
        assert!(!UIMimeType::HtmlMcp.is_chatgpt());
        assert!(UIMimeType::HtmlMcpApp.is_chatgpt());
        assert!(!UIMimeType::HtmlMcpApp.is_mcp_apps());
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
    fn test_tool_ui_metadata_to_nested_format() {
        let meta = ToolUIMetadata::new().with_ui_resource("ui://test");

        assert_eq!(meta.ui_resource_uri, Some("ui://test".to_string()));

        let map = meta.to_metadata();
        // Must emit nested format
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://test");
        // Must emit openai/outputTemplate
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&serde_json::Value::String("ui://test".to_string()))
        );
        // Must NOT emit flat key
        assert!(
            map.get("ui/resourceUri").is_none(),
            "must not have flat ui/resourceUri key"
        );
    }

    #[test]
    fn test_tool_ui_metadata_from_nested_format() {
        let mut map = HashMap::new();
        map.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://test"}),
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

    #[test]
    fn test_tool_ui_metadata_from_legacy_flat_format() {
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
