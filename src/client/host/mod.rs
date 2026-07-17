//! Client host surface: answering server -> client requests.
//!
//! Historically a [`Client`](crate::Client) rejected any inbound request with an
//! "Unexpected message type" error. This module adds the **host** direction of
//! the MCP spec: a client can register handlers that answer inbound
//! `sampling/createMessage`, `elicitation/create`, and `roots/list` requests
//! while one of the client's own requests is in flight (the nested
//! sampling-during-`tools/call` flow).
//!
//! # Direction disambiguation (HOST-06)
//!
//! - [`HostSamplingHandler`] answers an INBOUND sampling request (server asks
//!   client — the spec host direction).
//! - [`pmcp::SamplingHandler`](crate::SamplingHandler) is the INVERSE
//!   "LLM-server pattern": a client calls
//!   [`Client::create_message`](crate::Client::create_message) to ask a server
//!   whose `SamplingHandler` runs the LLM. It is unchanged and not deprecated.
//!
//! Placing these traits under `pmcp::client::host::*` guarantees the public
//! paths are unmistakable even where names are similar.
//!
//! # Idle-host limitation
//!
//! The pmcp `Client` has no background receive loop; inbound requests are only
//! read while a client-initiated request awaits its response. Answering
//! sampling/elicitation/roots therefore works **during** an in-flight client
//! request (the agent/tool use case) but not while the client sits idle. This
//! is intentional for the current phase.

pub mod elicitation;
pub mod roots;
pub mod sampling;

use crate::types::protocol::{ClientRequest, Request, ServerRequest};
use std::sync::Arc;

pub use elicitation::HostElicitationHandler;
pub use roots::RootsProvider;
pub use sampling::{
    ApprovalDecision, HostSamplingHandler, PreflightApproval, SamplingResultReview,
};

/// Immutable set of host handlers a [`Client`](crate::Client) answers with.
///
/// Built once via [`ClientBuilder`](crate::ClientBuilder) and never mutated
/// afterwards, so dispatch stays lock-free (plain `Option<Arc<..>>` fields, no
/// interior mutability).
#[derive(Clone, Default)]
pub struct ClientHostRegistry {
    pub(crate) sampling: Option<Arc<dyn HostSamplingHandler>>,
    pub(crate) elicitation: Option<Arc<dyn HostElicitationHandler>>,
    pub(crate) roots: Option<RootsProvider>,
    pub(crate) approval: Option<PreflightApproval>,
    pub(crate) result_review: Option<SamplingResultReview>,
}

impl std::fmt::Debug for ClientHostRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientHostRegistry")
            .field("has_sampling", &self.sampling.is_some())
            .field("has_elicitation", &self.elicitation.is_some())
            .field("has_roots", &self.roots.is_some())
            .field("has_approval", &self.approval.is_some())
            .field("has_result_review", &self.result_review.is_some())
            .finish()
    }
}

/// Classification of an inbound request at a client into a host-handler kind.
///
/// Pure, synchronous, and side-effect free so it can be property/fuzz tested
/// independently of the async dispatch path.
// Why: consumed by `Client::dispatch_host_request` (wired in the next task of
// this plan); the annotation is removed there.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostRequestKind {
    /// `sampling/createMessage` (either parse variant — see below).
    Sampling,
    /// `elicitation/create`.
    Elicitation,
    /// `roots/list`.
    Roots,
    /// A known-but-unroutable request kind.
    Unhandled,
}

/// Classify an inbound [`Request`] into the host-handler kind that should answer
/// it.
///
/// Handles the parse ambiguity where an inbound `sampling/createMessage` is
/// delivered as `Request::Client(ClientRequest::CreateMessage)` (because
/// `parse_request` tries the client grammar first) as well as the
/// `Request::Server(ServerRequest::CreateMessage)` shape. Both map to
/// [`HostRequestKind::Sampling`].
// Why: consumed by `Client::dispatch_host_request` (wired in the next task of
// this plan); the annotation is removed there.
#[allow(dead_code)]
pub(crate) fn classify_host_request(request: &Request) -> HostRequestKind {
    match request {
        Request::Client(client) => match client.as_ref() {
            ClientRequest::CreateMessage(_) => HostRequestKind::Sampling,
            _ => HostRequestKind::Unhandled,
        },
        Request::Server(server) => match server.as_ref() {
            ServerRequest::CreateMessage(_) => HostRequestKind::Sampling,
            ServerRequest::ElicitationCreate(_) => HostRequestKind::Elicitation,
            ServerRequest::ListRoots => HostRequestKind::Roots,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::sampling::CreateMessageParams;

    #[test]
    fn registry_default_is_all_none() {
        let reg = ClientHostRegistry::default();
        assert!(reg.sampling.is_none());
        assert!(reg.elicitation.is_none());
        assert!(reg.roots.is_none());
        assert!(reg.approval.is_none());
        assert!(reg.result_review.is_none());
    }

    #[test]
    fn registry_debug_reports_has_flags() {
        let reg = ClientHostRegistry::default();
        let dbg = format!("{reg:?}");
        assert!(dbg.contains("has_sampling: false"));
        assert!(dbg.contains("has_elicitation: false"));
        assert!(dbg.contains("has_roots: false"));
    }

    #[test]
    fn classify_sampling_server_variant() {
        let req = Request::Server(Box::new(ServerRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))));
        assert_eq!(classify_host_request(&req), HostRequestKind::Sampling);
    }

    #[test]
    fn classify_sampling_client_alias_variant() {
        // Inbound sampling parses as the CLIENT variant (parse ambiguity).
        let req = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))));
        assert_eq!(classify_host_request(&req), HostRequestKind::Sampling);
    }

    #[test]
    fn classify_roots_and_elicitation() {
        let roots = Request::Server(Box::new(ServerRequest::ListRoots));
        assert_eq!(classify_host_request(&roots), HostRequestKind::Roots);

        let elicit = Request::Server(Box::new(ServerRequest::ElicitationCreate(Box::new(
            crate::types::elicitation::ElicitRequestParams::Form {
                message: "please".to_string(),
                requested_schema: serde_json::json!({}),
            },
        ))));
        assert_eq!(classify_host_request(&elicit), HostRequestKind::Elicitation);
    }

    #[test]
    fn classify_unhandled_client_request() {
        let req = Request::Client(Box::new(ClientRequest::ListTools(Default::default())));
        assert_eq!(classify_host_request(&req), HostRequestKind::Unhandled);
    }
}
