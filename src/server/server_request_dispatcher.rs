//! Outbound server-to-client request dispatcher with response correlation.
//!
//! Plumbing foundation for the peer back-channel. Adapts the
//! `ElicitationManager` pattern (`mpsc::Sender<ServerRequest>` +
//! pending-oneshot `HashMap` + `tokio::time::timeout`) into a generalized
//! dispatcher that can fulfill any server-to-client RPC — CreateMessage,
//! ListRoots, or any future addition. Keyed by correlation id so a single
//! dispatcher multiplexes many in-flight requests.
//!
//! This module is non-wasm only; wasm targets do not run the legacy
//! `Server` transport loop this integrates with.
//!
//! # Scope
//!
//! - `dispatch(request)` enqueues onto an outbound mpsc channel and awaits
//!   the correlated response via a pending-oneshot map.
//! - `handle_response(correlation_id, value)` fulfills the matching
//!   oneshot. Unknown correlation ids return `INVALID_REQUEST` without
//!   crashing the server loop.
//! - `spawn_server_request_drain(transport, outbound_rx)` wraps each
//!   outbound pair into a `TransportMessage::Request` and forwards to the
//!   transport — consumed by `Server::run`.
//!
//! # Threat model
//!
//! - Correlation ids are generated server-side from an `AtomicU64`
//!   counter. An attacker cannot predict live counter state from inbound
//!   responses alone.
//! - Timeout branches always remove the pending entry to prevent map leak
//!   (`test_dispatcher_timeout_cleans_pending`).
//! - The custom `Debug` impl never prints pending correlation ids.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::error::{Error, ErrorCode, Result};
use crate::types::ServerRequest;

/// Default timeout for a server-to-client RPC (60 seconds).
///
/// Shorter than `ElicitationManager`'s 5-minute default because
/// sampling/`list_roots` tend to be short and synchronous from the
/// client's perspective.
pub const DEFAULT_DISPATCH_TIMEOUT: Duration = Duration::from_secs(60);

static DISPATCH_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Dispatches server-initiated requests to the client and correlates
/// their responses back to the awaiting caller.
///
/// Held inside `Server` (and optionally `ServerCore`) as
/// `Arc<ServerRequestDispatcher>`. The enclosing `Server`'s `run` loop
/// is responsible for:
///   - draining `outbound_rx` and serializing each
///     `(correlation_id, ServerRequest)` onto the transport as a
///     `TransportMessage::Request { id, request }`;
///   - routing `TransportMessage::Response` back through
///     `handle_response`.
pub struct ServerRequestDispatcher {
    /// Outbound side. Kept on the dispatcher for `dispatch` to push onto.
    outbound_tx: mpsc::Sender<(String, ServerRequest)>,
    /// Pending requests awaiting response, keyed by correlation id.
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<Value>>>>,
    /// Per-request timeout.
    timeout_duration: Duration,
}

impl ServerRequestDispatcher {
    /// Construct with a pre-built outbound channel.
    ///
    /// The caller (`Server::run`) owns the matching receiver and is
    /// responsible for the drain-to-transport task.
    pub fn new_with_channel(outbound_tx: mpsc::Sender<(String, ServerRequest)>) -> Self {
        Self {
            outbound_tx,
            pending: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: DEFAULT_DISPATCH_TIMEOUT,
        }
    }

    /// Override the default timeout. Builder form.
    #[must_use]
    pub fn with_timeout(mut self, timeout_duration: Duration) -> Self {
        self.timeout_duration = timeout_duration;
        self
    }

    /// Generate a fresh correlation id. Monotonic + server-local.
    fn next_correlation_id() -> String {
        let id = DISPATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("dispatch-{id}")
    }

    /// Dispatch a server-to-client request and await its correlated
    /// response.
    ///
    /// Returns the raw JSON `Value` response; callers deserialize to the
    /// appropriate result type (`CreateMessageResult`, `ListRootsResult`,
    /// etc.).
    pub async fn dispatch(&self, request: ServerRequest) -> Result<Value> {
        if self.outbound_tx.is_closed() {
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "ServerRequestDispatcher outbound channel closed",
            ));
        }

        let (tx, rx) = oneshot::channel::<Value>();
        let correlation_id = Self::next_correlation_id();
        self.pending
            .write()
            .await
            .insert(correlation_id.clone(), tx);

        if let Err(e) = self
            .outbound_tx
            .send((correlation_id.clone(), request))
            .await
        {
            self.pending.write().await.remove(&correlation_id);
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to enqueue server request: {e}"),
            ));
        }

        debug!("Dispatched server request: {}", correlation_id);

        match timeout(self.timeout_duration, rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                // oneshot sender dropped without sending.
                self.pending.write().await.remove(&correlation_id);
                Err(Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    "Dispatch oneshot channel closed",
                ))
            },
            Err(_) => {
                // Timeout — remove pending entry to prevent leak.
                self.pending.write().await.remove(&correlation_id);
                Err(Error::protocol(
                    ErrorCode::REQUEST_TIMEOUT,
                    format!("Server request {correlation_id} timed out"),
                ))
            },
        }
    }

    /// Route a client response back to the awaiting `dispatch` caller.
    ///
    /// Called by `Server::run` when `TransportMessage::Response(...)`
    /// arrives. The `correlation_id` parameter must match the one the
    /// dispatcher assigned when sending the matching request. Unknown
    /// ids return `INVALID_REQUEST` without crashing the server loop.
    pub async fn handle_response(&self, correlation_id: &str, response: Value) -> Result<()> {
        let mut pending = self.pending.write().await;
        if let Some(tx) = pending.remove(correlation_id) {
            if tx.send(response).is_err() {
                warn!("Dispatch response receiver dropped: {}", correlation_id);
            }
            Ok(())
        } else {
            warn!(
                "Received response for unknown correlation: {}",
                correlation_id
            );
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Unknown correlation id: {correlation_id}"),
            ))
        }
    }

    /// Number of pending dispatches. Used by integration tests; no in-tree
    /// library call sites yet, hence the `dead_code` allow.
    #[allow(dead_code)]
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

