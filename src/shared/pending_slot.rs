//! Target-agnostic one-slot pending-response buffer for one-shot transports.
//!
//! A one-shot request/response transport (e.g. browser Fetch via
//! [`WasmHttpTransport`](crate::shared::wasm_http::WasmHttpTransport)) must
//! adapt the bidirectional-stream model the high-level
//! [`Client`](crate::client::Client) uses: it calls `transport.send(msg)`
//! once, then LOOPS on `transport.receive()` until it correlates the response
//! (see `src/client/mod.rs` send-then-loop-receive). A one-shot POST does the
//! request in `send()` and must BUFFER the parsed response so `receive()` can
//! return it.
//!
//! [`PendingSlot`] is exactly that buffer, factored out as pure plumbing with
//! NO `web_sys` dependency. Because it is target-agnostic and NOT `cfg`-gated,
//! the send→receive correlation contract is provable by a host-target
//! `cargo test` rather than a wasm-only compile check.
//!
//! Semantics (one-slot):
//! - [`PendingSlot::put`] stores a message; it ERRORS (never silently
//!   overwrites) when the slot is already occupied — a double `send()` before
//!   `receive()` surfaces an internal error instead of dropping the first
//!   response (MEDIUM-4).
//! - [`PendingSlot::take`] returns the buffered message; it ERRORS when the
//!   slot is empty — the `receive()`-before-`send()` path.

// Items here are `pub(crate)` so the enclosing `pub(crate) mod pending_slot`
// (LOW-7) keeps `PendingSlot` crate-internal AND `unreachable_pub`-clean.
// `redundant_pub_crate` would prefer bare `pub`, but bare `pub` then trips
// `unreachable_pub` (the module is not publicly reachable) — the two lints are
// mutually exclusive here, so we keep `pub(crate)` and silence the former.
#![allow(clippy::redundant_pub_crate)]

use crate::error::{Error, Result};
use crate::shared::transport::TransportMessage;

/// A one-slot buffer holding at most one pending [`TransportMessage`].
///
/// Used by one-shot HTTP transports to bridge the
/// `send()`-then-loop-`receive()` correlation the high-level client performs:
/// `send()` POSTs and `put`s the parsed response here; `receive()` `take`s it.
///
/// The sole non-test consumer ([`WasmHttpTransport`](crate::shared::wasm_http))
/// is `#[cfg(target_arch = "wasm32")]`, so on a non-test HOST build these items
/// are intentionally unused — the buffer stays ungated purely so the
/// correlation contract is host-testable (see module docs / LOW-7).
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
#[derive(Debug, Clone, Default)]
pub(crate) struct PendingSlot {
    slot: Option<TransportMessage>,
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
impl PendingSlot {
    /// Create an empty pending slot.
    pub(crate) fn new() -> Self {
        Self { slot: None }
    }

    /// Store a message in the one slot.
    ///
    /// # Errors
    ///
    /// Returns [`Error::internal`] when the slot is ALREADY occupied — i.e. a
    /// second `send()` happened before the prior response was `receive()`d. The
    /// buffered response is NEVER silently overwritten (MEDIUM-4).
    pub(crate) fn put(&mut self, message: TransportMessage) -> Result<()> {
        if self.slot.is_some() {
            return Err(Error::internal(
                "send() called before the prior response was received on HTTP transport",
            ));
        }
        self.slot = Some(message);
        Ok(())
    }

    /// Take the buffered message, emptying the slot.
    ///
    /// # Errors
    ///
    /// Returns [`Error::internal`] when the slot is empty — i.e. `receive()`
    /// was called before any `send()` buffered a response.
    pub(crate) fn take(&mut self) -> Result<TransportMessage> {
        self.slot
            .take()
            .ok_or_else(|| Error::internal("receive() called before send() on HTTP transport"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::jsonrpc::{JSONRPCResponse, ResponsePayload};
    use crate::types::RequestId;
    use serde_json::json;

    /// Build a simple, host-constructable `TransportMessage::Response` for tests.
    fn sample_response(id: i64, value: serde_json::Value) -> TransportMessage {
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(value),
        })
    }

    /// Two `TransportMessage`s are "the same" if they serialize identically.
    /// (`TransportMessage` does not derive `PartialEq`.)
    fn same_message(a: &TransportMessage, b: &TransportMessage) -> bool {
        serde_json::to_value(a).unwrap() == serde_json::to_value(b).unwrap()
    }

    #[test]
    fn pending_slot_put_then_take_returns_message() {
        let mut slot = PendingSlot::new();
        let msg = sample_response(1, json!({"ok": true}));
        let expected = sample_response(1, json!({"ok": true}));

        assert!(slot.put(msg).is_ok(), "put into an empty slot must succeed");

        let taken = slot.take().expect("take after put must return the message");
        assert!(
            same_message(&taken, &expected),
            "take() must return exactly the message that was put"
        );
    }

    #[test]
    fn pending_slot_second_take_is_error() {
        let mut slot = PendingSlot::new();
        slot.put(sample_response(2, json!({"v": 1})))
            .expect("first put succeeds");

        // The single buffered message is consumed here.
        slot.take().expect("first take returns the message");

        // A second take() on the now-empty slot is the receive-before-send path.
        assert!(
            slot.take().is_err(),
            "take() on an empty slot must error (receive before send)"
        );
    }

    #[test]
    fn pending_slot_put_on_occupied_is_error() {
        let mut slot = PendingSlot::new();
        slot.put(sample_response(3, json!({"first": true})))
            .expect("first put into empty slot succeeds");

        // A second put() WITHOUT an intervening take() must error (MEDIUM-4):
        // the first response must NOT be silently overwritten.
        let err = slot.put(sample_response(4, json!({"second": true})));
        assert!(
            err.is_err(),
            "put() into an occupied slot must error (no silent overwrite)"
        );

        // Prove the FIRST response survived the rejected double-write.
        let survived = slot.take().expect("first response still buffered");
        assert!(
            same_message(&survived, &sample_response(3, json!({"first": true}))),
            "the originally-buffered response must survive a rejected double put()"
        );
    }

    /// The send→store→receive TRANSPORT contract (MEDIUM-5), modeled on the
    /// EXACT sequence `WasmHttpTransport::send`/`receive` use (see
    /// `crate::shared::wasm_http::WasmHttpTransport`): the parsed `do_request`
    /// output flows through `pending.put(...)`, then `receive()` (`take`)
    /// returns precisely that stored message — with NO real Fetch/`window`.
    #[test]
    fn transport_send_stores_and_receive_returns() {
        // Stand in for the canned `do_request(&message).await?` output that
        // `WasmHttpTransport::send` would buffer.
        let canned = sample_response(42, json!({"result": "from do_request"}));
        let expected = sample_response(42, json!({"result": "from do_request"}));

        let mut slot = PendingSlot::new();

        // send(): self.pending.put(self.do_request(&message).await?)?
        slot.put(canned)
            .expect("send() buffers the do_request output");

        // receive(): self.pending.take()
        let received = slot
            .take()
            .expect("receive() returns the buffered response");

        assert!(
            same_message(&received, &expected),
            "receive() must return exactly what send() stored from do_request"
        );
    }
}
