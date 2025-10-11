//! Request cancellation support for MCP server.

use crate::error::Result;
use crate::types::protocol::{CancelledNotification, Notification};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio_util::sync::CancellationToken;

/// Manages cancellation tokens for requests.
pub struct CancellationManager {
    tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    notification_sender: Option<Arc<dyn Fn(Notification) + Send + Sync>>,
}

impl CancellationManager {
    /// Create a new cancellation manager.
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
        }
    }

    /// Set the notification sender.
    pub fn set_notification_sender(&mut self, sender: Arc<dyn Fn(Notification) + Send + Sync>) {
        self.notification_sender = Some(sender);
    }

    /// Create a cancellation token for a request.
    pub async fn create_token(&self, request_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        let mut tokens = self.tokens.write().await;
        tokens.insert(request_id, token.clone());
        token
    }

    /// Cancel a request by ID.
    pub async fn cancel_request(&self, request_id: String, reason: Option<String>) -> Result<()> {
        let token = {
            let mut tokens = self.tokens.write().await;
            tokens.remove(&request_id)
        };

        if let Some(token) = token {
            // Cancel the token
            token.cancel();

            // Send cancellation notification
            if let Some(sender) = &self.notification_sender {
                let notification = Notification::Client(
                    crate::types::ClientNotification::Cancelled(CancelledNotification {
                        request_id: crate::types::RequestId::String(request_id.clone()),
                        reason: Some(reason.unwrap_or_else(|| "Cancelled by server".to_string())),
                    }),
                );
                sender(notification);
            }
        }

        Ok(())
    }

    /// Remove a completed request's token.
    pub async fn remove_token(&self, request_id: &str) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(request_id);
    }

    /// Check if a request is cancelled.
    pub async fn is_cancelled(&self, request_id: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens
            .get(request_id)
            .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
    }

    /// Get the cancellation token for a request.
    pub async fn get_token(&self, request_id: &str) -> Option<CancellationToken> {
        let tokens = self.tokens.read().await;
        tokens.get(request_id).cloned()
    }

    /// Clear all cancellation tokens.
    pub async fn clear(&self) {
        let mut tokens = self.tokens.write().await;
        // Cancel all active tokens
        for token in tokens.values() {
            token.cancel();
        }
        tokens.clear();
    }
}

impl Default for CancellationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancellationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationManager")
            .field(
                "active_tokens",
                &self.tokens.try_read().map(|t| t.len()).unwrap_or(0),
            )
            .finish()
    }
}

/// Extra context passed to request handlers.
#[derive(Clone)]
pub struct RequestHandlerExtra {
    /// Cancellation token for the request
    pub cancellation_token: CancellationToken,
    /// Request ID
    pub request_id: String,
    /// Session ID
    pub session_id: Option<String>,
    /// Authentication info
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    /// Validated authentication context (if auth is enabled)
    pub auth_context: Option<crate::server::auth::AuthContext>,
    /// Custom metadata for middleware (e.g., OAuth tokens, session data)
    ///
    /// **Security Note**: Metadata may contain sensitive values like OAuth tokens.
    /// The Debug implementation redacts these values to prevent accidental logging.
    pub metadata: HashMap<String, String>,
}

impl RequestHandlerExtra {
    /// Create new handler extra context.
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set the auth info.
    pub fn with_auth_info(mut self, auth_info: Option<crate::types::auth::AuthInfo>) -> Self {
        self.auth_info = auth_info;
        self
    }

    /// Set the auth context.
    pub fn with_auth_context(
        mut self,
        auth_context: Option<crate::server::auth::AuthContext>,
    ) -> Self {
        self.auth_context = auth_context;
        self
    }

    /// Get the auth context if available.
    pub fn auth_context(&self) -> Option<&crate::server::auth::AuthContext> {
        self.auth_context.as_ref()
    }

    /// Get metadata value by key.
    ///
    /// Metadata is typically set by middleware (e.g., OAuth token injection).
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Set metadata value.
    ///
    /// This is typically used by middleware to inject data for tools to consume.
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Check if the request has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Wait for cancellation.
    pub async fn cancelled(&self) {
        self.cancellation_token.cancelled().await;
    }
}

