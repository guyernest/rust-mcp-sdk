//! Host-side roots provider.
//!
//! A [`Client`](crate::Client) answers an inbound `roots/list` request from a
//! registered provider closure. A closure (rather than a full trait) is
//! sufficient because `roots/list` carries no params — the provider simply
//! yields the current [`ListRootsResult`].

use crate::error::Result;
use crate::types::roots::ListRootsResult;
use futures::future::BoxFuture;
use std::sync::Arc;

/// Provides the client's roots in response to an inbound `roots/list` request.
///
/// Registered via [`ClientBuilder::on_roots`](crate::ClientBuilder::on_roots).
///
/// The future returns a [`Result`] so a provider failure maps to a sanitized
/// JSON-RPC `-32603` response, mirroring the sampling and elicitation handler
/// error paths.
pub type RootsProvider = Arc<dyn Fn() -> BoxFuture<'static, Result<ListRootsResult>> + Send + Sync>;
