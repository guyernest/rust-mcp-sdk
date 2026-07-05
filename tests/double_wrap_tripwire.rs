//! TOUT-02 double-wrap tripwire acceptance gate (Phase 104, Plan 03).
//!
//! Proves the high-precision structural detector that WARNs (all builds) and
//! `debug_assert!`-fails (debug builds) when dispatch is about to text-wrap a
//! `Value` that STRUCTURALLY resembles an already-built `CallToolResult` — the
//! exact silent bug that caused the agent-lake 2-week outage. "One local run
//! would have caught it."
//!
//! The `looks_like_call_tool_result` marker fn and the `double_wrap_tripwire`
//! decision fn are crate-private (`task_dispatch` is a `pub(crate) mod`); this
//! integration binary reaches them through the hidden `pmcp::__test_support`
//! seam (mirroring how `tests/server_request_dispatcher_integration.rs` reaches
//! the otherwise-`pub(crate)` dispatcher). Testing at the helper level keeps the
//! debug-panic behavior observable WITHOUT spinning up a full dispatch that a
//! `debug_assert!` would abort mid-call (Codex MEDIUM: such integration tests
//! are brittle).
//!
//! Task 1: the six `looks_like_call_tool_result` behavior cases plus a
//! `proptest` precision fuzz. Task 2: the `double_wrap_tripwire` decision/panic
//! tests (helper level) and the end-to-end suppression parity across `Server`
//! and `ServerCore`.

#![cfg(not(target_arch = "wasm32"))]

use pmcp::__test_support::{double_wrap_tripwire, looks_like_call_tool_result, DoubleWrapMarker};
use serde_json::json;

// ---------------------------------------------------------------------------
// Task 1 — looks_like_call_tool_result: the six behavior cases.
// ---------------------------------------------------------------------------

/// `_meta` carrying the related-task key → `RelatedTaskMeta` (checked first).
#[test]
fn looks_like_fires_on_related_task_meta() {
    let v = json!({
        "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "t1" } }
    });
    assert_eq!(
        looks_like_call_tool_result(&v),
        Some(DoubleWrapMarker::RelatedTaskMeta)
    );
}

/// A NON-EMPTY `content` array whose every element is a `Content` → `ContentArray`.
#[test]
fn looks_like_fires_on_content_array() {
    let v = json!({ "content": [ { "type": "text", "text": "hi" } ] });
    assert_eq!(
        looks_like_call_tool_result(&v),
        Some(DoubleWrapMarker::ContentArray)
    );
}

/// The full envelope shape (`content` + other `CallToolResult` keys) fires —
/// the envelope-keys guard admits every legitimate `CallToolResult` key.
#[test]
fn looks_like_fires_on_full_envelope_keys() {
    let v = json!({
        "content": [ { "type": "text", "text": "hi" } ],
        "isError": false,
        "structuredContent": { "answer": 42 }
    });
    assert_eq!(
        looks_like_call_tool_result(&v),
        Some(DoubleWrapMarker::ContentArray)
    );
}

