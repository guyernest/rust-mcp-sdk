//! Integration test for `DispatchPeerHandle` end-to-end round-trip through
//! the `ServerRequestDispatcher`.
//!
//! Proves: `peer.sample()` / `peer.list_roots()` → `dispatcher.dispatch()` →
//! outbound channel → `handle_response()` → typed result parsing.
//!
//! Uses the `#[doc(hidden)] pub mod __test_support` re-exports from
//! `src/lib.rs` — not part of the stable API surface.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use pmcp::__test_support::{DispatchPeerHandle, ServerRequest, ServerRequestDispatcher};
use pmcp::types::sampling::CreateMessageParams;
use pmcp::PeerHandle;

/// End-to-end round-trip: `DispatchPeerHandle` → dispatcher → outbound channel
/// → `handle_response` → typed `CreateMessageResult` back to the caller.
#[tokio::test]
async fn test_peer_sample_round_trip_through_dispatcher() {
    let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
    let dispatcher = Arc::new(
        ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
    );
    let peer: Arc<dyn PeerHandle> = Arc::new(DispatchPeerHandle::new(dispatcher.clone()));

    // REAL constructor (no Default impl exists — Codex HIGH Finding 4).
    let params = CreateMessageParams::new(Vec::new());

    // Fire sample() in a background task so we can drain the outbound channel.
    let sample_fut = tokio::spawn(async move { peer.sample(params).await });

    // Drain the outbound request emitted by DispatchPeerHandle::sample.
    let (correlation_id, req) = tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("outbound recv deadline")
        .expect("outbound channel closed unexpectedly");
    assert!(
        matches!(req, ServerRequest::CreateMessage(_)),
        "must emit ServerRequest::CreateMessage variant"
    );
    assert!(
        !correlation_id.is_empty(),
        "dispatcher must assign a non-empty correlation id"
    );

    // Inject a well-shaped CreateMessageResult JSON response. Field names must
    // match the #[serde(rename_all = "camelCase")] convention on the type.
    let response = json!({
        "content": {"type": "text", "text": "hello from peer"},
        "model": "mock-model"
    });
    dispatcher
        .handle_response(&correlation_id, response)
        .await
        .expect("handle_response must succeed");

    // Await the sample() call — should receive the parsed result.
    let result = sample_fut
        .await
        .expect("sample task panicked")
        .expect("sample must succeed");
    assert_eq!(result.model, "mock-model");
}

/// End-to-end `list_roots` round-trip through the dispatcher.
#[tokio::test]
async fn test_peer_list_roots_round_trip_through_dispatcher() {
    let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
    let dispatcher = Arc::new(
        ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
    );
    let peer: Arc<dyn PeerHandle> = Arc::new(DispatchPeerHandle::new(dispatcher.clone()));

    let list_fut = tokio::spawn(async move { peer.list_roots().await });

    let (correlation_id, req) = tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("outbound recv deadline")
        .expect("outbound channel closed");
    assert!(
        matches!(req, ServerRequest::ListRoots),
        "must emit ServerRequest::ListRoots variant"
    );

    let response = json!({"roots": []});
    dispatcher
        .handle_response(&correlation_id, response)
        .await
        .expect("handle_response must succeed");

    let result = list_fut
        .await
        .expect("list_roots task panicked")
        .expect("list_roots must succeed");
    assert!(
        result.roots.is_empty(),
        "mock response had empty roots array"
    );
}
