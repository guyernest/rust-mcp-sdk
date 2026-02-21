//! Task capability types for MCP server and client initialization.
//!
//! These types are serialized into `experimental.tasks` during capability
//! negotiation. The server advertises what task operations it supports,
//! and the client declares that it supports tasks.
//!
//! # Server Capabilities
//!
//! ```
//! use pmcp_tasks::ServerTaskCapabilities;
//! use serde_json;
//!
//! let caps = ServerTaskCapabilities::full();
//! let json = serde_json::to_value(&caps).unwrap();
//! assert!(json.get("list").is_some());
//! assert!(json.get("cancel").is_some());
//! assert!(json["requests"]["tools"]["call"].is_object());
//! ```

use serde::{Deserialize, Serialize};

/// An empty JSON object `{}` used for boolean-like capability fields.
///
/// The MCP spec uses `{}` to indicate a capability is present (vs. absent/null).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::EmptyObject;
/// use serde_json;
///
/// let obj = EmptyObject {};
/// let json = serde_json::to_value(&obj).unwrap();
/// assert!(json.is_object());
/// assert_eq!(json.as_object().unwrap().len(), 0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyObject {}

/// Server-side task capabilities advertised during initialization.
///
/// Controls which task operations the server supports. Use convenience
/// constructors [`full()`](Self::full) or [`tools_only()`](Self::tools_only)
/// for common configurations.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::ServerTaskCapabilities;
/// use serde_json;
///
/// // Full capabilities: list, cancel, and tools.call
/// let full = ServerTaskCapabilities::full();
/// let json = serde_json::to_value(&full).unwrap();
/// assert!(json.get("list").is_some());
/// assert!(json.get("cancel").is_some());
/// assert!(json["requests"]["tools"]["call"].is_object());
///
/// // Tools-only: just tools.call, no list/cancel
/// let tools = ServerTaskCapabilities::tools_only();
/// let json = serde_json::to_value(&tools).unwrap();
/// assert!(json.get("list").is_none());
/// assert!(json.get("cancel").is_none());
/// assert!(json["requests"]["tools"]["call"].is_object());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTaskCapabilities {
    /// Whether the server supports `tasks/list`. Present as `{}` if supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<EmptyObject>,

    /// Whether the server supports `tasks/cancel`. Present as `{}` if supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<EmptyObject>,

    /// Request-level task capabilities (e.g., which request types support tasks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ServerTaskRequests>,
}

/// Request-level task capability configuration.
///
/// Specifies which request types support task augmentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTaskRequests {
    /// Tool-related task capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsTaskRequests>,
}

/// Tool-specific task request capabilities.
///
/// Specifies whether `tools/call` supports task augmentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsTaskRequests {
    /// Whether `tools/call` supports task augmentation. Present as `{}` if supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<EmptyObject>,
}

/// Client-side task capabilities declared during initialization.
///
/// The client sets `supported: true` to indicate it can handle task
/// responses and notifications.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::ClientTaskCapabilities;
/// use serde_json;
///
/// let caps = ClientTaskCapabilities { supported: true };
/// let json = serde_json::to_value(&caps).unwrap();
/// assert_eq!(json["supported"], true);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTaskCapabilities {
    /// Whether the client supports task responses and notifications.
    pub supported: bool,
}

impl ServerTaskCapabilities {
    /// Creates capabilities with all task operations enabled:
    /// `tasks/list`, `tasks/cancel`, and `tools/call` task augmentation.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::ServerTaskCapabilities;
    ///
    /// let caps = ServerTaskCapabilities::full();
    /// assert!(caps.list.is_some());
    /// assert!(caps.cancel.is_some());
    /// assert!(caps.requests.is_some());
    /// ```
    pub fn full() -> Self {
        Self {
            list: Some(EmptyObject {}),
            cancel: Some(EmptyObject {}),
            requests: Some(ServerTaskRequests {
                tools: Some(ToolsTaskRequests {
                    call: Some(EmptyObject {}),
                }),
            }),
        }
    }

    /// Creates capabilities with only `tools/call` task augmentation.
    ///
    /// The server will not support `tasks/list` or `tasks/cancel`, but
    /// `tools/call` can return `CreateTaskResult` with a task.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::ServerTaskCapabilities;
    ///
    /// let caps = ServerTaskCapabilities::tools_only();
    /// assert!(caps.list.is_none());
    /// assert!(caps.cancel.is_none());
    /// assert!(caps.requests.is_some());
    /// ```
    pub fn tools_only() -> Self {
        Self {
            list: None,
            cancel: None,
            requests: Some(ServerTaskRequests {
                tools: Some(ToolsTaskRequests {
                    call: Some(EmptyObject {}),
                }),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_object_serializes_to_empty_json_object() {
        let obj = EmptyObject {};
        let json = serde_json::to_value(&obj).unwrap();
        assert!(json.is_object());
        assert_eq!(json.as_object().unwrap().len(), 0);
    }

    #[test]
    fn server_task_capabilities_full() {
        let caps = ServerTaskCapabilities::full();
        let json = serde_json::to_value(&caps).unwrap();

        assert!(json.get("list").is_some());
        assert!(json.get("cancel").is_some());
        assert!(json.get("requests").is_some());
        assert!(json["requests"]["tools"]["call"].is_object());
    }

    #[test]
    fn server_task_capabilities_tools_only() {
        let caps = ServerTaskCapabilities::tools_only();
        let json = serde_json::to_value(&caps).unwrap();

        assert!(json.get("list").is_none());
        assert!(json.get("cancel").is_none());
        assert!(json.get("requests").is_some());
        assert!(json["requests"]["tools"]["call"].is_object());
    }

    #[test]
    fn server_task_capabilities_default_is_empty() {
        let caps = ServerTaskCapabilities::default();
        let json = serde_json::to_value(&caps).unwrap();

        assert!(json.get("list").is_none());
        assert!(json.get("cancel").is_none());
        assert!(json.get("requests").is_none());
    }

    #[test]
    fn client_task_capabilities_supported() {
        let caps = ClientTaskCapabilities { supported: true };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["supported"], true);
    }

    #[test]
    fn client_task_capabilities_not_supported() {
        let caps = ClientTaskCapabilities { supported: false };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["supported"], false);
    }

    #[test]
    fn server_task_capabilities_round_trip() {
        let original = ServerTaskCapabilities::full();
        let json_str = serde_json::to_string(&original).unwrap();
        let back: ServerTaskCapabilities = serde_json::from_str(&json_str).unwrap();

        assert!(back.list.is_some());
        assert!(back.cancel.is_some());
        assert!(back.requests.is_some());
    }

    #[test]
    fn full_capabilities_structure_matches_spec() {
        // Spec shape: { list: {}, cancel: {}, requests: { tools: { call: {} } } }
        let caps = ServerTaskCapabilities::full();
        let json = serde_json::to_value(&caps).unwrap();
        let json_str = serde_json::to_string(&json).unwrap();

        // Verify the nested structure exists
        assert!(json_str.contains("\"list\""));
        assert!(json_str.contains("\"cancel\""));
        assert!(json_str.contains("\"requests\""));
        assert!(json_str.contains("\"tools\""));
        assert!(json_str.contains("\"call\""));
    }
}
