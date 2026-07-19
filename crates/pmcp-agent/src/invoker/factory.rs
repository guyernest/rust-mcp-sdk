//! Connector-client factory seam + the object-safe connector client it produces.
//!
//! A [`pmcp::Client<T>`] is transport-generic, so it is NOT object-safe: an
//! agent that talks to several heterogeneous endpoints cannot hold a
//! `Vec<Client<T>>`. The [`ConnectorClient`] trait erases the transport behind
//! the minimal surface [`ClientToolInvoker`](super::client::ClientToolInvoker)
//! needs (`call_tool` + `wait_for_related_task`), and [`ConnectorClientFactory`]
//! mints one per resolved endpoint.
//!
//! Both traits are UNCONDITIONAL (they compile on `wasm32`) so the seam holds
//! everywhere. The one concrete impl this plan ships,
//! [`UrlConnectorClientFactory`], wraps a `pmcp::Client<StreamableHttpTransport>`
//! and is therefore gated behind the crate's `url-connector` feature (that
//! transport is native-only and pulls a real HTTP client). Command/stdio
//! transports are a narrow, justified follow-up: URL endpoints already cover the
//! AGNT-05/AGNT-09 targets and a command transport adds no new loop behavior —
//! only a second `ConnectorClient` impl behind the same seam.

use async_trait::async_trait;
use std::sync::Arc;

use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, ToolInfo};
use pmcp::WaitForTaskOptions;

/// An error establishing or using a connector client.
///
/// Distinct from a per-call tool error (which is carried as data in a
/// [`ToolCallResult`](crate::seams::ToolCallResult)): this is a
/// transport/configuration failure of the connection itself. Messages never
/// contain secret material.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InvokerError {
    /// Transport / connection / protocol failure — transient.
    #[error("connector transport error: {0}")]
    Transport(String),
    /// The endpoint scheme is not supported by this factory (e.g. a `file://`
    /// URL handed to the URL/HTTP factory).
    #[error("unsupported endpoint scheme: {0}")]
    UnsupportedScheme(String),
    /// The endpoint or client could not be configured.
    #[error("connector configuration error: {0}")]
    Config(String),
}

/// The minimal, object-safe MCP client surface the tool invoker needs.
///
/// Implementors erase a concrete `pmcp::Client<T>` so the invoker can hold an
/// `Arc<dyn ConnectorClient>` over heterogeneous transports.
#[async_trait]
pub trait ConnectorClient: Send + Sync {
    /// Call a tool, returning the raw [`CallToolResult`] (which may carry a
    /// related-task envelope under `_meta`).
    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, InvokerError>;

    /// Drive a related task to terminal and return its FINAL result.
    ///
    /// The caller supplies a hard `max_poll_duration_secs` cap in `opts`, so an
    /// implementation that delegates to
    /// [`pmcp::Client::wait_for_related_task`] can never poll forever.
    async fn wait_for_related_task(
        &self,
        meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult, InvokerError>;

    /// Discover the tools this connector advertises (`tools/list`), so the loop
    /// can tell the model what it may call.
    ///
    /// DEFAULT: `Ok(empty)` — a connector that does not support discovery
    /// advertises nothing, which is backward-compatible (existing impls keep
    /// compiling) and makes the loop simply send no `tools` to the model.
    async fn list_tools(&self) -> Result<Vec<ToolInfo>, InvokerError> {
        Ok(Vec::new())
    }
}

/// Produces a [`ConnectorClient`] for a resolved endpoint (URL or, in a
/// follow-up, command transport).
#[async_trait]
pub trait ConnectorClientFactory: Send + Sync {
    /// Establish a connector client for `endpoint`.
    async fn client_for(&self, endpoint: &str) -> Result<Arc<dyn ConnectorClient>, InvokerError>;
}

// The concrete URL/HTTP connector. Native-only + behind `url-connector`
// (BLOCKER-2): `StreamableHttpTransport` is `#[cfg(not(target_arch = "wasm32"))]`
// and pulls a real HTTP client, so the default + wasm32 build never sees it.
#[cfg(feature = "url-connector")]
pub use url_impl::UrlConnectorClientFactory;

#[cfg(feature = "url-connector")]
mod url_impl {
    use super::{ConnectorClient, ConnectorClientFactory, InvokerError};
    use async_trait::async_trait;
    use std::sync::Arc;

    use pmcp::shared::streamable_http::{
        StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
    };
    use pmcp::types::tasks::TaskMetadata;
    use pmcp::types::{CallToolResult, ToolInfo};
    use pmcp::{Client, ClientCapabilities, WaitForTaskOptions};

    /// A [`ConnectorClientFactory`] that connects `http(s)://` endpoints over
    /// `StreamableHttpTransport`.
    #[derive(Debug, Default, Clone)]
    pub struct UrlConnectorClientFactory;

    impl UrlConnectorClientFactory {
        /// Create a URL connector factory.
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl ConnectorClientFactory for UrlConnectorClientFactory {
        async fn client_for(
            &self,
            endpoint: &str,
        ) -> Result<Arc<dyn ConnectorClient>, InvokerError> {
            let url = url::Url::parse(endpoint)
                .map_err(|e| InvokerError::Config(format!("invalid endpoint URL: {e}")))?;
            // T-108-05-05: only http(s) endpoints are dispatched (mirrors the
            // 108-04 scheme policy) — never a `file://`/`data:`/etc. scheme.
            match url.scheme() {
                "http" | "https" => {},
                other => return Err(InvokerError::UnsupportedScheme(other.to_string())),
            }
            let config = StreamableHttpTransportConfigBuilder::new(url).build();
            let transport = StreamableHttpTransport::new(config);
            let mut client = Client::new(transport);
            client
                .initialize(ClientCapabilities::default())
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))?;
            Ok(Arc::new(UrlConnectorClient { client }))
        }
    }

    /// A [`ConnectorClient`] backed by an initialized `Client<StreamableHttpTransport>`.
    struct UrlConnectorClient {
        client: Client<StreamableHttpTransport>,
    }

    #[async_trait]
    impl ConnectorClient for UrlConnectorClient {
        async fn call_tool(
            &self,
            name: &str,
            arguments: serde_json::Value,
        ) -> Result<CallToolResult, InvokerError> {
            self.client
                .call_tool(name.to_string(), arguments)
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }

        async fn wait_for_related_task(
            &self,
            meta: &TaskMetadata,
            opts: WaitForTaskOptions,
        ) -> Result<CallToolResult, InvokerError> {
            self.client
                .wait_for_related_task(meta, opts)
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }

        async fn list_tools(&self) -> Result<Vec<ToolInfo>, InvokerError> {
            self.client
                .list_tools(None)
                .await
                .map(|result| result.tools)
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }
    }
}
