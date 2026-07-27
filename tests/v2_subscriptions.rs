//! Phase 113-10 (HTTP-04): live-HTTP acceptance for `subscriptions/listen`.
//!
//! Both conformant configurations are proven over a REAL loopback socket:
//!
//! * **advertise nothing** — `server/discover` publishes no subscription-delivered
//!   capability AND `subscriptions/listen` answers `404` + `-32601`. The
//!   conformance rule is the CONJUNCTION: a `-32601` without an observed discover
//!   is recorded FAILURE, not SKIPPED, so both halves are asserted in one test.
//! * **advertise anything** — the stream is SERVED, ack-first, `subscriptionId`
//!   tagged, filter-respecting, collision-free across callers sharing a request
//!   id, and reclaiming its slot on disconnect.
//!
//! # Why a raw TCP client rather than `reqwest`
//!
//! Reading a long-lived SSE body requires a STREAMING read; the shared harness's
//! `post` helper reads to EOF and would hang forever on a stream that never ends.
//! `reqwest` is compiled here without its `stream` feature, so [`SseStream`]
//! below speaks HTTP/1.1 over a `tokio::net::TcpStream` directly. That also buys
//! the deterministic client-disconnect this file needs: dropping the socket IS
//! the disconnect, with no connection pool holding it open.
//!
//! EVERY stream read is wrapped in a [`tokio::time::timeout`], so a hung or
//! never-acknowledged stream fails the test instead of wedging CI (T-113-36).
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server_with, post, spawn_default_config, spawn_shared, v2_body, v2_headers,
    BearerSubjects, GreetingPrompt, SearchTool, FRAME_TIMEOUT, V1, V2,
};
use pmcp::server::Server;
use pmcp::types::protocol::error_codes::{METHOD_NOT_FOUND, RATE_LIMITED};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::subscriptions::{
    advertises_subscriptions, ACKNOWLEDGED_METHOD, SUBSCRIPTION_ID_META_KEY,
};
use pmcp::types::{
    PromptCapabilities, ResourceCapabilities, ServerCapabilities, ServerNotification,
    ToolCapabilities,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// ===========================================================================
// Servers.
// ===========================================================================

/// The four subscription-delivered capabilities, by their conformance names.
const CAPABILITY_NAMES: [&str; 4] = [
    "tools.listChanged",
    "prompts.listChanged",
    "resources.listChanged",
    "resources.subscribe",
];

/// `ServerCapabilities` advertising exactly ONE of the four, or none.
///
/// Registering handlers AFTER `.capabilities(..)` only fills sub-capabilities
/// that are still `None`, and pmcp's registration defaults are `Some(false)`, so
/// the advertise-nothing server really does advertise nothing.
fn advertising(which: Option<&str>) -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    match which {
        Some("tools.listChanged") => {
            caps.tools = Some(ToolCapabilities {
                list_changed: Some(true),
            });
        },
        Some("prompts.listChanged") => {
            caps.prompts = Some(PromptCapabilities {
                list_changed: Some(true),
            });
        },
        Some("resources.listChanged") => {
            caps.resources = Some(ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(true),
            });
        },
        Some("resources.subscribe") => {
            caps.resources = Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(false),
            });
        },
        _ => {},
    }
    caps
}

/// A v2-opted-in server with the given capabilities and one handler per method.
fn server_with(caps: ServerCapabilities) -> Server {
    build_v2_server_with("v2-subscriptions", caps)
}

/// The two-principal server the `ListenKey` collision test drives.
fn server_with_two_principals() -> Server {
    let mut caps = ServerCapabilities::default();
    caps.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    caps.prompts = Some(PromptCapabilities {
        list_changed: Some(true),
    });
    Server::builder()
        .name("v2-subscriptions-auth")
        .version("1.0.0")
        .capabilities(caps)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .auth_provider(BearerSubjects)
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .build()
        .expect("server builds")
}

/// Spawn a server this test does not need a handle to.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(server).await
}

// ===========================================================================
// Request bodies.
// ===========================================================================