impl std::fmt::Debug for RequestHandlerExtra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // List of sensitive metadata keys that should be redacted
        const SENSITIVE_KEYS: &[&str] = &[
            "oauth_token",
            "access_token",
            "refresh_token",
            "api_key",
            "secret",
            "password",
            "bearer_token",
            "auth_token",
        ];

        // Create a redacted version of metadata
        let redacted_metadata: HashMap<String, String> = self
            .metadata
            .iter()
            .map(|(k, v)| {
                let is_sensitive = SENSITIVE_KEYS
                    .iter()
                    .any(|sensitive| k.to_lowercase().contains(sensitive));
                if is_sensitive {
                    (k.clone(), "[REDACTED]".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();

        f.debug_struct("RequestHandlerExtra")
            .field("cancellation_token", &self.cancellation_token)
            .field("request_id", &self.request_id)
            .field("session_id", &self.session_id)
            .field("auth_info", &self.auth_info)
            .field("auth_context", &self.auth_context)
            .field("metadata", &redacted_metadata)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_cancel_token() {
        let manager = CancellationManager::new();

        // Create a token
        let token = manager.create_token("test-request".to_string()).await;
        assert!(!token.is_cancelled());

        // Cancel the request
        manager
            .cancel_request("test-request".to_string(), None)
            .await
            .unwrap();

        // Token should be cancelled
        assert!(token.is_cancelled());

        // Token should be removed from manager
        assert!(manager.get_token("test-request").await.is_none());
    }

    #[tokio::test]
    async fn test_cancel_with_reason() {
        let manager = CancellationManager::new();

        // Set up notification tracking
        let notifications = Arc::new(RwLock::new(Vec::new()));
        let notifications_clone = notifications.clone();

        let mut manager = manager;
        manager.set_notification_sender(Arc::new(move |notif| {
            let notifications = notifications_clone.clone();
            tokio::spawn(async move {
                notifications.write().await.push(notif);
            });
        }));

        // Create and cancel with reason
        let _token = manager.create_token("test-request".to_string()).await;
        manager
            .cancel_request("test-request".to_string(), Some("Test reason".to_string()))
            .await
            .unwrap();

        // Give notification time to be sent
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Check notification was sent
        let notifs = notifications.read().await;
        assert_eq!(notifs.len(), 1);

        if let Notification::Client(crate::types::ClientNotification::Cancelled(cancelled)) =
            &notifs[0]
        {
            assert_eq!(
                cancelled.request_id,
                crate::types::RequestId::String("test-request".to_string())
            );
            assert_eq!(cancelled.reason, Some("Test reason".to_string()));
        } else {
            panic!("Expected Cancelled notification");
        }
    }

    #[tokio::test]
    async fn test_remove_token() {
        let manager = CancellationManager::new();

        // Create a token
        let token = manager.create_token("test-request".to_string()).await;
        assert!(manager.get_token("test-request").await.is_some());

        // Remove the token
        manager.remove_token("test-request").await;
        assert!(manager.get_token("test-request").await.is_none());

        // Token should still be valid (not cancelled)
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn test_clear_all_tokens() {
        let manager = CancellationManager::new();

        // Create multiple tokens
        let token1 = manager.create_token("request1".to_string()).await;
        let token2 = manager.create_token("request2".to_string()).await;
        let token3 = manager.create_token("request3".to_string()).await;

        // Clear all tokens
        manager.clear().await;

        // All tokens should be cancelled
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
        assert!(token3.is_cancelled());

        // Manager should have no tokens
        assert!(manager.get_token("request1").await.is_none());
        assert!(manager.get_token("request2").await.is_none());
        assert!(manager.get_token("request3").await.is_none());
    }

    #[tokio::test]
    async fn test_request_handler_extra() {
        let token = CancellationToken::new();
        let extra = RequestHandlerExtra::new("test-req".to_string(), token.clone())
            .with_session_id(Some("session-123".to_string()));

        assert_eq!(extra.request_id, "test-req");
        assert_eq!(extra.session_id, Some("session-123".to_string()));
        assert!(!extra.is_cancelled());

        // Cancel the token
        token.cancel();
        assert!(extra.is_cancelled());
    }

    #[tokio::test]
    async fn test_metadata_redaction_in_debug() {
        let token = CancellationToken::new();
        let mut extra = RequestHandlerExtra::new("test-req".to_string(), token);

        // Add sensitive and non-sensitive metadata
        extra.set_metadata("oauth_token".to_string(), "secret-token-123".to_string());
        extra.set_metadata("access_token".to_string(), "bearer-xyz".to_string());
        extra.set_metadata("user_id".to_string(), "user-456".to_string());
        extra.set_metadata("request_count".to_string(), "42".to_string());

        // Get debug representation
        let debug_output = format!("{:?}", extra);

        // Verify sensitive values are redacted
        assert!(
            debug_output.contains("[REDACTED]"),
            "Expected redacted values in: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("secret-token-123"),
            "OAuth token should be redacted: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("bearer-xyz"),
            "Access token should be redacted: {}",
            debug_output
        );

        // Verify non-sensitive values are not redacted
        assert!(
            debug_output.contains("user-456"),
            "Non-sensitive metadata should not be redacted: {}",
            debug_output
        );
        assert!(
            debug_output.contains("42"),
            "Non-sensitive metadata should not be redacted: {}",
            debug_output
        );
    }
}
