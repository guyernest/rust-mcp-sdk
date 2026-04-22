//! Fuzz target: `Client::list_all_tools` cursor loop (Phase 73, T-73-01).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run list_all_cursor_loop`.
//!
//! Invariants (tightened per 73-REVIEWS.md MEDIUM finding #5):
//!
//! 1. The loop terminates within `ClientOptions::max_iterations` for any
//!    adversarial cursor sequence (empty strings, very long strings,
//!    repeated values, A->B->A cycles).
//! 2. Cap-exceeded yields `Error::Validation`, never a panic.
//! 3. None-cursor terminates cleanly with `Ok(accumulator)`.
//! 4. Accepted result set is EXACTLY one of:
//!    - `Ok(_)`
//!    - `Err(Error::Validation(_))` (cap-exceeded).
//!    - `Err(Error::Protocol { .. })` (transport-exhaustion: the
//!      MockTransport returns Error::protocol_msg when the scripted
//!      response pool is empty; `Error::parse` also produces
//!      Error::Protocol with ErrorCode::PARSE_ERROR).
//!    - `Err(Error::Serialization(_))` (serde_json::Error during response
//!      deserialization — this is the Parse-like variant on the pmcp
//!      Error enum; it MAY fire on fuzzer-crafted payloads).
//!
//! Any other error variant is a bug and panics the fuzzer.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use async_trait::async_trait;
use libfuzzer_sys::fuzz_target;
use pmcp::{
    shared::Transport,
    types::{
        jsonrpc::ResponsePayload, ClientCapabilities, JSONRPCResponse, RequestId, TransportMessage,
    },
    Client, ClientOptions, Error,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Debug, Arbitrary)]
struct FuzzCursorSeq {
    /// Each entry: `Some(next_cursor)` continues the loop, `None` terminates.
    /// Empty strings, long strings, and repeated values are all valid inputs.
    cursors: Vec<Option<String>>,
    /// Clamped to `1..=200` inside the target to keep runs bounded.
    max_iterations: u8,
}

#[derive(Debug)]
struct MockTransport {
    responses: Arc<Mutex<Vec<TransportMessage>>>,
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&mut self, _: TransportMessage) -> pmcp::Result<()> {
        Ok(())
    }
    async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
        self.responses
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| Error::protocol_msg("no more responses"))
    }
    async fn close(&mut self) -> pmcp::Result<()> {
        Ok(())
    }
}

fn build_responses(seq: &FuzzCursorSeq, cap: usize) -> Vec<TransportMessage> {
    let init = TransportMessage::Response(JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(1i64),
        payload: ResponsePayload::Result(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "fuzz", "version": "0.0.0" }
        })),
    });

    // Script at least `cap + 1` page responses so the transport isn't
    // prematurely exhausted under normal fuzz conditions — this narrows the
    // Protocol-error escape hatch so the oracle's Validation-or-Ok branches
    // dominate.
    let mut page_cursors: Vec<Option<String>> = seq.cursors.iter().take(cap + 1).cloned().collect();
    while page_cursors.len() < cap + 1 {
        // Pad with Some("pad") so the helper sees Some(_) for every
        // in-budget iteration; deterministically forces cap-exceeded.
        page_cursors.push(Some("pad".to_string()));
    }

    let mut pages: Vec<TransportMessage> = page_cursors
        .iter()
        .enumerate()
        .map(|(i, cur)| {
            let mut payload = json!({ "tools": [] });
            if let Some(next) = cur {
                payload["nextCursor"] = json!(next);
            }
            TransportMessage::Response(JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::from((i as i64) + 2),
                payload: ResponsePayload::Result(payload),
            })
        })
        .collect();
    pages.reverse();
    pages.push(init);
    pages
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(seq) = FuzzCursorSeq::arbitrary(&mut u) else {
        return;
    };
    let cap = (seq.max_iterations as usize).clamp(1, 200);

    let responses = build_responses(&seq, cap);
    let transport = MockTransport {
        responses: Arc::new(Mutex::new(responses)),
    };
    let opts = ClientOptions::default().with_max_iterations(cap);

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async move {
        let mut client = Client::with_client_options(transport, opts);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let outcome = client.list_all_tools().await;
        // Tightened oracle — only the narrow expected error set is accepted.
        // A broader `Err(_)` catch-all is intentionally absent so that any
        // unexpected error variant (Internal, Transport, Io, etc.) panics
        // the fuzzer.
        match outcome {
            Ok(_) => {},
            Err(Error::Validation(_)) => {},
            Err(Error::Protocol { .. }) => {},
            Err(Error::Serialization(_)) => {},
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    });
});
