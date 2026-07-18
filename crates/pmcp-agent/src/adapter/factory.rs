//! [`CompletionSourceFactory`] — construct the completion source PER request.
//!
//! A hosted agent's completion source is not fixed for the life of the server:
//! the zero-dependency [`SamplingSource`] must be rebuilt on every `tools/call`
//! from that request's [`RequestHandlerExtra::peer`](pmcp::RequestHandlerExtra),
//! because the peer is request-scoped (it correlates server→client sampling for
//! THIS call). An HTTP source, by contrast, is a fixed, shareable client.
//!
//! This module resolves that mismatch with a small factory seam the
//! [`AgentServer`](super::server::AgentServer) tool handler calls once per
//! invocation:
//!
//! - [`SamplingSourceFactory`] builds a request-scoped [`SamplingSource`] from
//!   `extra.peer()` (AGNT-04, hosted-sampled path).
//! - [`FixedSourceFactory`] returns a preconstructed source as-is (e.g. an HTTP
//!   `OpenAiCompatSource`/`AnthropicSource`, or a test mock).
//!
//! The whole module is NATIVE-ONLY (`cfg(not(target_arch = "wasm32"))`) — it is
//! reachable only from the native-only adapter, and `SamplingSource` itself
//! rides the native-only `pmcp::PeerHandle`.

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};
use pmcp::RequestHandlerExtra;

use crate::seams::{CompletionError, CompletionSource};
use crate::sources::SamplingSource;

/// Builds the [`CompletionSource`] for one `tools/call` invocation.
///
/// The adapter calls [`create`](CompletionSourceFactory::create) once per
/// request, passing that request's [`RequestHandlerExtra`], and drives a fresh
/// [`AgentEngine`](crate::iteration::AgentEngine) run over the returned source.
pub trait CompletionSourceFactory: Send + Sync {
    /// Construct the completion source for this request.
    ///
    /// Implementations that need the request-scoped peer read it from `extra`;
    /// fixed-source implementations ignore it.
    fn create(&self, extra: &RequestHandlerExtra) -> Arc<dyn CompletionSource>;
}

/// Builds a request-scoped [`SamplingSource`] from `extra.peer()` (AGNT-04).
///
/// The peer speaks spec sampling back to the hosting client, so a fresh
/// `SamplingSource` is minted for every call. When no peer is attached (the
/// server was not run through a peer-wiring loop such as
/// [`Server::run`](pmcp::Server::run)), the factory returns a source that fails
/// every completion with a transient error rather than panicking — the run then
/// terminates cleanly as a failed outcome.
#[derive(Debug, Default, Clone)]
pub struct SamplingSourceFactory;

impl SamplingSourceFactory {
    /// Create a sampling-source factory.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CompletionSourceFactory for SamplingSourceFactory {
    fn create(&self, extra: &RequestHandlerExtra) -> Arc<dyn CompletionSource> {
        match extra.peer() {
            Some(peer) => Arc::new(SamplingSource::new(peer.clone())),
            None => Arc::new(UnavailablePeerSource),
        }
    }
}

/// Returns a preconstructed [`CompletionSource`] as-is on every request.
///
/// Used for a shared HTTP source (`OpenAiCompatSource`/`AnthropicSource`) or a
/// test mock — anything that is fixed for the life of the server rather than
/// request-scoped.
#[derive(Clone)]
pub struct FixedSourceFactory {
    source: Arc<dyn CompletionSource>,
}

impl FixedSourceFactory {
    /// Wrap a preconstructed, shareable completion source.
    #[must_use]
    pub fn new(source: Arc<dyn CompletionSource>) -> Self {
        Self { source }
    }
}

impl CompletionSourceFactory for FixedSourceFactory {
    fn create(&self, _extra: &RequestHandlerExtra) -> Arc<dyn CompletionSource> {
        self.source.clone()
    }
}

/// Fallback source returned by [`SamplingSourceFactory`] when no peer is
/// attached — every completion fails transiently instead of panicking.
struct UnavailablePeerSource;

#[async_trait]
impl CompletionSource for UnavailablePeerSource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Err(CompletionError::Transport(
            "no server peer attached: run the AgentServer through a peer-wiring \
             loop (Server::run) to enable sampling"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionSource, CompletionSourceFactory, FixedSourceFactory, SamplingSourceFactory,
    };
    use crate::seams::CompletionError;
    use async_trait::async_trait;
    use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};
    use pmcp::types::Role;
    use std::sync::Arc;

    struct OkSource;
    #[async_trait]
    impl CompletionSource for OkSource {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools, CompletionError> {
            Ok(CreateMessageResultWithTools::new(
                "fixed-model",
                Role::Assistant,
                vec![],
            ))
        }
    }

    #[test]
    fn fixed_factory_returns_the_same_source() {
        let factory = FixedSourceFactory::new(Arc::new(OkSource) as Arc<dyn CompletionSource>);
        // A bare extra with no peer — the fixed factory ignores it.
        let extra = pmcp::RequestHandlerExtra::default();
        let source = factory.create(&extra);
        // Two calls yield the same underlying Arc.
        let again = factory.create(&extra);
        assert!(Arc::ptr_eq(&source, &again));
    }

    #[tokio::test]
    async fn sampling_factory_without_peer_yields_a_failing_source() {
        let factory = SamplingSourceFactory::new();
        let extra = pmcp::RequestHandlerExtra::default();
        let source = factory.create(&extra);
        let err = source
            .create_message(CreateMessageParams::new(vec![]))
            .await
            .expect_err("no peer -> error, not panic");
        assert!(matches!(err, CompletionError::Transport(_)));
    }
}
