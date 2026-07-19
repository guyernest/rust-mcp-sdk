//! Tasks-aware [`ToolInvoker`](crate::seams::ToolInvoker) + connector-client
//! factory (plan 108-05).
//!
//! - [`ClientToolInvoker`] drives task-augmented tool results to terminal via
//!   `wait_for_related_task` under a hard poll cap, and dispatches batches with
//!   bounded parallelism (AGNT-08).
//! - [`ConnectorClient`] / [`ConnectorClientFactory`] are the object-safe seam
//!   over heterogeneous MCP transports; [`UrlConnectorClientFactory`] (behind
//!   the `url-connector` feature) is the shipped URL/HTTP impl.

mod client;
mod factory;

pub use client::ClientToolInvoker;
pub use factory::{ConnectorClient, ConnectorClientFactory, InvokerError};

#[cfg(feature = "url-connector")]
pub use factory::UrlConnectorClientFactory;
