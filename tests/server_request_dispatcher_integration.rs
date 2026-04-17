//! Integration test for `ServerRequestDispatcher` end-to-end round-trip
//! (Phase 70 / PARITY-HANDLER-01 — plumbing foundation).
//!
//! Proves the correlation layer works outside the lib-internal unit tests:
//! `dispatch(ServerRequest::X)` enqueues onto an outbound mpsc channel, a
//! drain-like reader can read the correlated pair, and
//! `handle_response(id, value)` routes the response back to the awaiting
//! caller. Covers Codex review Finding 3 — the gap that existed before
//! Phase 70 Plan 02 landed: client responses were dropped by
//! `Server::spawn_message_handler`.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use pmcp::__test_support::{ServerRequest, ServerRequestDispatcher};

/// Proves the correlation layer: one dispatch, one fulfill, one result.
#[tokio::test]
async fn test_single_request_response_roundtrip() {
    let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
    let dispatcher = Arc::new(ServerRequestDispatcher::new_with_channel(tx));

    // Fire the dispatch in a background task — same shape as a tool
    // handler invoking `peer.list_roots()` from inside `handle()`.
    let dispatch_fut = {
        let d = dispatcher.clone();
        tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await })
    };

    // Drain the outbound channel — same role as
    // spawn_server_request_drain in Server::run.
    let (correlation_id, request) = tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("outbound deadline")
        .expect("outbound channel closed");
    assert!(matches!(request, ServerRequest::ListRoots));
    assert!(
        !correlation_id.is_empty(),
        "correlation id must be non-empty"
    );

    // Simulate the client's response arriving at the transport layer —
    // same role as `handle_transport_message` routing a
    // TransportMessage::Response through the dispatcher.
    let response_payload = serde_json::json!({"roots": []});
    dispatcher
        .handle_response(&correlation_id, response_payload.clone())
        .await
        .expect("handle_response must succeed");

    // Dispatch awaits the oneshot — must now return Ok with the payload.
    let result = dispatch_fut
        .await
        .expect("task panic")
        .expect("dispatch must succeed");
    assert_eq!(result, response_payload);
    assert_eq!(dispatcher.pending_count().await, 0);
}

/// Proves concurrent dispatches correlate by id, not by arrival order.
///
/// Three in-flight dispatches, responses arrive in reverse order. If the
/// dispatcher were FIFO-correlated (the pre-Phase-70 bug shape), dispatches
/// would receive the wrong payloads. The pending map being correlation-id
/// keyed is what makes this test pass.
#[tokio::test]
async fn test_concurrent_multiplex_out_of_order() {
    let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(8);
    let dispatcher = Arc::new(
        ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
    );

    // Fire three concurrent dispatches.
    let d1 = dispatcher.clone();
    let fut_a = tokio::spawn(async move { d1.dispatch(ServerRequest::ListRoots).await });
    let d2 = dispatcher.clone();
    let fut_b = tokio::spawn(async move { d2.dispatch(ServerRequest::ListRoots).await });
    let d3 = dispatcher.clone();
    let fut_c = tokio::spawn(async move { d3.dispatch(ServerRequest::ListRoots).await });

    // Drain all three outbound requests; correlation ids must be unique.
    let (id_a, _) = rx.recv().await.expect("a");
    let (id_b, _) = rx.recv().await.expect("b");
    let (id_c, _) = rx.recv().await.expect("c");
    assert_ne!(id_a, id_b);
    assert_ne!(id_b, id_c);
    assert_ne!(id_a, id_c);

    // Build distinct per-request payloads so we can assert correct routing.
    let resp_a = serde_json::json!({"roots": [{"uri":"file:///a"}]});
    let resp_b = serde_json::json!({"roots": [{"uri":"file:///b"}]});
    let resp_c = serde_json::json!({"roots": [{"uri":"file:///c"}]});

    // Fulfill IN REVERSE ORDER (c, then b, then a) to prove correlation
    // works by id — not by arrival order.
    dispatcher
        .handle_response(&id_c, resp_c.clone())
        .await
        .expect("fulfill c");
    dispatcher
        .handle_response(&id_b, resp_b.clone())
        .await
        .expect("fulfill b");
    dispatcher
        .handle_response(&id_a, resp_a.clone())
        .await
        .expect("fulfill a");

    assert_eq!(fut_a.await.unwrap().unwrap(), resp_a);
    assert_eq!(fut_b.await.unwrap().unwrap(), resp_b);
    assert_eq!(fut_c.await.unwrap().unwrap(), resp_c);
    assert_eq!(dispatcher.pending_count().await, 0);
}
