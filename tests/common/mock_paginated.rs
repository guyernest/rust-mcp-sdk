//! Shared test helpers for Phase 73 `list_all_*` auto-pagination tests.
//!
//! Encapsulates the `MockTransport` reverse-push quirk: callers pass pages in
//! NATURAL order; [`build_paginated_responses`] internally reverses, chains
//! cursors, and appends the init response at the correct pop position.
//!
//! Used by:
//! - `tests/list_all_pagination.rs` (integration tests)
//! - `tests/property_tests.rs` (property tests)
//!
//! Each file in `tests/` is compiled as a separate integration crate, and
//! this module is included per-crate via `#[path = "common/mock_paginated.rs"] mod mock_paginated;`.

#![allow(dead_code)]
#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use pmcp::{
    shared::Transport,
    types::{jsonrpc::ResponsePayload, JSONRPCResponse, RequestId, TransportMessage},
    Result,
};
use serde::Serialize;
use serde_json::json;
use std::sync::{Arc, Mutex};

/// `MockTransport` variant shared across integration + property tests.
///
/// `receive()` pops from the TAIL of `responses`. Responses must therefore
/// be pushed in REVERSE arrival order — the [`build_paginated_responses`]
/// helper does this for you.
#[derive(Debug)]
pub struct MockTransport {
    pub responses: Arc<Mutex<Vec<TransportMessage>>>,
    pub sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
}

impl MockTransport {
    pub fn with_responses(responses: Vec<TransportMessage>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            sent_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.sent_messages.lock().unwrap().push(message);
        Ok(())
    }
    async fn receive(&mut self) -> Result<TransportMessage> {
        self.responses
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| pmcp::Error::protocol_msg("no more responses"))
    }
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Canonical protocol init response advertising `tools`, `prompts`, and
/// `resources` server capabilities. `resources/templates/list` is gated by
/// the `resources` capability, so this suffices for all four `list_all_*`
/// families.
pub fn init_response() -> TransportMessage {
    TransportMessage::Response(JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(1i64),
        payload: ResponsePayload::Result(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {
                "tools": {},
                "prompts": {},
                "resources": {},
            },
            "serverInfo": { "name": "test-server", "version": "1.0.0" }
        })),
    })
}

/// Which `list_*` family a page sequence belongs to.
///
/// Controls the JSON field name used to hold the page's items
/// (`tools` / `prompts` / `resources` / `resourceTemplates`).
#[derive(Debug, Clone, Copy)]
pub enum PaginationCapability {
    Tools,
    Prompts,
    Resources,
    ResourceTemplates,
}

impl PaginationCapability {
    fn items_field(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::Prompts => "prompts",
            Self::Resources => "resources",
            Self::ResourceTemplates => "resourceTemplates",
        }
    }
}

/// Build a reverse-ordered `Vec<TransportMessage>` suitable for
/// [`MockTransport::with_responses`], given pages in NATURAL order.
///
/// - `init`: the protocol init response — pops FIRST (so it is appended LAST
///   to the vec).
/// - `pages`: page items in natural order. Page `i` gets
///   `next_cursor: Some("p{i+2}")` except the last, which gets
///   `next_cursor: None`.
/// - `cap`: which capability shape to produce.
pub fn build_paginated_responses<T: Serialize>(
    init: TransportMessage,
    pages: Vec<Vec<T>>,
    cap: PaginationCapability,
) -> Vec<TransportMessage> {
    let n = pages.len();
    let mut natural: Vec<TransportMessage> = pages
        .into_iter()
        .enumerate()
        .map(|(i, page_items)| {
            let next = if i + 1 < n {
                Some(format!("p{}", i + 2))
            } else {
                None
            };
            let items_json = serde_json::to_value(&page_items).unwrap_or_else(|_| json!([]));
            let mut payload = json!({ cap.items_field(): items_json });
            if let Some(cur) = next {
                payload["nextCursor"] = json!(cur);
            }
            TransportMessage::Response(JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::from(i64::try_from(i).unwrap_or(i64::MAX) + 2),
                payload: ResponsePayload::Result(payload),
            })
        })
        .collect();
    // Reverse for pop-from-tail; then append init LAST so it pops FIRST.
    natural.reverse();
    natural.push(init);
    natural
}
