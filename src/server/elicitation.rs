//! User input elicitation support for MCP servers (MCP 2025-11-25).
//!
//! This module provides the `ElicitationManager` for handling elicitation
//! requests using the spec-compliant `elicitation/create` method.

use crate::error::{Error, ErrorCode, Result};
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use crate::types::protocol::ServerRequest;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, oneshot, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

/// Monotonically increasing counter for elicitation IDs.
static ELICITATION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Manager for handling input elicitation requests.
pub struct ElicitationManager {
    /// Pending elicitation requests waiting for responses.
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<ElicitResult>>>>,
    /// Channel for sending requests to the client.
    request_tx: Option<mpsc::Sender<ServerRequest>>,
    /// Default timeout for elicitation requests.
    timeout_duration: Duration,
}

impl std::fmt::Debug for ElicitationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElicitationManager")
            .field("has_request_tx", &self.request_tx.is_some())
            .field("timeout_duration", &self.timeout_duration)
            .finish()
    }
}

impl ElicitationManager {
    /// Create a new elicitation manager.
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            request_tx: None,
            timeout_duration: Duration::from_secs(300), // 5 minutes default
        }
    }

    /// Set the request channel for sending elicitation requests.
    pub fn set_request_channel(&mut self, tx: mpsc::Sender<ServerRequest>) {
        self.request_tx = Some(tx);
    }

    /// Set the timeout duration for elicitation requests.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    /// Generate a unique elicitation ID.
    fn next_elicitation_id() -> String {
        let id = ELICITATION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("elicit-{id}")
    }

    /// Request input from the user using the spec-compliant elicitation/create method.
    #[allow(clippy::cognitive_complexity)]
    pub async fn elicit_input(&self, request: ElicitRequestParams) -> Result<ElicitResult> {
        let request_tx = self.request_tx.as_ref().ok_or_else(|| {
            Error::protocol(ErrorCode::INTERNAL_ERROR, "Elicitation not configured")
        })?;

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Generate a correlation ID for tracking the pending request
        let elicitation_id = Self::next_elicitation_id();

        // Store pending request
        {
            let mut pending = self.pending.write().await;
            pending.insert(elicitation_id.clone(), tx);
        }

        // Send elicitation request
        let server_request = ServerRequest::ElicitationCreate(Box::new(request));
        if let Err(e) = request_tx.send(server_request).await {
            // Remove from pending on send error
            self.pending.write().await.remove(&elicitation_id);
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to send elicitation request: {e}"),
            ));
        }

        debug!("Sent elicitation request: {}", elicitation_id);

        // Wait for response with timeout
        match timeout(self.timeout_duration, rx).await {
            Ok(Ok(response)) => {
                debug!("Received elicitation response: {}", elicitation_id);
                Ok(response)
            },
            Ok(Err(_)) => {
                warn!("Elicitation channel closed: {}", elicitation_id);
                Err(Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    "Elicitation channel closed",
                ))
            },
            Err(_) => {
                warn!("Elicitation timeout: {}", elicitation_id);
                self.pending.write().await.remove(&elicitation_id);
                Err(Error::protocol(
                    ErrorCode::REQUEST_TIMEOUT,
                    "Elicitation request timed out",
                ))
            },
        }
    }

    /// Handle an elicitation response from the client.
    ///
    /// The `elicitation_id` parameter correlates this response to the
    /// original pending request.
    pub async fn handle_response(
        &self,
        elicitation_id: &str,
        response: ElicitResult,
    ) -> Result<()> {
        let mut pending = self.pending.write().await;

        if let Some(tx) = pending.remove(elicitation_id) {
            if tx.send(response).is_err() {
                warn!("Failed to deliver elicitation response - receiver dropped");
            }
            Ok(())
        } else {
            warn!(
                "Received response for unknown elicitation: {}",
                elicitation_id
            );
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Unknown elicitation ID",
            ))
        }
    }

    /// Cancel a pending elicitation request.
    pub async fn cancel(&self, elicitation_id: &str) -> Result<()> {
        let mut pending = self.pending.write().await;

        if let Some(tx) = pending.remove(elicitation_id) {
            // Send cancellation response
            let response = ElicitResult {
                action: crate::types::elicitation::ElicitAction::Cancel,
                content: None,
            };

            if tx.send(response).is_err() {
                debug!("Elicitation already completed: {}", elicitation_id);
            }
            Ok(())
        } else {
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Unknown elicitation ID",
            ))
        }
    }

    /// Cancel all pending elicitation requests.
    pub async fn cancel_all(&self) {
        let mut pending = self.pending.write().await;

        for (_id, tx) in pending.drain() {
            let response = ElicitResult {
                action: crate::types::elicitation::ElicitAction::Cancel,
                content: None,
            };

            let _ = tx.send(response);
        }
    }

    /// Get the number of pending elicitation requests.
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

impl Default for ElicitationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for tool handlers to elicit input.
#[async_trait::async_trait]
pub trait ElicitInput {
    /// Request input from the user.
    async fn elicit_input(&self, request: ElicitRequestParams) -> Result<ElicitResult>;
}

/// Context that provides elicitation capabilities to tool handlers.
#[derive(Debug)]
pub struct ElicitationContext {
    manager: Arc<ElicitationManager>,
}

impl ElicitationContext {
    /// Create a new elicitation context.
    pub fn new(manager: Arc<ElicitationManager>) -> Self {
        Self { manager }
    }
}

#[async_trait::async_trait]
impl ElicitInput for ElicitationContext {
    async fn elicit_input(&self, request: ElicitRequestParams) -> Result<ElicitResult> {
        self.manager.elicit_input(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_elicitation_manager_no_channel() {
        let manager = ElicitationManager::new();

        // Should fail without request channel
        let request = ElicitRequestParams::Form {
            message: "Test prompt".to_string(),
            requested_schema: serde_json::json!({"type": "object"}),
        };
        let result = manager.elicit_input(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_elicitation_timeout() {
        let (tx, mut _rx) = mpsc::channel(10);
        let mut manager = ElicitationManager::new();
        manager.set_request_channel(tx);
        manager.set_timeout(Duration::from_millis(50)); // Short timeout for test

        let request = ElicitRequestParams::Form {
            message: "Test prompt".to_string(),
            requested_schema: serde_json::json!({"type": "object"}),
        };

        // Should timeout since nobody responds
        let result = manager.elicit_input(request).await;
        assert!(result.is_err());
    }
}
