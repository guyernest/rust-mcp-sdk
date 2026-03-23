//! Content types for MCP protocol messages.
//!
//! This module contains the content representation types used in tool results,
//! prompt messages, sampling messages, and resource responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Content annotations providing metadata hints (MCP 2025-11-25).
///
/// # Construction
///
/// ```rust
/// use pmcp::types::content::Annotations;
///
/// let ann = Annotations::new()
///     .with_priority(0.8)
///     .with_audience(vec!["user".into()]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    /// Target audience for this content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    /// Priority hint (0.0 = lowest, 1.0 = highest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    /// ISO 8601 timestamp of last modification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

impl Annotations {
    /// Create empty annotations with all fields set to `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the target audience.
    pub fn with_audience(mut self, audience: Vec<String>) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Set the priority hint (0.0 = lowest, 1.0 = highest).
    pub fn with_priority(mut self, priority: f64) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set the last-modified ISO 8601 timestamp.
    pub fn with_last_modified(mut self, last_modified: impl Into<String>) -> Self {
        self.last_modified = Some(last_modified.into());
        self
    }
}

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
        /// Optional metadata for resource content (e.g., widget metadata for MCP Apps)
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, serde_json::Value>>,
    },
    /// Audio content (MCP 2025-11-25)
    #[serde(rename = "audio", rename_all = "camelCase")]
    Audio {
        /// Base64-encoded audio data
        data: String,
        /// Audio MIME type (e.g., "audio/wav", "audio/mp3")
        mime_type: String,
        /// Optional content annotations
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Annotations>,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
    /// Resource link content (MCP 2025-11-25).
    /// Boxed to avoid inflating the Content enum size — `ResourceLink` has ~264 bytes
    /// of fields while Text has ~24 bytes.
    #[serde(rename = "resource_link")]
    ResourceLink(Box<ResourceLinkContent>),
}

impl Content {
    /// Create text content.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::text("Hello, world!");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create image content from base64-encoded data.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::image("iVBORw0KGgo=", "image/png");
    /// ```
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Create a minimal resource reference (URI only).
    ///
    /// Use [`Content::resource_with_text`] when you have content to include.
    pub fn resource(uri: impl Into<String>) -> Self {
        Self::Resource {
            uri: uri.into(),
            text: None,
            mime_type: None,
            meta: None,
        }
    }

    /// Create a resource reference with text content and MIME type.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_with_text("file://test.txt", "hello", "text/plain");
    /// ```
    pub fn resource_with_text(
        uri: impl Into<String>,
        text: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self::Resource {
            uri: uri.into(),
            text: Some(text.into()),
            mime_type: Some(mime_type.into()),
            meta: None,
        }
    }

    /// Create audio content from base64-encoded data.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::audio("base64data==", "audio/wav");
    /// ```
    pub fn audio(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Audio {
            data: data.into(),
            mime_type: mime_type.into(),
            annotations: None,
            meta: None,
        }
    }

    /// Create a resource link.
    ///
    /// Delegates to [`ResourceLinkContent::new`] and wraps in a `Box`.
    ///
    /// ```rust
    /// use pmcp::types::Content;
    ///
    /// let c = Content::resource_link("my-file", "file:///path/to/file.txt");
    /// ```
    pub fn resource_link(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self::ResourceLink(Box::new(ResourceLinkContent::new(name, uri)))
    }
}

/// Resource link content fields (MCP 2025-11-25).
///
/// # Construction
///
/// ```rust
/// use pmcp::types::content::ResourceLinkContent;
///
/// let rl = ResourceLinkContent::new("my-file", "file:///path/to/file.txt")
///     .with_title("My File")
///     .with_mime_type("text/plain");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ResourceLinkContent {
    /// Resource name
    pub name: String,
    /// Resource URI
    pub uri: String,
    /// Optional title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional content annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    /// Optional metadata
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

impl ResourceLinkContent {
    /// Create a new resource link with the required name and URI fields.
    ///
    /// All optional fields default to `None`.
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uri: uri.into(),
            title: None,
            description: None,
            mime_type: None,
            icons: None,
            annotations: None,
            meta: None,
        }
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the icons.
    pub fn with_icons(mut self, icons: Vec<super::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set content annotations.
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Set metadata.
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self.meta = Some(meta);
        self
    }
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

