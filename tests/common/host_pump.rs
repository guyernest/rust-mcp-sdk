//! Shared raw-duplex pump helpers for the client host-surface dispatcher tests.
//!
//! Extracted from the (previously copy-pasted) harness in
//! `tests/client_host_roundtrip.rs` / `tests/client_host_approval.rs`: a fake
//! server that answers `initialize`, waits for the client's in-flight
//! `tools/list`, injects a raw inbound server -> client request, and captures
//! the client's `Response`.
//!
//! Each file in `tests/` is compiled as a separate integration crate; include
//! this module per-crate via `#[path = "common/host_pump.rs"] mod host_pump;`
//! alongside the `#[path = "common/duplex.rs"] mod duplex;` include it depends
//! on (this module references `crate::duplex::DuplexTransport`).

#![allow(dead_code)]
#![cfg(not(target_arch = "wasm32"))]

use serde_json::{json, Value};

use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::jsonrpc::JSONRPCResponse;
use pmcp::types::protocol::Request;
use pmcp::types::{Implementation, InitializeResult, RequestId, ServerCapabilities};

use crate::duplex::DuplexTransport;

/// Receive frames until an inbound `Request` arrives, returning its id.
pub async fn recv_request_id(t: &mut DuplexTransport) -> RequestId {
    loop {
        if let TransportMessage::Request { id, .. } = t.receive().await.expect("recv request") {
            return id;
        }
    }
}

/// Receive frames until a `Response` arrives, returning it.
pub async fn recv_response(t: &mut DuplexTransport) -> JSONRPCResponse {
    loop {
        if let TransportMessage::Response(r) = t.receive().await.expect("recv response") {
            return r;
        }
    }
}

/// Send a success `Response` carrying `result` for `id`.
pub async fn send_success(t: &mut DuplexTransport, id: RequestId, result: Value) {
    t.send(TransportMessage::Response(JSONRPCResponse::success(
        id, result,
    )))
    .await
    .expect("send response");
}

/// Build the `initialize` result value a fake tools-only server returns.
pub fn init_result_value() -> Value {
    serde_json::to_value(InitializeResult::new(
        Implementation::new("fake-server", "1.0.0"),
        ServerCapabilities::tools_only(),
    ))
    .unwrap()
}

/// Shared driver prefix: answer `initialize`, wait for the client's in-flight
/// `tools/list` (skipping the `initialized` note), inject `inbound` while the
/// call is in flight, capture the client's `Response` to it, then answer the
/// original call so that first `list_tools` returns. Leaves `server_t` open for
/// any follow-up probe. Returns the captured inbound `Response`.
pub async fn inject_and_capture(
    server_t: &mut DuplexTransport,
    inbound_id: RequestId,
    inbound: Request,
) -> JSONRPCResponse {
    // 1. Answer initialize.
    let init_id = recv_request_id(server_t).await;
    send_success(server_t, init_id, init_result_value()).await;

    // 2. Wait for the in-flight client request (skips the `initialized` note).
    let call_id = recv_request_id(server_t).await;

    // 3. Push the raw inbound request while the call is in flight.
    server_t
        .send(TransportMessage::Request {
            id: inbound_id,
            request: inbound,
        })
        .await
        .expect("send inbound request");

    // 4. Capture the client's Response to the inbound request.
    let response = recv_response(server_t).await;

    // 5. Answer the original call so the client returns.
    send_success(server_t, call_id, json!({ "tools": [] })).await;

    response
}
