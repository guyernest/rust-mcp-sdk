//! Concrete [`PeerHandle`] implementation that delegates to the
//! [`ServerRequestDispatcher`] (Phase 70 / PARITY-HANDLER-01).
//!
//! `DispatchPeerHandle` does NOT own a channel. It holds an
//! `Arc<ServerRequestDispatcher>` and delegates every outbound RPC to
//! `dispatcher.dispatch(...)`. The dispatcher owns the correlation layer
//! (pending oneshot map keyed by correlation id) and the drain-to-transport
//! task. This avoids the anti-pattern of ad-hoc per-site channel
//! construction: every peer handle shares the single correlation authority.
//!
//! Deserialization: the dispatcher returns `serde_json::Value`; the
//! `DispatchPeerHandle` parses into the typed result and surfaces malformed
//! responses as a protocol `INTERNAL_ERROR`.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{Error, ErrorCode, Result};
use crate::server::roots::ListRootsResult;
use crate::server::server_request_dispatcher::ServerRequestDispatcher;
use crate::shared::peer::PeerHandle;
use crate::types::sampling::{CreateMessageParams, CreateMessageResult};
use crate::types::{ProgressToken, ServerRequest};

/// [`PeerHandle`] that delegates outbound RPCs to a shared
/// [`ServerRequestDispatcher`].
///
/// Constructed fresh-per-request at each `ServerCore` dispatch site when
/// the enclosing `ServerCore` was built with
/// [`crate::server::core::ServerCore::with_server_request_dispatcher`].
/// The construction is near-zero-cost — the struct is a single `Arc`
/// clone — so per-request allocation is not a concern.
#[derive(Debug)]
pub struct DispatchPeerHandle {
    dispatcher: Arc<ServerRequestDispatcher>,
}

impl DispatchPeerHandle {
    /// Build a peer handle around a shared dispatcher.
    ///
    /// Pub (not `pub(crate)`) so the `#[doc(hidden)] __test_support`
    /// re-export in `src/lib.rs` can link from integration tests; the
    /// enclosing `peer_impl` module is `pub(crate)`, so this stays
    /// internal from a doc/discoverability standpoint.
    pub fn new(dispatcher: Arc<ServerRequestDispatcher>) -> Self {
        Self { dispatcher }
    }
}

#[async_trait]
impl PeerHandle for DispatchPeerHandle {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        let value = self
            .dispatcher
            .dispatch(ServerRequest::CreateMessage(Box::new(params)))
            .await?;
        serde_json::from_value::<CreateMessageResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid sample response: {e}"),
            )
        })
    }

    async fn list_roots(&self) -> Result<ListRootsResult> {
        let value = self.dispatcher.dispatch(ServerRequest::ListRoots).await?;
        serde_json::from_value::<ListRootsResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid list_roots response: {e}"),
            )
        })
    }

    async fn progress_notify(
        &self,
        _token: ProgressToken,
        _progress: f64,
        _total: Option<f64>,
        _message: Option<String>,
    ) -> Result<()> {
        // Progress is a notification (one-way, no response) not a
        // request/response. The existing `Server::notification_tx:
        // Sender<Notification>` channel is the right vehicle, but
        // DispatchPeerHandle doesn't hold a clone. For this phase we
        // preserve the existing `RequestHandlerExtra::report_progress`
        // no-op behavior: return Ok(()) silently. Follow-on work can plumb
        // notification_tx through DispatchPeerHandle for live progress.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn build_dispatcher_with_short_timeout() -> (
        Arc<ServerRequestDispatcher>,
        mpsc::Receiver<(String, ServerRequest)>,
    ) {
        let (tx, rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40)),
        );
        (dispatcher, rx)
    }

    #[tokio::test]
    async fn test_peer_handle_trait_shape() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer: Arc<dyn PeerHandle> = Arc::new(DispatchPeerHandle::new(dispatcher));
        // Trait-shape smoke: casts to Arc<dyn PeerHandle>. The Arc itself
        // can be cloned and stored — no ?Sized errors.
        let _clone = peer.clone();
    }

    #[tokio::test]
    async fn test_peer_progress_notify_always_ok() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer = DispatchPeerHandle::new(dispatcher);
        let result = peer
            .progress_notify(
                ProgressToken::String("tok-1".to_string()),
                0.5,
                Some(1.0),
                None,
            )
            .await;
        assert!(result.is_ok(), "progress_notify is a no-op for this phase");
    }

    #[tokio::test]
    async fn test_peer_sample_propagates_dispatcher_timeout() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer = DispatchPeerHandle::new(dispatcher);
        // Use REAL constructor — CreateMessageParams has no Default impl.
        let params = CreateMessageParams::new(Vec::new());
        let start = std::time::Instant::now();
        let result = peer.sample(params).await;
        let elapsed = start.elapsed();
        assert!(
            result.is_err(),
            "sample must return Err when dispatcher times out"
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "timeout must fire within 500ms (was {:?})",
            elapsed
        );
    }
}