impl std::fmt::Debug for ServerRequestDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerRequestDispatcher")
            .field("timeout_duration", &self.timeout_duration)
            .field("outbound_tx_closed", &self.outbound_tx.is_closed())
            .finish()
    }
}

/// Drain outbound server-to-client requests and serialize them onto the
/// transport as JSON-RPC `Request` messages.
///
/// Spawned by `Server::run` once per server lifetime. Exits cleanly when
/// the outbound channel is closed (dispatcher dropped) or on transport
/// send failure (logged).
pub fn spawn_server_request_drain<T>(
    transport: Arc<crate::runtime::RwLock<T>>,
    mut outbound_rx: mpsc::Receiver<(String, ServerRequest)>,
) where
    T: crate::shared::Transport + 'static,
{
    tokio::spawn(async move {
        while let Some((correlation_id, server_request)) = outbound_rx.recv().await {
            let request = crate::types::Request::Server(Box::new(server_request));
            let id = crate::types::RequestId::from(correlation_id.clone());

            let mut t = transport.write().await;
            if let Err(e) = t
                .send(crate::shared::TransportMessage::Request { id, request })
                .await
            {
                warn!(
                    "Failed to dispatch server request {}: {}",
                    correlation_id, e
                );
                // Continue draining — transient transport failures
                // shouldn't drop the entire drain loop.
            }
        }
        debug!("Server-request drain task exited");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatcher_enqueues_on_outbound_channel() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher =
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(100));
        let dispatch_fut =
            tokio::spawn(async move { dispatcher.dispatch(ServerRequest::ListRoots).await });

        // Drain the outbound channel — same role as
        // Server::spawn_server_request_drain.
        let (correlation_id, req) = tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("recv deadline")
            .expect("channel closed unexpectedly");
        assert!(
            !correlation_id.is_empty(),
            "correlation id must be non-empty"
        );
        assert!(matches!(req, ServerRequest::ListRoots));
        // Let dispatch time out (we never fulfill); drain the spawned future.
        let _ = dispatch_fut.await;
    }

    #[tokio::test]
    async fn test_dispatcher_fulfills_on_handle_response() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
        );

        let dispatch_fut = {
            let d = dispatcher.clone();
            tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await })
        };

        let (correlation_id, _req) = rx.recv().await.expect("outbound must receive");
        let response = serde_json::json!({"roots": []});
        dispatcher
            .handle_response(&correlation_id, response.clone())
            .await
            .expect("handle_response must succeed");

        let result = dispatch_fut.await.unwrap().expect("dispatch must succeed");
        assert_eq!(result, response);
        assert_eq!(dispatcher.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_dispatcher_timeout_cleans_pending() {
        let (tx, mut _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher =
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40));
        let result = dispatcher.dispatch(ServerRequest::ListRoots).await;
        assert!(result.is_err(), "dispatch must timeout");
        assert_eq!(
            dispatcher.pending_count().await,
            0,
            "timeout must clean pending"
        );
    }

    #[tokio::test]
    async fn test_dispatcher_handle_response_unknown_id_returns_err() {
        let (tx, _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = ServerRequestDispatcher::new_with_channel(tx);
        let result = dispatcher
            .handle_response("does-not-exist", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dispatcher_debug_does_not_leak_correlation_ids() {
        let (tx, _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(5)),
        );
        // Kick off a dispatch to populate a pending entry.
        let d = dispatcher.clone();
        let _fut = tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await });
        // Give the task a moment to insert into `pending`.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let debug_str = format!("{:?}", dispatcher);
        assert!(
            !debug_str.contains("dispatch-"),
            "debug must not leak correlation id: {debug_str}"
        );
        assert!(debug_str.contains("ServerRequestDispatcher"));
    }
}
