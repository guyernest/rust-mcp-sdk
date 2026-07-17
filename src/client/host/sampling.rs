//! Host-side sampling handler + approval seam.
//!
//! This module defines the trait a [`Client`](crate::Client) implements to
//! **answer** an inbound, spec-direction `sampling/createMessage` request — the
//! MCP *host* direction, where a connected server asks the client to run an LLM
//! completion on its behalf.
//!
//! # Not to be confused with the LLM-server pattern
//!
//! This is the **inverse** of [`pmcp::SamplingHandler`](crate::SamplingHandler)
//! (defined at `crate::server::SamplingHandler`). That server-side trait powers
//! the legacy "LLM-server pattern", where a *client* calls
//! [`Client::create_message`](crate::Client::create_message) to ask a *server*
//! whose `SamplingHandler` runs the LLM. Here the roles are reversed: the
//! **server** requests sampling and the **client host** answers it. See
//! [`crate::client::host`] for the full picture.

use crate::error::Result;
use crate::types::sampling::{CreateMessageParams, CreateMessageResult};
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;

/// Answers an inbound spec-direction `sampling/createMessage` request.
///
/// A [`Client`](crate::Client) registers an implementation via
/// [`ClientBuilder::on_sampling`](crate::ClientBuilder::on_sampling). When a
/// connected server calls `extra.peer().sample(..)` while one of the client's
/// own requests is in flight, the inbound request is routed to this handler and
/// the produced [`CreateMessageResult`] is returned to the server.
///
/// Distinct from [`pmcp::SamplingHandler`](crate::SamplingHandler), which is the
/// inverted LLM-server pattern (client asks server). The unambiguous public path
/// for *this* trait is `pmcp::client::host::HostSamplingHandler`.
#[async_trait]
pub trait HostSamplingHandler: Send + Sync {
    /// Produce a completion for the given inbound sampling request.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion cannot be produced. The client maps a
    /// handler error to a sanitized JSON-RPC `-32603` response (the raw error is
    /// logged locally and never forwarded to the remote server).
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResult>;
}

/// Outcome of a human-in-the-loop approval callback.
///
/// The approval seam is a host-side access-control hook on sampling. This phase
/// defines the type; its INVOCATION (gating an LLM call before/after the
/// handler runs) lands in the follow-on plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Allow the sampling call to proceed / the completion to be returned.
    Allow,
    /// Deny the sampling call, carrying a human-readable reason.
    Deny(String),
}

/// Mandatory pre-handler approval gate for sampling.
///
/// Invoked with owned [`CreateMessageParams`] (owned so the value moves cleanly
/// into a `'static` future) BEFORE the [`HostSamplingHandler`] runs. Returning
/// [`ApprovalDecision::Deny`] prevents the LLM call.
///
/// The type is defined here; wiring the invocation into dispatch is the
/// follow-on plan's responsibility.
pub type PreflightApproval =
    Arc<dyn Fn(CreateMessageParams) -> BoxFuture<'static, ApprovalDecision> + Send + Sync>;

/// Optional post-handler review of a produced completion.
///
/// Invoked with the owned request params and the produced
/// [`CreateMessageResult`] AFTER the [`HostSamplingHandler`] runs, letting an
/// approver inspect the actual completion before it is returned. Returning
/// [`ApprovalDecision::Deny`] suppresses the completion.
///
/// The type is defined here; wiring the invocation into dispatch is the
/// follow-on plan's responsibility.
pub type SamplingResultReview = Arc<
    dyn Fn(CreateMessageParams, CreateMessageResult) -> BoxFuture<'static, ApprovalDecision>
        + Send
        + Sync,
>;