/// A `subscriptions/listen` body requesting `filter`.
fn listen_body(id: Value, filter: &Value) -> String {
    // Built through a `Map` rather than the `json!` macro because the macro
    // BORROWS its interpolated values, which would leave `id` passed by value
    // but never consumed.
    let mut params = serde_json::Map::new();
    params.insert("notifications".to_string(), filter.clone());
    v2_body("subscriptions/listen", id, Value::Object(params))
}

/// The three required v2 headers for `subscriptions/listen` (a name-less method,
/// so `Mcp-Name` is present and empty — the locked cross-plan rule).
fn listen_headers() -> Vec<(String, String)> {
    v2_headers("subscriptions/listen", "")
}

// ===========================================================================
// Raw streaming SSE client.
// ===========================================================================

/// One parsed SSE event: either a `data:` payload or a comment line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SseEvent {
    Data(String),
    Comment(String),
}

/// A minimal HTTP/1.1 client that can read a response body INCREMENTALLY.
///
/// It handles BOTH framings the listen route produces: a `chunked` SSE stream
/// for a served subscription, and a `Content-Length` JSON body for every
/// rejection (`-32601`, `-32602`, the concurrency refusal). A test therefore
/// reads the first frame the same way regardless of which it got.
struct SseStream {
    reader: BufReader<TcpStream>,
    status: u16,
    headers: Vec<(String, String)>,
    /// Undelivered decoded body text.
    buffer: String,
    /// `true` when the body is `Transfer-Encoding: chunked`.
    chunked: bool,
    /// Remaining `Content-Length` bytes, for the non-chunked framing.
    remaining: usize,
    /// `true` once the body has signalled its end.
    finished: bool,
}