/// A chat-message-shaped payload (`role` + `content` array of Content-parsable
/// elements — Anthropic/OpenAI style, ubiquitous in LLM-proxying tools) must
/// NOT fire: foreign keys mark it as NOT a `CallToolResult` envelope (WR-02).
#[test]
fn looks_like_ignores_chat_message_payload() {
    let v = json!({
        "role": "assistant",
        "content": [ { "type": "text", "text": "hello from the model" } ]
    });
    assert_eq!(looks_like_call_tool_result(&v), None);

    // Same with extra LLM-response keys.
    let v = json!({
        "role": "assistant",
        "model": "some-model",
        "stopReason": "end_turn",
        "content": [ { "type": "text", "text": "hello" } ]
    });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// An EMPTY `content: []` must NOT fire (a benign payload can carry one).
#[test]
fn looks_like_ignores_empty_content_array() {
    let v = json!({ "content": [] });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A `content` array holding a non-`Content` element must NOT fire (the
/// internally tagged enum rejects an element without a valid `"type"`).
#[test]
fn looks_like_ignores_non_content_element() {
    let v = json!({ "content": [ "not-a-content-item" ] });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A plain benign object with neither marker → `None`.
#[test]
fn looks_like_ignores_benign_object() {
    let v = json!({ "foo": 1 });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A non-object JSON value (e.g. a bare number) → `None` (no panic).
#[test]
fn looks_like_ignores_non_object() {
    assert_eq!(looks_like_call_tool_result(&json!(42)), None);
    assert_eq!(looks_like_call_tool_result(&json!("string")), None);
    assert_eq!(looks_like_call_tool_result(&json!([1, 2, 3])), None);
}

// ---------------------------------------------------------------------------
// Task 1 — proptest precision: near-zero false positives.
// ---------------------------------------------------------------------------

mod precision {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for arbitrary JSON scalars/containers that are NOT built-result
    /// markers: scalars, and small objects/arrays whose keys avoid `_meta` and
    /// whose `content`, if present, holds only non-`Content` scalars.
    fn benign_json() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            // Keys/strings deliberately never equal the marker key.
            "[a-z]{1,6}".prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(3, 16, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
                prop::collection::hash_map("[a-z]{1,6}", inner, 0..4).prop_map(|m| {
                    serde_json::Value::from(m.into_iter().collect::<serde_json::Map<_, _>>())
                }),
            ]
        })
    }

    proptest! {
        /// PROPERTY: an object lacking BOTH markers (no `_meta[related-task]`,
        /// and no non-empty all-`Content` `content` array) NEVER returns `Some`.
        /// The `benign_json` strategy never emits the marker key nor a valid
        /// `Content` element, so the detector must always yield `None`.
        #[test]
        fn benign_json_never_trips(v in benign_json()) {
            prop_assert_eq!(looks_like_call_tool_result(&v), None);
        }
    }
}

// ---------------------------------------------------------------------------
// Task 2 — double_wrap_tripwire decision fn (helper level, no dispatch).
// ---------------------------------------------------------------------------

mod decision {
    use super::*;

    /// A value that structurally resembles a built `CallToolResult`.
    fn tripping() -> serde_json::Value {
        json!({ "content": [ { "type": "text", "text": "x" } ] })
    }

    /// Suppressed → `None`, NO warn/panic, in EVERY build (short-circuits before
    /// `looks_like` even runs). Safe to assert in both debug and release.
    #[test]
    fn suppressed_never_fires() {
        assert_eq!(double_wrap_tripwire("wrapper", &tripping(), true), None);
    }

    /// Benign value → `None`, NO warn/panic, in every build.
    #[test]
    fn benign_never_fires() {
        let benign = json!({ "answer": 42 });
        assert_eq!(double_wrap_tripwire("plain", &benign, false), None);
    }

    /// RELEASE builds: an unsuppressed tripping value returns `Some(marker)` and
    /// NEVER panics (D-06: `debug_assert!` compiled out). Gated to release so the
    /// debug-build `debug_assert!` panic does not abort this assertion.
    #[cfg(not(debug_assertions))]
    #[test]
    fn release_returns_marker_without_panic() {
        assert_eq!(
            double_wrap_tripwire("wrapper", &tripping(), false),
            Some(DoubleWrapMarker::ContentArray)
        );
    }

    /// DEBUG builds: an unsuppressed tripping value hard-fails via `debug_assert!`.
    /// Exercised at the HELPER level with `catch_unwind` — NO server/dispatch is
    /// spun up (Codex MEDIUM: end-to-end debug-assert tests abort the call path
    /// and are brittle). Gated to debug, where `debug_assertions` is on.
    #[cfg(debug_assertions)]
    #[test]
    fn debug_panics_on_tripping_unsuppressed() {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence the expected panic output
        let outcome =
            std::panic::catch_unwind(|| double_wrap_tripwire("wrapper", &tripping(), false));
        std::panic::set_hook(prev);
        assert!(
            outcome.is_err(),
            "debug build must panic via debug_assert! on an unsuppressed tripping value"
        );
    }

    /// The `_meta` related-task marker also drives the tripwire (release return
    /// value; debug would panic). Proves BOTH markers reach the decision fn.
    #[cfg(not(debug_assertions))]
    #[test]
    fn release_related_task_meta_marker_returns() {
        let v = json!({ "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "t" } } });
        assert_eq!(
            double_wrap_tripwire("wrapper", &v, false),
            Some(DoubleWrapMarker::RelatedTaskMeta)
        );
    }
}

// ---------------------------------------------------------------------------
// Task 2 — end-to-end suppression parity across Server and ServerCore.
//
// A tool that returns a double-wrap-shaped Payload trips the tripwire. With the
// tool registered via `suppress_double_wrap_check`, BOTH dispatchers must honor
// the opt-out: no debug_assert panic, the call completes and text-wraps the
// payload. If the suppression set failed to thread into EITHER dispatcher, the
// debug build would panic mid-dispatch and the client call would fail — so a
// green run in debug proves the set reached both `Server` and `ServerCore`.
// ---------------------------------------------------------------------------

mod parity {
    use async_trait::async_trait;
    use pmcp::server::builder::ServerCoreBuilder;
    use pmcp::server::core::ProtocolHandler;
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::{CallToolResult, ClientCapabilities};
    use pmcp::{Client, Error, RequestHandlerExtra, Result, Server, ToolHandler};
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    /// Name of the tool whose Payload structurally looks like a built result.
    const WRAPPER: &str = "wrapper_tool";

    /// A hand-written tool that returns a Payload `Value` shaped exactly like a
    /// built `CallToolResult` (`content` array of `Content`). Text-wrapping this
    /// is the silent double-wrap bug; the tripwire fires unless suppressed.
    struct WrapperTool;

    #[async_trait]
    impl ToolHandler for WrapperTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(json!({ "content": [ { "type": "text", "text": "already-wrapped" } ] }))
        }
    }

    // --- minimal in-process duplex transport (client <-> server) ---

    struct DuplexTransport {
        tx: mpsc::UnboundedSender<TransportMessage>,
        rx: mpsc::UnboundedReceiver<TransportMessage>,
        connected: bool,
    }

    impl std::fmt::Debug for DuplexTransport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("DuplexTransport")
        }
    }

    impl DuplexTransport {
        fn pair() -> (Self, Self) {
            let (client_tx, server_rx) = mpsc::unbounded_channel();
            let (server_tx, client_rx) = mpsc::unbounded_channel();
            (
                Self {
                    tx: client_tx,
                    rx: client_rx,
                    connected: true,
                },
                Self {
                    tx: server_tx,
                    rx: server_rx,
                    connected: true,
                },
            )
        }
    }

    #[async_trait]
    impl Transport for DuplexTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.tx
                .send(message)
                .map_err(|_| Error::internal("duplex peer dropped"))
        }
        async fn receive(&mut self) -> Result<TransportMessage> {
            self.rx
                .recv()
                .await
                .ok_or_else(|| Error::internal("duplex peer closed"))
        }
        async fn close(&mut self) -> Result<()> {
            self.connected = false;
            Ok(())
        }
        fn is_connected(&self) -> bool {
            self.connected
        }
        fn transport_type(&self) -> &'static str {
            "in-process-duplex"
        }
    }

    fn spawn_core_pump(mut server_transport: DuplexTransport, handler: Arc<dyn ProtocolHandler>) {
        tokio::spawn(async move {
            while let Ok(message) = server_transport.receive().await {
                if let TransportMessage::Request { id, request } = message {
                    let response = handler.handle_request(id, request, None).await;
                    if server_transport
                        .send(TransportMessage::Response(response))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
    }

    async fn call_via_core(
        core: Arc<dyn ProtocolHandler>,
        name: &str,
        args: Value,
    ) -> CallToolResult {
        let (client_t, server_t) = DuplexTransport::pair();
        spawn_core_pump(server_t, core);
        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("client initializes against core");
        client
            .call_tool(name.to_string(), args)
            .await
            .expect("tools/call succeeds against core (suppression must prevent debug panic)")
    }

    async fn call_via_server(server: Server, name: &str, args: Value) -> CallToolResult {
        let (client_t, server_t) = DuplexTransport::pair();
        tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });
        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("client initializes against server");
        client
            .call_tool(name.to_string(), args)
            .await
            .expect("tools/call succeeds against server (suppression must prevent debug panic)")
    }

    /// Assert the suppressed wrapper tool text-wrapped its payload (i.e. the
    /// dispatch completed normally — no tripwire abort).
    fn assert_text_wrapped(result: &CallToolResult) {
        let v = serde_json::to_value(result).expect("serialize CallToolResult");
        let text = v["content"][0]["text"]
            .as_str()
            .expect("content[0].text present");
        assert!(
            text.contains("already-wrapped"),
            "suppressed wrapper payload should be text-wrapped, got: {text}"
        );
    }

    /// `ServerCore` honors `suppress_double_wrap_check`: the wrapper tool completes
    /// (no `debug_assert` panic) and its payload is text-wrapped.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn core_honors_suppression() {
        let core: Arc<dyn ProtocolHandler> = Arc::new(
            ServerCoreBuilder::new()
                .name("tripwire-core")
                .version("1.0.0")
                .tool(WRAPPER, WrapperTool)
                .suppress_double_wrap_check(WRAPPER)
                .build()
                .expect("core builds"),
        );
        let result = call_via_core(core, WRAPPER, json!({})).await;
        assert_text_wrapped(&result);
    }

    /// High-level `Server` honors `suppress_double_wrap_check` identically — the
    /// SAME suppression set semantics on the other dispatcher (no drift).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_honors_suppression() {
        let server = Server::builder()
            .name("tripwire-server")
            .version("1.0.0")
            .tool(WRAPPER, WrapperTool)
            .suppress_double_wrap_check(WRAPPER)
            .build()
            .expect("server builds");
        let result = call_via_server(server, WRAPPER, json!({})).await;
        assert_text_wrapped(&result);
    }
}
