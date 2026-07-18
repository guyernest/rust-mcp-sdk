//! Peer back-channel trait for server-to-client RPCs from inside request handlers.
//!
//! Implementations route outbound RPCs (`sampling/createMessage`, `roots/list`,
//! `notifications/progress`) to the client that originated the current request.
//! The trait is object-safe so [`crate::RequestHandlerExtra`] can hold
//! `Option<Arc<dyn PeerHandle>>`.
//!
//! This module is non-wasm only; on wasm32 targets the server dispatch path does
//! not carry a peer, and handlers should treat `extra.peer()` returning `None`
//! as the normal case.
//!
//! # Session isolation
//!
//! Peer handles are constructed fresh-per-request by the dispatch site. Each
//! `Server` instance owns its own dispatcher (and therefore its own set of
//! peer handles) bound to its own transport. Cross-session confusion requires
//! cross-process access, which is out of threat model.
//!
//! # Authorization
//!
//! Peer calls inherit the originating tool's authorization context. Tool-level
//! authz runs BEFORE the dispatch site wires `peer` — an unauthorized caller
//! never reaches the handler body and therefore never sees `extra.peer()`.

#![cfg(not(target_arch = "wasm32"))]

use crate::error::Result;
use crate::server::roots::ListRootsResult;
use crate::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools,
};
use crate::types::ProgressToken;
use async_trait::async_trait;

/// Server-to-client back-channel accessible from inside request handlers.
///
/// Implementations delegate outbound RPCs to the client session that
/// originated the current inbound request. The trait is object-safe so
/// [`crate::RequestHandlerExtra`] can hold `Option<Arc<dyn PeerHandle>>`.
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::PeerHandle;
/// use std::sync::Arc;
/// # async fn demo(peer: Arc<dyn PeerHandle>) -> pmcp::Result<()> {
/// let _roots = peer.list_roots().await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait PeerHandle: Send + Sync {
    /// Request the client to sample its LLM (`sampling/createMessage`).
    ///
    /// Delegates through the enclosing `Server`'s outbound request
    /// dispatcher. The response is deserialized into the typed
    /// [`CreateMessageResult`]; malformed responses surface as a protocol
    /// error (`INTERNAL_ERROR`).
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult>;

    /// Request the client to sample its LLM with tool support
    /// (`sampling/createMessage`, MCP 2025-11-25).
    ///
    /// Returns a [`CreateMessageResultWithTools`], whose `content` is an array
    /// that can carry `tool_use` / `tool_result` blocks — unlike the
    /// single-`Content` [`CreateMessageResult`] from [`PeerHandle::sample`].
    ///
    /// This is an ADDITIVE trait method with a default body that delegates to
    /// [`PeerHandle::sample`] and lifts the single result into the `WithTools`
    /// shape (via [`CreateMessageResultWithTools::from_single`]). Existing
    /// `PeerHandle` implementors therefore keep compiling unchanged; the
    /// dispatch-backed [`crate::server::peer_impl::DispatchPeerHandle`] overrides
    /// it to decode a real `CreateMessageResultWithTools` (with a legacy
    /// single-content fallback).
    async fn sample_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        let legacy = self.sample(params).await?;
        Ok(CreateMessageResultWithTools::from_single(legacy))
    }

    /// Request the client's root list (`roots/list`).
    ///
    /// Delegates through the enclosing `Server`'s outbound request
    /// dispatcher. The response is deserialized into the typed
    /// [`ListRootsResult`].
    async fn list_roots(&self) -> Result<ListRootsResult>;

    /// Send a progress notification (`notifications/progress`).
    ///
    /// Best-effort: returns `Ok(())` silently when no progress channel is
    /// configured — matches the existing
    /// [`crate::RequestHandlerExtra::report_progress`] no-op guard. This
    /// phase does NOT attempt to surface transport errors on the progress
    /// path; a follow-on phase may plumb `notification_tx` through the peer
    /// implementation for live progress reporting.
    async fn progress_notify(
        &self,
        token: ProgressToken,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;
}