impl SseStream {
    /// POST `body` and read only as far as the response headers.
    async fn open(addr: SocketAddr, extra: &[(String, String)], body: &str) -> Self {
        let stream = TcpStream::connect(addr).await.expect("connects");
        let mut request = format!(
            "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);

        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .expect("request written");

        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .await
            .expect("status line");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("header line");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }

        let chunked = headers
            .iter()
            .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"));
        let remaining = headers
            .iter()
            .find(|(n, _)| n == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        Self {
            reader,
            status,
            headers,
            buffer: String::new(),
            chunked,
            remaining,
            finished: false,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Read more body bytes into [`Self::buffer`], in whichever framing applies.
    async fn pull(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if !self.chunked {
            // `Content-Length` framing: one read, then the body is complete.
            let mut payload = vec![0u8; self.remaining];
            let ok = self.remaining > 0 && self.reader.read_exact(&mut payload).await.is_ok();
            self.finished = true;
            if !ok {
                return false;
            }
            self.buffer.push_str(&String::from_utf8_lossy(&payload));
            return true;
        }
        let mut size_line = String::new();
        if self.reader.read_line(&mut size_line).await.unwrap_or(0) == 0 {
            self.finished = true;
            return false;
        }
        let size_token = size_line.trim().split(';').next().unwrap_or("").to_string();
        let Ok(size) = usize::from_str_radix(&size_token, 16) else {
            self.finished = true;
            return false;
        };
        if size == 0 {
            self.finished = true;
            return false;
        }
        let mut payload = vec![0u8; size];
        if self.reader.read_exact(&mut payload).await.is_err() {
            self.finished = true;
            return false;
        }
        let mut crlf = [0u8; 2];
        let _ = self.reader.read_exact(&mut crlf).await;
        self.buffer.push_str(&String::from_utf8_lossy(&payload));
        true
    }

    /// Pop one complete SSE block (`...\n\n`) from the buffer, if present.
    fn take_block(&mut self) -> Option<String> {
        let end = self.buffer.find("\n\n")?;
        let block = self.buffer[..end].to_string();
        self.buffer.drain(..end + 2);
        Some(block)
    }

    /// The next SSE event, or `None` at end of stream.
    ///
    /// ALWAYS call this through [`Self::expect_event`] / [`Self::expect_no_event`]
    /// so the read is bounded.
    async fn next_event(&mut self) -> Option<SseEvent> {
        loop {
            if let Some(block) = self.take_block() {
                let mut data = String::new();
                let mut comment = None;
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        data.push_str(rest.trim_start());
                    } else if let Some(rest) = line.strip_prefix(':') {
                        comment = Some(rest.trim().to_string());
                    }
                }
                if !data.is_empty() {
                    return Some(SseEvent::Data(data));
                }
                if let Some(comment) = comment {
                    return Some(SseEvent::Comment(comment));
                }
                continue;
            }
            if !self.pull().await {
                // A `Content-Length` JSON body has no `\n\n` block; flush it as
                // the single frame it is.
                if !self.buffer.trim().is_empty() {
                    let rest = std::mem::take(&mut self.buffer);
                    return Some(SseEvent::Data(rest.trim().to_string()));
                }
                return None;
            }
        }
    }

    /// The next `data:` payload, parsed as JSON. Bounded by [`FRAME_TIMEOUT`].
    async fn expect_json(&mut self) -> Value {
        loop {
            let event = tokio::time::timeout(FRAME_TIMEOUT, self.next_event())
                .await
                .expect("a frame arrived within the timeout")
                .expect("the stream did not end");
            if let SseEvent::Data(data) = event {
                return serde_json::from_str(&data).expect("the frame is JSON");
            }
            // A keep-alive comment is not a protocol frame; keep reading.
        }
    }

    /// Assert NO data frame arrives within `window`.
    async fn expect_no_json(&mut self, window: Duration) {
        if let Ok(Some(SseEvent::Data(data))) =
            tokio::time::timeout(window, self.next_event()).await
        {
            panic!("unexpected frame delivered to this stream: {data}");
        }
    }
}

/// The `subscriptionId` carried by any listen frame (a notification's
/// `params._meta` or a result's `result._meta`).
///
/// Indexed rather than read via `Value::pointer`: the reserved key CONTAINS a
/// `/`, which JSON Pointer would treat as a path separator unless escaped.
fn subscription_id_of(frame: &Value) -> Option<&Value> {
    ["params", "result"].into_iter().find_map(|section| {
        frame
            .get(section)?
            .get("_meta")?
            .get(SUBSCRIPTION_ID_META_KEY)
    })
}

// ===========================================================================
// Tests.
// ===========================================================================

/// The DEFAULT, conformant-by-absence configuration.
///
/// The conformance rule is a CONJUNCTION — a `-32601` is SKIPPED only when
/// `server/discover` was observed AND advertises nothing subscription-delivered
/// — so both halves are asserted here, in one test.
#[tokio::test]
async fn absent_capability_is_conformant() {
    let (addr, handle) = spawn(server_with(advertising(None))).await;

    let discover = post(
        addr,
        &v2_headers("server/discover", ""),
        &v2_body("server/discover", json!(1), json!({})),
    )
    .await;
    assert_eq!(discover.status, 200, "discover must be OBSERVED");
    let capabilities: ServerCapabilities =
        serde_json::from_value(discover.body["result"]["capabilities"].clone())
            .expect("the projection deserializes");
    assert!(
        !advertises_subscriptions(&capabilities),
        "the default advertises no subscription-delivered capability: {:?}",
        discover.body["result"]["capabilities"]
    );

    let listen = post(addr, &listen_headers(), &listen_body(json!(2), &json!({}))).await;
    assert_eq!(listen.status, 404, "spec: unimplemented method is 404");
    assert_eq!(listen.body["error"]["code"], json!(METHOD_NOT_FOUND));
    assert_eq!(listen.body["id"], json!(2), "the ORIGINAL id is echoed");

    handle.abort();
}

/// THE tripwire: advertising ANY of the four means the stream is SERVED.
///
/// Each capability is exercised INDIVIDUALLY, which is exactly the conformance
/// rule ("claims a feature it does not serve") encoded locally.
#[tokio::test]
async fn advertise_implies_serve() {
    for which in CAPABILITY_NAMES {
        let (addr, handle) = spawn(server_with(advertising(Some(which)))).await;

        let mut stream = SseStream::open(
            addr,
            &listen_headers(),
            &listen_body(json!(1), &json!({ "toolsListChanged": true })),
        )
        .await;

        assert_eq!(
            stream.status, 200,
            "{which} is advertised, so the stream must be served"
        );
        assert_eq!(
            stream.header("content-type"),
            Some("text/event-stream"),
            "{which}: the served response is an SSE stream"
        );
        let first = stream.expect_json().await;
        assert_ne!(
            first["error"]["code"],
            json!(METHOD_NOT_FOUND),
            "{which} is advertised, so -32601 here would be a conformance FAILURE"
        );
        assert_eq!(
            first["method"],
            json!(ACKNOWLEDGED_METHOD),
            "{which}: the first frame is the acknowledgement"
        );

        drop(stream);
        handle.abort();
    }
}

/// The served stream's wire protocol: SSE content type, ack first, matching
/// `subscriptionId` on the ack AND on every subsequent notification.
#[tokio::test]
async fn listen_stream_protocol() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "tools.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(11), &json!({ "toolsListChanged": true })),
    )
    .await;

    assert_eq!(stream.status, 200);
    assert_eq!(stream.header("content-type"), Some("text/event-stream"));
    assert_eq!(
        stream.header("x-accel-buffering"),
        Some("no"),
        "spec: servers SHOULD disable proxy buffering on the stream"
    );

    let ack = stream.expect_json().await;
    assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
    assert_eq!(
        ack["params"]["notifications"],
        json!({ "toolsListChanged": true }),
        "the ack reports the AGREED filter"
    );
    assert_eq!(
        subscription_id_of(&ack),
        Some(&json!(11)),
        "the subscriptionId equals the listen request's JSON-RPC id"
    );

    // Drive the server's REAL notification path.
    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let notification = stream.expect_json().await;
    assert_eq!(
        notification["method"],
        json!("notifications/tools/list_changed")
    );
    assert_eq!(
        subscription_id_of(&notification),
        Some(&json!(11)),
        "every subsequent frame carries the SAME subscriptionId"
    );

    drop(stream);
    handle.abort();
}

