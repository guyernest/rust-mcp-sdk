//! Host-side elicitation handler.
//!
//! Defines the trait a [`Client`](crate::Client) implements to **answer** an
//! inbound `elicitation/create` request — a server asking the client/host to
//! collect user input.

use crate::error::Result;
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use async_trait::async_trait;

/// Answers an inbound `elicitation/create` request.
///
/// A [`Client`](crate::Client) registers an implementation via
/// [`ClientBuilder::on_elicitation`](crate::ClientBuilder::on_elicitation).
/// Unlike sampling, `elicitation/create` has no client-request alias, so it
/// always parses as a server request — no parse-ambiguity handling is needed.
#[async_trait]
pub trait HostElicitationHandler: Send + Sync {
    /// Collect user input for the given inbound elicitation request.
    ///
    /// # Errors
    ///
    /// Returns an error if input cannot be collected. The client maps a handler
    /// error to a sanitized JSON-RPC `-32603` response (the raw error is logged
    /// locally and never forwarded to the remote server).
    async fn handle_elicitation(&self, params: ElicitRequestParams) -> Result<ElicitResult>;
}
