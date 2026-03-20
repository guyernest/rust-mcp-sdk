//! Notification types for MCP protocol.
//!
//! This module contains notification-related types including progress,
//! cancellation, logging, and resource update notifications.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