/// A notification type the client did not request is never delivered.
#[tokio::test]
async fn no_unrequested_notification_types() {
    let mut caps = ServerCapabilities::default();
    caps.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    caps.prompts = Some(PromptCapabilities {
        list_changed: Some(true),
    });
    let server = Arc::new(Mutex::new(server_with(caps)));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    // TWO advertised, only ONE requested.
    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(21), &json!({ "toolsListChanged": true })),
    )
    .await;
    let ack = stream.expect_json().await;
    assert_eq!(
        ack["params"]["notifications"],
        json!({ "toolsListChanged": true }),
        "the agreed filter is never a superset of the request"
    );

    // Trigger BOTH change notifications.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::PromptsChanged)
            .await;
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "only the REQUESTED type appears, and it appears FIRST despite prompts \
         being triggered first"
    );
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
}

/// No notification frame may precede the acknowledgement.
///
/// The change notification is fired IMMEDIATELY after the request goes out and
/// before anything is read, which is the tightest race this can be put under.
#[tokio::test]
async fn ack_is_first_frame() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "tools.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(31), &json!({ "toolsListChanged": true })),
    )
    .await;
    for _ in 0..5 {
        server
            .lock()
            .await
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    let first = stream.expect_json().await;
    assert_eq!(
        first["method"],
        json!(ACKNOWLEDGED_METHOD),
        "the acknowledgement MUST be the first message on the stream"
    );

    drop(stream);
    handle.abort();
}

