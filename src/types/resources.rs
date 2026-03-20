//! Resource types for MCP protocol.
//!
//! This module contains resource-related types including resource information,
//! templates, read/list requests, and subscription types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Content;
use super::protocol::Cursor;
use super::protocol::RequestMeta;

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
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<crate::types::protocol::IconInfo>>,
    /// Optional content annotations (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<crate::types::content::Annotations>,
    /// Optional metadata (e.g., widget descriptor keys for `ChatGPT` MCP Apps)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

/// List resources response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourcesResult;
///
/// let result = ListResourcesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources
    pub resources: Vec<ResourceInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListResourcesResult {
    /// Create a new list resources result.
    pub fn new(resources: Vec<ResourceInfo>) -> Self {
        Self {
            resources,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
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
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Template description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type for resources created from this template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<crate::types::protocol::IconInfo>>,
    /// Optional content annotations (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<crate::types::content::Annotations>,
    /// Optional metadata (MCP 2025-11-25)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

/// List resource templates result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourceTemplatesResult;
///
/// let result = ListResourceTemplatesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesResult {
    /// Available resource templates
    pub resource_templates: Vec<ResourceTemplate>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListResourceTemplatesResult {
    /// Create a new list resource templates result.
    pub fn new(resource_templates: Vec<ResourceTemplate>) -> Self {
        Self {
            resource_templates,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
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

/// Read resource result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ReadResourceResult;
///
/// let result = ReadResourceResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Resource contents.
    ///
    /// Per the MCP spec, these are `ResourceContents` objects (`uri` + `text`/`blob` +
    /// optional `mimeType`). The custom serializer strips the `type` discriminator tag
    /// that [`Content`]'s tagged-enum representation would otherwise emit.
    #[serde(
        serialize_with = "crate::types::content::resource_contents_serde::serialize",
        deserialize_with = "crate::types::content::resource_contents_serde::deserialize"
    )]
    pub contents: Vec<Content>,
}

impl ReadResourceResult {
    /// Create a new read resource result.
    pub fn new(contents: Vec<Content>) -> Self {
        Self { contents }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_types() {
        let resource = ResourceInfo {
            uri: "file://test.txt".to_string(),
            name: "test.txt".to_string(),
            title: None,
            description: Some("Test file".to_string()),
            mime_type: Some("text/plain".to_string()),
            icons: None,
            annotations: None,
            meta: None,
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["uri"], "file://test.txt");
        assert_eq!(json["name"], "test.txt");
        assert_eq!(json["description"], "Test file");
        assert_eq!(json["mimeType"], "text/plain");
    }
}
