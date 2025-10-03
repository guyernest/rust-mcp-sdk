//! Type-safe handles for tools and resources
//!
//! Handles are lightweight identifiers using Arc<str> for O(1) cloning.

use super::{error::WorkflowError, newtypes::Uri};
use std::sync::Arc;

/// Type-safe identifier for a tool
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolHandle {
    name: Arc<str>,
}

impl ToolHandle {
    /// Create a new tool handle
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: Arc::from(name.as_ref()),
        }
    }

    /// Get the tool name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for ToolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<&str> for ToolHandle {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ToolHandle {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Type-safe identifier for a resource
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceHandle {
    uri: Uri,
}

impl ResourceHandle {
    /// Create a new resource handle with URI validation
    pub fn new(uri: impl AsRef<str>) -> Result<Self, WorkflowError> {
        Ok(Self {
            uri: Uri::new(uri)?,
        })
    }

    /// Get the resource URI
    pub fn uri(&self) -> &str {
        self.uri.as_str()
    }
}

impl std::fmt::Display for ResourceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uri.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_handle() {
        let handle = ToolHandle::new("greet");
        assert_eq!(handle.name(), "greet");
        assert_eq!(handle.to_string(), "greet");
    }

    #[test]
    fn test_tool_handle_from_str() {
        let handle: ToolHandle = "greet".into();
        assert_eq!(handle.name(), "greet");
    }

    #[test]
    fn test_tool_handle_equality() {
        let h1 = ToolHandle::new("greet");
        let h2 = ToolHandle::new("greet");
        let h3 = ToolHandle::new("farewell");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_resource_handle() {
        let handle = ResourceHandle::new("resource://test/path").unwrap();
        assert_eq!(handle.uri(), "resource://test/path");
    }

    #[test]
    fn test_resource_handle_invalid_uri() {
        let result = ResourceHandle::new("http://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_handles_are_cheap_to_clone() {
        let handle1 = ToolHandle::new("greet");
        let handle2 = handle1.clone();

        // Arc pointer equality
        assert_eq!(handle1.name(), handle2.name());
    }

    #[test]
    fn test_tool_handle_display() {
        let handle = ToolHandle::new("greet");
        assert_eq!(format!("{}", handle), "greet");
    }

    #[test]
    fn test_tool_handle_from_string() {
        let handle: ToolHandle = String::from("greet").into();
        assert_eq!(handle.name(), "greet");
    }

    #[test]
    fn test_tool_handle_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ToolHandle::new("greet"));
        set.insert(ToolHandle::new("greet")); // Duplicate
        set.insert(ToolHandle::new("farewell"));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_tool_handle_with_special_characters() {
        let handle = ToolHandle::new("my-tool_v2.0");
        assert_eq!(handle.name(), "my-tool_v2.0");
    }

    #[test]
    fn test_tool_handle_empty_string() {
        let handle = ToolHandle::new("");
        assert_eq!(handle.name(), "");
    }

    #[test]
    fn test_resource_handle_display() {
        let handle = ResourceHandle::new("resource://test/path").unwrap();
        assert_eq!(format!("{}", handle), "resource://test/path");
    }

    #[test]
    fn test_resource_handle_equality() {
        let h1 = ResourceHandle::new("resource://test/path").unwrap();
        let h2 = ResourceHandle::new("resource://test/path").unwrap();
        let h3 = ResourceHandle::new("file:///different").unwrap();

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_resource_handle_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ResourceHandle::new("resource://test/path").unwrap());
        set.insert(ResourceHandle::new("resource://test/path").unwrap()); // Duplicate
        set.insert(ResourceHandle::new("file:///different").unwrap());

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_resource_handle_file_scheme() {
        let handle = ResourceHandle::new("file:///absolute/path");
        assert!(handle.is_ok());
        assert_eq!(handle.unwrap().uri(), "file:///absolute/path");
    }

    #[test]
    fn test_resource_handle_validation_errors() {
        assert!(ResourceHandle::new("http://example.com").is_err());
        assert!(ResourceHandle::new("https://example.com").is_err());
        assert!(ResourceHandle::new("./relative/path").is_err());
        assert!(ResourceHandle::new("not-a-uri").is_err());
    }

    #[test]
    fn test_resource_handle_clone() {
        let h1 = ResourceHandle::new("resource://test/path").unwrap();
        let h2 = h1.clone();
        assert_eq!(h1.uri(), h2.uri());
    }

    #[test]
    fn test_handles_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ToolHandle>();
        assert_send_sync::<ResourceHandle>();
    }

    #[test]
    fn test_tool_handle_debug() {
        let handle = ToolHandle::new("greet");
        let debug = format!("{:?}", handle);
        assert!(debug.contains("ToolHandle"));
    }

    #[test]
    fn test_resource_handle_debug() {
        let handle = ResourceHandle::new("resource://test/path").unwrap();
        let debug = format!("{:?}", handle);
        assert!(debug.contains("ResourceHandle"));
    }
}
