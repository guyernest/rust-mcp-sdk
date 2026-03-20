//! Content types for MCP protocol messages.
//!
//! This module contains the content representation types used in tool results,
//! prompt messages, sampling messages, and resource responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;


/// Content annotations providing metadata hints (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Resource link content (MCP 2025-11-25)
    #[serde(rename = "resource_link", rename_all = "camelCase")]
    ResourceLink {
        /// Resource name
        name: String,
        /// Resource URI
        uri: String,
        /// Optional title
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Optional description
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Optional MIME type
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        /// Optional icons
        #[serde(skip_serializing_if = "Option::is_none")]
        icons: Option<Vec<super::protocol::IconInfo>>,
        /// Optional content annotations
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Annotations>,
        /// Optional metadata
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
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
                other @ Content::Image { .. }
                | other @ Content::Audio { .. }
                | other @ Content::ResourceLink { .. } => {
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
        let content = Content::Text {
            text: "Hello".to_string(),
        };
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
        let content = Content::Resource {
            uri: "file:///test.txt".to_string(),
            text: Some("hello".to_string()),
            mime_type: Some("text/plain".to_string()),
            meta: None,
        };
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
            annotations: Some(Annotations {
                priority: Some(0.8),
                ..Default::default()
            }),
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
        let content = Content::ResourceLink {
            name: "my-file".to_string(),
            uri: "file:///path/to/file.txt".to_string(),
            title: Some("My File".to_string()),
            description: Some("A file resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            icons: None,
            annotations: None,
            meta: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "resource_link");
        assert_eq!(json["name"], "my-file");
        assert_eq!(json["uri"], "file:///path/to/file.txt");
        assert_eq!(json["title"], "My File");

        let roundtrip: Content = serde_json::from_value(json).unwrap();
        match roundtrip {
            Content::ResourceLink { name, uri, .. } => {
                assert_eq!(name, "my-file");
                assert_eq!(uri, "file:///path/to/file.txt");
            },
            _ => panic!("Expected Content::ResourceLink"),
        }
    }

    #[test]
    fn test_annotations_default() {
        let ann = Annotations::default();
        assert!(ann.audience.is_none());
        assert!(ann.priority.is_none());
        assert!(ann.last_modified.is_none());
    }
}