/// Custom serde for `ReadResourceResult.contents`.
///
/// MCP spec defines `ReadResourceResult.contents` as `ResourceContents[]` --
/// plain objects with `uri`, `mimeType`, and `text`/`blob` fields but NO `type`
/// discriminator. The SDK reuses [`Content`] (a tagged enum) for convenience,
/// so this module strips the `type` tag on serialization and tolerates its
/// absence on deserialization.
#[allow(clippy::redundant_pub_crate)]
pub(crate) mod resource_contents_serde {
    use super::Content;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S>(contents: &[Content], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(contents.len()))?;
        for content in contents {
            match content {
                Content::Resource {
                    uri,
                    text,
                    mime_type,
                    meta,
                } => {
                    #[derive(Serialize)]
                    #[serde(rename_all = "camelCase")]
                    struct Rc<'a> {
                        uri: &'a str,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        mime_type: &'a Option<String>,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        text: &'a Option<String>,
                        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
                        meta: &'a Option<serde_json::Map<String, serde_json::Value>>,
                    }
                    seq.serialize_element(&Rc {
                        uri,
                        mime_type,
                        text,
                        meta,
                    })?;
                },
                Content::Text { text } => {
                    #[derive(Serialize)]
                    struct Tc<'a> {
                        text: &'a str,
                    }
                    seq.serialize_element(&Tc { text })?;
                },
                other @ (Content::Image { .. }
                | Content::Audio { .. }
                | Content::ResourceLink { .. }) => {
                    seq.serialize_element(other)?;
                },
            }
        }
        seq.end()
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Content>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
        let mut contents = Vec::with_capacity(values.len());
        for value in values {
            if value.get("type").is_some() {
                // Tagged Content -- standard deserialization
                contents.push(
                    serde_json::from_value::<Content>(value).map_err(serde::de::Error::custom)?,
                );
            } else if let Some(uri) = value.get("uri").and_then(|v| v.as_str()) {
                // Untagged ResourceContents from MCP spec (has uri)
                let text = value.get("text").and_then(|v| v.as_str()).map(String::from);
                let mime_type = value
                    .get("mimeType")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let meta = value.get("_meta").and_then(|v| v.as_object()).cloned();
                contents.push(Content::Resource {
                    uri: uri.to_string(),
                    text,
                    mime_type,
                    meta,
                });
            } else if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                // Text-only content (no type tag, no uri)
                contents.push(Content::Text {
                    text: text.to_string(),
                });
            }
        }
        Ok(contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_content() {
        let content = Content::text("Hello");
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_content_resource_meta_serialization() {
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(
            "widgetDescription".to_string(),
            serde_json::Value::String("A chess board widget".to_string()),
        );
        let content = Content::Resource {
            uri: "ui://chess/board".to_string(),
            text: Some("<html>chess</html>".to_string()),
            mime_type: Some("text/html;profile=mcp-app".to_string()),
            meta: Some(meta_map),
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["_meta"]["widgetDescription"], "A chess board widget");
        assert_eq!(json["uri"], "ui://chess/board");
    }

    #[test]
    fn test_content_resource_no_meta_serialization() {
        let content = Content::resource_with_text("file:///test.txt", "hello", "text/plain");
        let json = serde_json::to_value(&content).unwrap();
        assert!(json.get("_meta").is_none());
        assert_eq!(json["uri"], "file:///test.txt");
    }

    #[test]
    fn test_content_resource_meta_deserialization() {
        let json = json!({
            "type": "resource",
            "uri": "ui://widget",
            "text": "<html></html>",
            "mimeType": "text/html",
            "_meta": {
                "widgetDescription": "test widget",
                "csp": { "connectDomains": ["https://api.example.com"] }
            }
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "ui://widget");
                let meta = meta.unwrap();
                assert_eq!(meta["widgetDescription"], "test widget");
                assert!(meta.contains_key("csp"));
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_content_resource_backward_compat() {
        let json = json!({
            "type": "resource",
            "uri": "file:///old.txt",
            "text": "old content",
            "mimeType": "text/plain"
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "file:///old.txt");
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_audio_content_serialization_roundtrip() {
        let content = Content::Audio {
            data: "base64audiodata==".to_string(),
            mime_type: "audio/wav".to_string(),
            annotations: Some(Annotations::new().with_priority(0.8)),
            meta: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "audio");
        assert_eq!(json["data"], "base64audiodata==");
        assert_eq!(json["mimeType"], "audio/wav");
        assert_eq!(json["annotations"]["priority"], 0.8);

        let roundtrip: Content = serde_json::from_value(json).unwrap();
        match roundtrip {
            Content::Audio {
                data, mime_type, ..
            } => {
                assert_eq!(data, "base64audiodata==");
                assert_eq!(mime_type, "audio/wav");
            },
            _ => panic!("Expected Content::Audio"),
        }
    }

    #[test]
    fn test_resource_link_content_serialization_roundtrip() {
        let content = Content::ResourceLink(Box::new(
            ResourceLinkContent::new("my-file", "file:///path/to/file.txt")
                .with_title("My File")
                .with_description("A file resource")
                .with_mime_type("text/plain"),
        ));
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "resource_link");
        assert_eq!(json["name"], "my-file");
        assert_eq!(json["uri"], "file:///path/to/file.txt");
        assert_eq!(json["title"], "My File");

        let roundtrip: Content = serde_json::from_value(json).unwrap();
        match roundtrip {
            Content::ResourceLink(rl) => {
                assert_eq!(rl.name, "my-file");
                assert_eq!(rl.uri, "file:///path/to/file.txt");
            },
            _ => panic!("Expected Content::ResourceLink"),
        }
    }

    #[test]
    fn test_annotations_default() {
        let ann = Annotations::new();
        assert!(ann.audience.is_none());
        assert!(ann.priority.is_none());
        assert!(ann.last_modified.is_none());
    }

    #[test]
    fn test_content_text_helper() {
        let c = Content::text("Hello");
        match c {
            Content::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Content::Text"),
        }
    }

    #[test]
    fn test_content_image_helper() {
        let c = Content::image("data==", "image/png");
        match c {
            Content::Image { data, mime_type } => {
                assert_eq!(data, "data==");
                assert_eq!(mime_type, "image/png");
            },
            _ => panic!("Expected Content::Image"),
        }
    }

    #[test]
    fn test_content_resource_helper() {
        let c = Content::resource("file://test.txt");
        match c {
            Content::Resource {
                uri,
                text,
                mime_type,
                meta,
            } => {
                assert_eq!(uri, "file://test.txt");
                assert!(text.is_none());
                assert!(mime_type.is_none());
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_content_audio_helper() {
        let c = Content::audio("audiodata==", "audio/wav");
        match c {
            Content::Audio {
                data,
                mime_type,
                annotations,
                meta,
            } => {
                assert_eq!(data, "audiodata==");
                assert_eq!(mime_type, "audio/wav");
                assert!(annotations.is_none());
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Audio"),
        }
    }

    #[test]
    fn test_content_resource_link_helper() {
        let c = Content::resource_link("my-file", "file:///path");
        match c {
            Content::ResourceLink(rl) => {
                assert_eq!(rl.name, "my-file");
                assert_eq!(rl.uri, "file:///path");
                assert!(rl.title.is_none());
            },
            _ => panic!("Expected Content::ResourceLink"),
        }
    }

    #[test]
    fn test_annotations_with_methods() {
        let ann = Annotations::new()
            .with_priority(0.9)
            .with_audience(vec!["user".into(), "admin".into()])
            .with_last_modified("2025-01-01T00:00:00Z");
        assert_eq!(ann.priority, Some(0.9));
        assert_eq!(ann.audience.as_ref().unwrap().len(), 2);
        assert_eq!(ann.last_modified.as_deref(), Some("2025-01-01T00:00:00Z"));
    }

    #[test]
    fn test_resource_link_content_with_methods() {
        let rl = ResourceLinkContent::new("test", "file:///test")
            .with_title("Test")
            .with_description("A test resource")
            .with_mime_type("text/plain");
        assert_eq!(rl.name, "test");
        assert_eq!(rl.uri, "file:///test");
        assert_eq!(rl.title.as_deref(), Some("Test"));
        assert_eq!(rl.description.as_deref(), Some("A test resource"));
        assert_eq!(rl.mime_type.as_deref(), Some("text/plain"));
    }
}