/// Two DIFFERENT principals both using JSON-RPC id `1` must not cross-deliver.
///
/// This is the live half of the `ListenKey { principal, request_id }` fix: an
/// id-keyed registry would have bob's registration EVICT alice's, and alice —
/// the only caller who asked for `toolsListChanged` — would receive nothing.
#[tokio::test]
async fn two_callers_same_request_id_do_not_cross() {
    let server = Arc::new(Mutex::new(server_with_two_principals()));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut alice_headers = listen_headers();
    alice_headers.push(("authorization".to_string(), "Bearer alice".to_string()));
    let mut bob_headers = listen_headers();
    bob_headers.push(("authorization".to_string(), "Bearer bob".to_string()));

    // BOTH use id 1.
    let mut alice = SseStream::open(
        addr,
        &alice_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    let mut bob = SseStream::open(
        addr,
        &bob_headers,
        &listen_body(json!(1), &json!({ "promptsListChanged": true })),
    )
    .await;

    for stream in [&mut alice, &mut bob] {
        let ack = stream.expect_json().await;
        assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
        assert_eq!(subscription_id_of(&ack), Some(&json!(1)));
    }

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let delivered = alice.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "alice's entry survived bob's registration under the SAME request id"
    );
    bob.expect_no_json(Duration::from_millis(300)).await;

    drop(alice);
    drop(bob);
    handle.abort();
}

/// The SAME-principal twin of the test above — the half that shipped UNTESTED.
///
/// Plan 113-10 proved id reuse only ACROSS principals (both tests that claimed
/// to cover it used two different subjects), and `113-VERIFICATION.md` gap item
/// 4 recorded that omission after independently reproducing the defect: two
/// connections authenticated as ONE subject — several tabs, a shared service
/// account, a token with a constant `sub` — collapse onto ONE principal and can
/// still choose the same JSON-RPC id.
///
/// Before the plan-14 fix the second registration EVICTED the first (dropping
/// its `mpsc::Sender`, ending that stream with no terminal frame), so the
/// closing assertion here — that the FIRST stream still receives a fanned-out
/// `tools/list_changed` — is the load-bearing one: alice-1 was already
/// disconnected at that point and the read would time out.
///
/// Plan 113-18 changed only the SHAPE of the refusal, never its existence: the
/// duplicate now answers the RETRYABLE `-32005` at HTTP 200 instead of `-32600`
/// at HTTP 400, because the condition is transient server state rather than a
/// malformed request. The status and code assertions below moved with it; the
/// two MESSAGE assertions did not, and are now the only thing distinguishing
/// this refusal from a capacity refusal.
#[tokio::test]
async fn same_principal_id_reuse_rejects_the_second_and_spares_the_first() {
    let server = Arc::new(Mutex::new(server_with_two_principals()));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    // The ONE difference from the cross-principal twin above: the second caller
    // presents alice's subject too, so both resolve to `AuthContext.subject ==
    // "alice"` and share ONE principal.
    let mut first_headers = listen_headers();
    first_headers.push(("authorization".to_string(), "Bearer alice".to_string()));
    let mut second_headers = listen_headers();
    second_headers.push(("authorization".to_string(), "Bearer alice".to_string()));

    let mut first = SseStream::open(
        addr,
        &first_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    let ack = first.expect_json().await;
    assert_eq!(
        ack["method"],
        json!(ACKNOWLEDGED_METHOD),
        "the first stream is served, ack first"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(1)));

    // The SAME principal, the SAME id, a second connection.
    let mut second = SseStream::open(
        addr,
        &second_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    assert_eq!(
        second.status, 200,
        "a duplicate is a transient, RETRYABLE condition: RATE_LIMITED is not in \
         v2_status_for_code's 400 arm, so it answers at 200 with a JSON-RPC error \
         body, exactly as both capacity refusals already do"
    );
    let refusal = second.expect_json().await;
    assert!(
        refusal["error"].is_object(),
        "the second stream is refused, not served: {refusal}"
    );
    assert_eq!(
        refusal["error"]["code"],
        json!(RATE_LIMITED),
        "the refusal is the RETRYABLE -32005, not the non-retryable -32600 it \
         answered with before 113-18: {refusal}"
    );
    let message = refusal["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("already open for this subscription id"),
        "the refusal names the real reason: {refusal}"
    );
    assert!(
        !message.contains("too many concurrent"),
        "this is a DUPLICATE refusal, not a cap refusal: {refusal}"
    );

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let delivered = first.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "the FIRST stream survived the duplicate registration"
    );
    assert_eq!(
        subscription_id_of(&delivered),
        Some(&json!(1)),
        "and is still tagged with its own subscriptionId"
    );

    drop(first);
    drop(second);
    handle.abort();
}

