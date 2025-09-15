//! Protocol implementation for MCP.
//!
//! This module provides the core protocol state machine and request handling.

use crate::error::Result;
use crate::shared::runtime::{self, Mutex};
use crate::types::{JSONRPCResponse, RequestId};
#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::oneshot;

/// Progress callback type.
pub type ProgressCallback = Box<dyn Fn(u64, Option<u64>) + Send + Sync>;

/// Protocol options for configuring behavior.
#[derive(Debug, Clone, Default)]
pub struct ProtocolOptions {
    /// Whether to enforce strict capability checking.
    pub enforce_strict_capabilities: bool,
    /// Methods that should be debounced.
    pub debounced_notification_methods: Vec<String>,
}

/// Request options for individual requests.
#[derive(Default)]
pub struct RequestOptions {
    /// Timeout for the request.
    pub timeout: Option<Duration>,
    /// Progress callback.
    pub on_progress: Option<ProgressCallback>,
}

impl std::fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestOptions")
            .field("timeout", &self.timeout)
            .field(
                "on_progress",
                &self.on_progress.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

/// Unique identifier for a transport instance.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::protocol::TransportId;
/// use std::collections::HashSet;
///
/// // Create a new unique transport ID
/// let id1 = TransportId::new();
/// let id2 = TransportId::new();
/// assert_ne!(id1, id2);
///
/// // Create from a specific string
/// let id3 = TransportId::from_string("custom-transport-1".to_string());
/// let id4 = TransportId::from_string("custom-transport-1".to_string());
/// assert_eq!(id3, id4);
///
/// // Use in collections
/// let mut transports = HashSet::new();
/// transports.insert(id1.clone());
/// transports.insert(id2.clone());
/// assert_eq!(transports.len(), 2);
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TransportId(Arc<str>);

impl TransportId {
    /// Create a new transport ID.
    pub fn new() -> Self {
        Self(Arc::from(uuid::Uuid::new_v4().to_string()))
    }

    /// Create a transport ID from a string.
    pub fn from_string(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl Default for TransportId {
    fn default() -> Self {
        Self::new()
    }
}

/// Request context containing transport information.
#[derive(Debug, Clone)]
struct RequestContext {
    /// Transport ID that initiated the request.
    transport_id: TransportId,
    /// Response sender.
    sender: Arc<Mutex<Option<oneshot::Sender<JSONRPCResponse>>>>,
}

/// Protocol state machine for handling JSON-RPC communication.
#[derive(Debug)]
pub struct Protocol {
    /// Protocol options.
    options: ProtocolOptions,
    /// Pending requests waiting for responses, keyed by request ID.
    /// Each request stores the transport ID to ensure responses go to the correct transport.
    pending_requests: HashMap<RequestId, RequestContext>,
    /// Current transport ID for this protocol instance.
    transport_id: TransportId,
}

impl Protocol {
    /// Create a new protocol instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::protocol::{Protocol, ProtocolOptions};
    ///
    /// // Create with default options
    /// let protocol = Protocol::new(ProtocolOptions::default());
    ///
    /// // Create with custom options
    /// let options = ProtocolOptions {
    ///     enforce_strict_capabilities: true,
    ///     debounced_notification_methods: vec!["progress".to_string()],
    /// };
    /// let protocol = Protocol::new(options);
    /// ```
    pub fn new(options: ProtocolOptions) -> Self {
        Self {
            options,
            pending_requests: HashMap::new(),
            transport_id: TransportId::new(),
        }
    }

    /// Create a new protocol instance with a specific transport ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::protocol::{Protocol, ProtocolOptions, TransportId};
    ///
    /// // Create with a specific transport ID
    /// let transport_id = TransportId::from_string("websocket-1".to_string());
    /// let protocol = Protocol::with_transport_id(
    ///     ProtocolOptions::default(),
    ///     transport_id.clone()
    /// );
    ///
    /// // Verify the transport ID is set
    /// assert_eq!(protocol.transport_id(), &transport_id);
    /// ```
    pub fn with_transport_id(options: ProtocolOptions, transport_id: TransportId) -> Self {
        Self {
            options,
            pending_requests: HashMap::new(),
            transport_id,
        }
    }

    /// Get protocol options.
    pub fn options(&self) -> &ProtocolOptions {
        &self.options
    }

    /// Get the transport ID for this protocol instance.
    pub fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    /// Register a pending request.
    pub fn register_request(&mut self, id: RequestId) -> oneshot::Receiver<JSONRPCResponse> {
        let (tx, rx) = oneshot::channel();
        let context = RequestContext {
            transport_id: self.transport_id.clone(),
            sender: Arc::new(Mutex::new(Some(tx))),
        };
        self.pending_requests.insert(id, context);
        rx
    }

    /// Complete a pending request.
    /// Only completes if the request was initiated by this transport.
    pub fn complete_request(&mut self, id: &RequestId, response: JSONRPCResponse) -> Result<()> {
        if let Some(context) = self.pending_requests.remove(id) {
            // Verify the response is for a request from this transport
            if context.transport_id == self.transport_id {
                // Use async runtime to send response
                let sender = context.sender;
                runtime::spawn(async move {
                    #[cfg(not(target_arch = "wasm32"))]
                    let tx_option = sender.lock().await.take();
                    #[cfg(target_arch = "wasm32")]
                    let tx_option = sender.lock().unwrap().take();
                    if let Some(tx) = tx_option {
                        let _ = tx.send(response);
                    }
                });
            } else {
                // Response is for a different transport, re-insert the request
                self.pending_requests.insert(id.clone(), context);
            }
        }
        Ok(())
    }

    /// Complete a pending request with transport verification.
    /// Only completes if the transport ID matches.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::shared::protocol::{Protocol, ProtocolOptions, TransportId};
    /// use pmcp::types::{RequestId, JSONRPCResponse};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport_id = TransportId::new();
    /// let mut protocol = Protocol::with_transport_id(
    ///     ProtocolOptions::default(),
    ///     transport_id.clone()
    /// );
    ///
    /// // Register a request
    /// let request_id = RequestId::from("req-123");
    /// let _rx = protocol.register_request(request_id.clone());
    ///
    /// // Complete with matching transport ID - succeeds
    /// let response = JSONRPCResponse::success(
    ///     request_id.clone(),
    ///     serde_json::json!("result")
    /// );
    /// let completed = protocol.complete_request_for_transport(
    ///     &request_id,
    ///     response.clone(),
    ///     &transport_id
    /// )?;
    /// assert!(completed);
    ///
    /// // Complete with wrong transport ID - fails
    /// let wrong_transport = TransportId::new();
    /// let completed = protocol.complete_request_for_transport(
    ///     &request_id,
    ///     response,
    ///     &wrong_transport
    /// )?;
    /// assert!(!completed);
    /// # Ok(())
    /// # }
    /// ```
    pub fn complete_request_for_transport(
        &mut self,
        id: &RequestId,
        response: JSONRPCResponse,
        transport_id: &TransportId,
    ) -> Result<bool> {
        if let Some(context) = self.pending_requests.get(id) {
            if &context.transport_id == transport_id {
                if let Some(context) = self.pending_requests.remove(id) {
                    let sender = context.sender;
                    runtime::spawn(async move {
                        #[cfg(not(target_arch = "wasm32"))]
                        let tx_option = sender.lock().await.take();
                        #[cfg(target_arch = "wasm32")]
                        let tx_option = sender.lock().unwrap().take();
                        if let Some(tx) = tx_option {
                            let _ = tx.send(response);
                        }
                    });
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Cancel a pending request.
    pub fn cancel_request(&mut self, id: &RequestId) {
        self.pending_requests.remove(id);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_options() {
        let options = ProtocolOptions {
            enforce_strict_capabilities: true,
            debounced_notification_methods: vec!["test".to_string()],
        };
        assert!(options.enforce_strict_capabilities);
        assert_eq!(options.debounced_notification_methods, vec!["test"]);

        let default_options = ProtocolOptions::default();
        assert!(!default_options.enforce_strict_capabilities);
        assert!(default_options.debounced_notification_methods.is_empty());
    }

    #[test]
    fn test_request_options() {
        let options = RequestOptions {
            timeout: Some(Duration::from_secs(30)),
            on_progress: None,
        };
        assert_eq!(options.timeout, Some(Duration::from_secs(30)));
        assert!(options.on_progress.is_none());

        // Test debug formatting
        let debug_str = format!("{:?}", options);
        assert!(debug_str.contains("timeout: Some"));
    }

    #[test]
    fn test_protocol_creation() {
        let options = ProtocolOptions::default();
        let protocol = Protocol::new(options);
        assert!(!protocol.options().enforce_strict_capabilities);
        assert_eq!(protocol.pending_requests.len(), 0);
    }

    #[tokio::test]
    async fn test_register_and_complete_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register a request
        let id = RequestId::Number(42);
        let mut rx = protocol.register_request(id.clone());
        assert_eq!(protocol.pending_requests.len(), 1);

        // Complete the request
        let response = JSONRPCResponse::success(id.clone(), serde_json::json!("success"));
        protocol.complete_request(&id, response.clone()).unwrap();

        // Give the async task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the receiver got the response
        let received = rx.try_recv().unwrap();
        assert_eq!(received.result(), Some(&serde_json::json!("success")));
    }

    #[test]
    fn test_cancel_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register multiple requests
        let id1 = RequestId::Number(1);
        let id2 = RequestId::String("req-2".to_string());
        let _rx1 = protocol.register_request(id1.clone());
        let _rx2 = protocol.register_request(id2.clone());
        assert_eq!(protocol.pending_requests.len(), 2);

        // Cancel one request
        protocol.cancel_request(&id1);
        assert_eq!(protocol.pending_requests.len(), 1);
        assert!(!protocol.pending_requests.contains_key(&id1));
        assert!(protocol.pending_requests.contains_key(&id2));

        // Cancel non-existent request (should not panic)
        protocol.cancel_request(&RequestId::Number(999));
        assert_eq!(protocol.pending_requests.len(), 1);
    }

    #[tokio::test]
    async fn test_complete_non_existent_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Try to complete a request that was never registered
        let id = RequestId::String("non-existent".to_string());
        let response = JSONRPCResponse::success(id.clone(), serde_json::json!("test"));

        // Should not panic
        let result = protocol.complete_request(&id, response);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_pending_requests() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register multiple requests
        let ids: Vec<_> = (0..5).map(RequestId::Number).collect();
        let _receivers: Vec<_> = ids
            .iter()
            .map(|id| protocol.register_request(id.clone()))
            .collect();
        assert_eq!(protocol.pending_requests.len(), 5);

        // Complete them in reverse order
        for (i, id) in ids.iter().enumerate().rev() {
            let response = JSONRPCResponse::success(id.clone(), serde_json::json!(i));
            protocol.complete_request(id, response).unwrap();
        }

        assert_eq!(protocol.pending_requests.len(), 0);
    }

    #[test]
    fn test_request_options_with_progress() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        let options = RequestOptions {
            timeout: Some(Duration::from_millis(100)),
            on_progress: Some(Box::new(move |current, total| {
                called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(current, 50);
                assert_eq!(total, Some(100));
            })),
        };

        // Call the progress callback
        if let Some(cb) = &options.on_progress {
            cb(50, Some(100));
        }

        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_protocol_with_enforced_capabilities() {
        let options = ProtocolOptions {
            enforce_strict_capabilities: true,
            debounced_notification_methods: vec![
                "notifications/progress".to_string(),
                "notifications/cancelled".to_string(),
            ],
        };

        let protocol = Protocol::new(options);
        assert!(protocol.options().enforce_strict_capabilities);
        assert_eq!(protocol.options().debounced_notification_methods.len(), 2);
    }
}