/// Dropping a client connection reclaims BOTH the registry entry and the
/// concurrency permit, with no explicit unregister call anywhere.
#[tokio::test]
async fn disconnect_releases_registry_slot() {
    let (addr, handle) = spawn(server_with_two_principals()).await;

    let mut headers = listen_headers();
    headers.push(("authorization".to_string(), "Bearer capped".to_string()));

    // Open streams up to the per-principal cap. The cap is a private constant,
    // so this walks up until the server refuses.
    let mut held = Vec::new();
    let mut refusal = None;
    for id in 0..16 {
        let mut stream = SseStream::open(
            addr,
            &headers,
            &listen_body(json!(id), &json!({ "toolsListChanged": true })),
        )
        .await;
        let first = stream.expect_json().await;
        if first["error"].is_object() {
            refusal = Some(first);
            break;
        }
        held.push(stream);
    }
    let refusal = refusal.expect("the per-principal cap must refuse an N+1th stream");
    assert!(
        refusal["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("too many concurrent"),
        "the refusal names the concurrency bound: {refusal}"
    );
    assert!(!held.is_empty(), "some streams were accepted first");

    // Disconnect ONE client and let the server observe the closed socket.
    drop(held.pop().expect("at least one open stream"));

    // The RAII guard releases asynchronously (the server notices the dropped
    // connection), so poll for the reclaimed slot rather than sleeping blindly.
    let mut accepted = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut probe = SseStream::open(
            addr,
            &headers,
            &listen_body(json!(99), &json!({ "toolsListChanged": true })),
        )
        .await;
        let first = probe.expect_json().await;
        if first["error"].is_object() {
            drop(probe);
            continue;
        }
        assert_eq!(first["method"], json!(ACKNOWLEDGED_METHOD));
        held.push(probe);
        accepted = true;
        break;
    }
    assert!(
        accepted,
        "a disconnect must release the registry entry AND the permit"
    );

    drop(held);
    handle.abort();
}

/// `resources/subscribe` and `resources/unsubscribe` are GONE on v2.
#[tokio::test]
async fn v2_resources_subscribe_gone() {
    let (addr, handle) = spawn(server_with(advertising(Some("resources.subscribe")))).await;

    for method in ["resources/subscribe", "resources/unsubscribe"] {
        let response = post(
            addr,
            &v2_headers(method, ""),
            &v2_body(method, json!(1), json!({ "uri": "mem://greeting" })),
        )
        .await;
        assert_eq!(response.status, 404, "{method} is retired on v2");
        assert_eq!(
            response.body["error"]["code"],
            json!(METHOD_NOT_FOUND),
            "{method}: -32601"
        );
        assert_eq!(
            response.body["id"],
            json!(1),
            "{method}: original id echoed"
        );
    }

    handle.abort();
}

/// The v1 `resources/subscribe` flow still works on the SAME server.
#[tokio::test]
async fn v1_subscribe_unchanged() {
    use common::v2::v1_body;

    let (addr, handle) = spawn(server_with(advertising(Some("resources.subscribe")))).await;

    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(1),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "0.0.0" },
            }),
        ),
    )
    .await;
    assert_eq!(init.status, 200, "the v1 handshake still works");
    let session = init
        .mcp_session_id
        .clone()
        .expect("v1 mints a session id (HTTP-01 leaves v1 untouched)");

    let subscribe = post(
        addr,
        &[
            ("mcp-session-id".to_string(), session.clone()),
            ("mcp-protocol-version".to_string(), V1.to_string()),
        ],
        &v1_body(
            "resources/subscribe",
            json!(2),
            json!({ "uri": "mem://greeting" }),
        ),
    )
    .await;
    assert_eq!(subscribe.status, 200, "v1 subscribe is untouched");
    assert!(
        subscribe.body["error"].is_null(),
        "v1 subscribe must not be retired: {}",
        subscribe.body
    );

    handle.abort();
}
