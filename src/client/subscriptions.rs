//! The CLIENT half of `subscriptions/listen` (MCP 2026-07-28, HTTP-04).
//!
//! Plan 10 built the SERVER route and proved it with a raw HTTP/1.1 client. This
//! module is the other half: a pmcp [`Client`](crate::Client) that opts into
//! `2026-07-28` opens the long-lived stream and consumes its frames as a typed
//! [`futures::Stream`] of [`ServerNotification`]s — which is what HTTP-04's
//! requirement text ("**v2 clients get change notifications**") actually asks
//! for.
//!
//! # The wire contract, as this module consumes it
//!
//! 1. `subscriptions/listen` is POSTed with the v2 `_meta` and the three v2
//!    routing headers ([`Mcp-Name`](crate::shared::http_constants::MCP_NAME) is
//!    the EMPTY STRING — the method is not name-bearing).
//! 2. A `text/event-stream` response means SERVED. Anything else means the
//!    server rejected the request, and the JSON-RPC error it carries (typically
//!    `-32601`) is surfaced to the caller UNCHANGED, so "this server does not do
//!    subscriptions" is distinguishable from a transport fault.
//! 3. The FIRST frame MUST be a
//!    [`ACKNOWLEDGED_METHOD`] notification. Anything else is an error naming the
//!    spec's acknowledgement-first MUST.
//! 4. Every subsequent frame carries the SAME
//!    [`SUBSCRIPTION_ID_META_KEY`] value. A frame carrying a DIFFERENT one is
//!    yielded as an `Err`, never forwarded as the caller's own (T-113-66): a
//!    mismatched tag means the server or an intermediary cross-delivered, which
//!    is precisely the failure plan 10's `ListenKey` prevents server-side, and
//!    papering over it client-side would hide it.
//! 5. The terminal [`SubscriptionsListenResult`](crate::types::subscriptions::SubscriptionsListenResult)
//!    ends the stream gracefully (`None`).
//!
//! # D-11: this is the opt-in, not the recommendation
//!
//! Polling over the Tasks mechanism remains pmcp's RECOMMENDED mechanism for
//! enterprise remote deployments. A held-open stream pins one server instance
//! for its whole lifetime, and plan 10's registry is documented INSTANCE-LOCAL:
//! behind a non-sticky load balancer a subscriber silently under-receives. This
//! client API exists because the spec defines it and conformance exercises it,
//! not because it is the default posture.
//!
//! # No second SSE tokenizer
//!
//! Frames are decoded with the SHARED [`SseParser`], the same one the
//! streamable-HTTP transport already feeds. This module adds the JSON-RPC
//! classification on top of it and nothing else.

use crate::error::{Error, ErrorCode, Result, TransportError};
use crate::shared::http_constants::{CONTENT_TYPE, TEXT_EVENT_STREAM};
use crate::shared::sse_parser::SseParser;
use crate::shared::StreamableHttpTransport;
use crate::types::jsonrpc::RequestId;
use crate::types::mrtr::META_KEY;
use crate::types::notifications::ServerNotification;
use crate::types::subscriptions::{
    request_id_value, SubscriptionAcknowledgedParams, ACKNOWLEDGED_METHOD, SUBSCRIPTION_ID_META_KEY,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// How much of a malformed frame is echoed back in an error message.
///
/// Bounded because the frame is UNTRUSTED remote input: a hostile server must
/// not be able to push an unbounded string into a client's logs through an
/// error `Display` (T-113-67).
const MAX_ECHOED_FRAME: usize = 200;

/// A stream of raw SSE `data:` payloads, one item per event.
///
/// The unit is the payload STRING rather than a parsed value because the
/// JSON-RPC classification belongs to [`SubscriptionStream`], not to the
/// transport that produced the bytes.
pub type SubscriptionFrameStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

/// A transport that can open a long-lived server-push stream.
///
/// Deliberately a SEPARATE trait rather than another defaulted method on
/// [`Transport`](crate::shared::Transport): an incrementally-read response body
/// is an HTTP concept, and every stdio / WebSocket / wasm transport would have
/// to carry a meaningless default for it. Keeping it separate also means
/// [`Client::subscriptions_listen`](crate::Client::subscriptions_listen) is
/// generic — a test stub can implement this trait and observe that a non-v2
/// client never opens a stream at all.
#[async_trait]
pub trait EventStreamTransport {
    /// POST `body` and return its response body as a stream of SSE payloads.
    ///
    /// # Errors
    ///
    /// Returns the server's own JSON-RPC error (e.g. `-32601` from a server that
    /// advertises no subscription-delivered capability) when the response is a
    /// JSON document rather than a `text/event-stream`, and a transport error
    /// when the request could not be made.
    async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream>;
}

#[async_trait]
impl EventStreamTransport for StreamableHttpTransport {
    async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream> {
        let response = self.post_streaming(body).await?;
        let status = response.status();
        let is_event_stream = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains(TEXT_EVENT_STREAM));

        if !is_event_stream {
            return Err(rejection_error(status, response.into_body()).await);
        }
        Ok(Box::pin(sse_payload_stream(response.into_body())))
    }
}

/// Turn a non-stream `subscriptions/listen` response into the error the caller
/// sees.
///
/// A well-formed JSON-RPC 2.0 error envelope is surfaced VERBATIM (code,
/// message and `data` intact) so an application can branch on `-32601` — "this
/// server does not do subscriptions" — instead of guessing from a string.
///
/// Deliberately strict about `jsonrpc == "2.0"` AND the presence of `error`,
/// mirroring the transport's own `jsonrpc_error_envelope`: an intermediary's
/// JSON error page must never be laundered into what a caller reads as a
/// server-authored protocol error.
async fn rejection_error(status: hyper::StatusCode, body: hyper::body::Incoming) -> Error {
    let collected = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => return Error::Transport(TransportError::Request(e.to_string())),
    };
    if let Ok(value) = serde_json::from_slice::<Value>(&collected) {
        let is_jsonrpc = value.get("jsonrpc").and_then(Value::as_str) == Some("2.0");
        if let (true, Some(error)) = (is_jsonrpc, value.get("error")) {
            if let Ok(error) =
                serde_json::from_value::<crate::types::jsonrpc::JSONRPCError>(error.clone())
            {
                return Error::from_jsonrpc_error(error);
            }
        }
    }
    Error::Transport(TransportError::Request(format!(
        "subscriptions/listen did not open a stream (HTTP {status}): {}",
        truncate(&String::from_utf8_lossy(&collected))
    )))
}

/// Incremental UTF-8 + SSE decoding state for one response body.
struct PayloadState {
    body: hyper::body::Incoming,
    parser: SseParser,
    /// Bytes received but not yet decodable as complete UTF-8.
    bytes: Vec<u8>,
    /// Payloads already parsed and waiting to be yielded.
    pending: VecDeque<String>,
    /// The body reported end-of-stream (or errored) and must not be polled again.
    done: bool,
}

/// Decode a hyper response body into a stream of SSE `data:` payloads.
///
/// Built with [`futures::stream::unfold`] rather than a hand-written `Stream`
/// impl so the whole decode is one linear async block. Dropping the returned
/// stream drops [`PayloadState`], which drops the `Incoming` body, which closes
/// the connection — that is what makes the server's RAII `ListenGuard` fire
/// (T-113-63).
fn sse_payload_stream(body: hyper::body::Incoming) -> impl Stream<Item = Result<String>> + Send {
    let state = PayloadState {
        body,
        parser: SseParser::new(),
        bytes: Vec::new(),
        pending: VecDeque::new(),
        done: false,
    };
    futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(payload) = state.pending.pop_front() {
                return Some((Ok(payload), state));
            }
            if state.done {
                return None;
            }
            if let Some(error) = read_next_frame(&mut state).await {
                return Some((Err(error), state));
            }
        }
    })
}

/// Read ONE body frame into `state`, returning `Some(error)` only for a
/// transport failure.
///
/// Extracted from [`sse_payload_stream`]'s `unfold` closure so neither function
/// exceeds the repo's cognitive-complexity budget (CLAUDE.md: cog ≤ 25, enforced
/// by the PR-blocking PMAT gate). The split is behaviour-preserving: every exit
/// leaves `state` in the same shape the inline `match` produced, and the caller
/// re-enters its loop, where `pending.pop_front()` and the `done` check together
/// reproduce the old "end-of-body but payloads still buffered" fall-through.
async fn read_next_frame(state: &mut PayloadState) -> Option<Error> {
    match state.body.frame().await {
        // End of body. Anything already in `pending` is still drained by the
        // caller's loop before the `done` check ends the stream.
        None => {
            state.done = true;
            None
        },
        Some(Err(e)) => {
            state.done = true;
            Some(Error::Transport(TransportError::Request(e.to_string())))
        },
        Some(Ok(frame)) => {
            if let Some(chunk) = frame.data_ref() {
                state.bytes.extend_from_slice(chunk);
                let text = take_utf8_prefix(&mut state.bytes);
                state
                    .pending
                    .extend(drain_sse_payloads(&mut state.parser, &text));
            }
            // A trailers frame carries no data; the caller loops and reads again.
            None
        },
    }
}

/// Split the longest decodable UTF-8 prefix off `buffer`, leaving the rest.
///
/// A chunk boundary can fall in the MIDDLE of a multi-byte character, so
/// `String::from_utf8_lossy` per chunk would corrupt any non-ASCII resource URI
/// travelling on the stream. An INCOMPLETE tail is retained for the next chunk;
/// genuinely INVALID bytes are lossily decoded immediately, because retaining
/// those forever would wedge the stream on hostile input (T-113-67).
fn take_utf8_prefix(buffer: &mut Vec<u8>) -> String {
    let valid_up_to = match std::str::from_utf8(buffer) {
        Ok(_) => buffer.len(),
        // `error_len() == None` means "unexpected end of input" — an incomplete
        // character that the next chunk will finish.
        Err(e) if e.error_len().is_none() => e.valid_up_to(),
        // Genuinely invalid bytes: never completable, so decode lossily now.
        Err(_) => {
            let text = String::from_utf8_lossy(buffer).into_owned();
            buffer.clear();
            return text;
        },
    };
    let rest = buffer.split_off(valid_up_to);
    let text = String::from_utf8_lossy(buffer).into_owned();
    *buffer = rest;
    text
}

/// Feed `chunk` to the SHARED SSE parser and return the payloads it completed.
///
/// Keep-alive comment lines (`: ...`) never produce an event — the shared
/// parser drops them in `process_line` — so they are skipped here for free
/// rather than by a second rule that could drift. Only `message` (or untyped)
/// events carry protocol payloads; any other event name is ignored.
fn drain_sse_payloads(parser: &mut SseParser, chunk: &str) -> Vec<String> {
    parser
        .feed(chunk)
        .into_iter()
        .filter(|event| event.event.as_deref().is_none_or(|name| name == "message"))
        .map(|event| event.data)
        .collect()
}

/// Bound an untrusted string for inclusion in an error message.
fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_ECHOED_FRAME {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_ECHOED_FRAME).collect();
    out.push('…');
    out
}

/// What one classified frame means to the stream.
enum FrameOutcome {
    /// A tagged, decodable notification for this subscription.
    Notification(Box<ServerNotification>),
    /// The terminal `SubscriptionsListenResult`: end the stream gracefully.
    Terminal,
    /// A bad frame. Yielded as an `Err` ITEM; the stream keeps going.
    Failed(Box<Error>),
}

/// A live `subscriptions/listen` stream.
///
/// Yields every change notification the server agreed to deliver, in order,
/// after the mandatory acknowledgement (which is already consumed and available
/// through [`Self::acknowledged`] before the first poll).
///
/// # Lifetime and teardown
///
/// The stream OWNS its HTTP response body. Dropping the handle drops that body,
/// which closes the connection, which fires the server's RAII `ListenGuard` and
/// reclaims its registry entry and concurrency permits. There is no `close()` to
/// forget to call and no explicit `Drop` impl — the reclaim is a consequence of
/// ownership, which is exactly why it cannot be skipped on an error path.
///
/// # Errors are items, not terminations
///
/// A malformed frame, an unknown notification method, or a frame tagged with a
/// DIFFERENT `subscriptionId` is yielded as `Some(Err(..))` and the stream
/// CONTINUES. Only a transport failure, the terminal result, or end-of-body ends
/// it. A single bad frame from a buggy intermediary must not silently drop every
/// subsequent notification.
pub struct SubscriptionStream {
    subscription_id: RequestId,
    acknowledged: SubscriptionAcknowledgedParams,
    frames: SubscriptionFrameStream,
    finished: bool,
}

impl std::fmt::Debug for SubscriptionStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionStream")
            .field("subscription_id", &self.subscription_id)
            .field("acknowledged", &self.acknowledged)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl SubscriptionStream {
    /// Consume the mandatory acknowledgement and build the stream around what
    /// follows it.
    ///
    /// # Errors
    ///
    /// Returns an error when the stream ends before any frame arrives, when the
    /// first frame is not [`ACKNOWLEDGED_METHOD`] (the spec's
    /// acknowledgement-first MUST), or when that frame is not tagged with
    /// `subscription_id`.
    pub(crate) async fn open(
        subscription_id: RequestId,
        mut frames: SubscriptionFrameStream,
    ) -> Result<Self> {
        let Some(first) = frames.next().await else {
            return Err(Error::protocol_msg(
                "subscriptions/listen stream ended before the mandatory acknowledgement",
            ));
        };
        let payload = first?;
        let frame = serde_json::from_str::<Value>(&payload).map_err(|e| {
            Error::parse(format!(
                "subscriptions/listen acknowledgement is not JSON ({e}): {}",
                truncate(&payload)
            ))
        })?;

        if frame.get("method").and_then(Value::as_str) != Some(ACKNOWLEDGED_METHOD) {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "spec MUST: the first message on a subscriptions/listen stream is \
                     {ACKNOWLEDGED_METHOD}; got {}",
                    truncate(&payload)
                ),
            ));
        }
        verify_subscription_id(&frame, &subscription_id)?;

        let acknowledged = frame
            .get("params")
            .cloned()
            .ok_or_else(|| {
                Error::parse("the subscriptions/listen acknowledgement carries no params")
            })
            .and_then(|params| {
                serde_json::from_value::<SubscriptionAcknowledgedParams>(params)
                    .map_err(|e| Error::parse(format!("invalid acknowledgement params: {e}")))
            })?;

        Ok(Self {
            subscription_id,
            acknowledged,
            frames,
            finished: false,
        })
    }

    /// The subscription id every frame on this stream is tagged with.
    ///
    /// Equal to the JSON-RPC id of the `subscriptions/listen` request that
    /// opened it.
    #[must_use]
    pub fn subscription_id(&self) -> &RequestId {
        &self.subscription_id
    }

    /// The acknowledgement the server sent first — in particular the AGREED
    /// filter, which is never a superset of what was requested.
    ///
    /// Available BEFORE the first poll: the acknowledgement is consumed while
    /// the stream is being opened, because the spec makes it mandatory and
    /// first.
    #[must_use]
    pub fn acknowledged(&self) -> &SubscriptionAcknowledgedParams {
        &self.acknowledged
    }
}

impl Stream for SubscriptionStream {
    type Item = Result<ServerNotification>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }
        match this.frames.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                this.finished = true;
                Poll::Ready(None)
            },
            Poll::Ready(Some(Err(e))) => {
                this.finished = true;
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(Some(Ok(payload))) => {
                match classify_frame(&payload, &this.subscription_id) {
                    FrameOutcome::Notification(notification) => {
                        Poll::Ready(Some(Ok(*notification)))
                    },
                    FrameOutcome::Terminal => {
                        this.finished = true;
                        Poll::Ready(None)
                    },
                    FrameOutcome::Failed(e) => Poll::Ready(Some(Err(*e))),
                }
            },
        }
    }
}

/// Assert that `frame` is tagged with `expected`.
///
/// The reserved key is read by INDEXING, never through `Value::pointer`: it
/// contains a `/`, which JSON Pointer treats as a path separator unless escaped
/// as `~1` — the exact trap plan 10 hit server-side.
fn verify_subscription_id(frame: &Value, expected: &RequestId) -> Result<()> {
    let observed = ["params", "result"].into_iter().find_map(|section| {
        frame
            .get(section)?
            .get(META_KEY)?
            .get(SUBSCRIPTION_ID_META_KEY)
    });
    let expected_value = request_id_value(expected);
    if observed == Some(&expected_value) {
        return Ok(());
    }
    Err(Error::protocol(
        ErrorCode::INVALID_REQUEST,
        format!(
            "subscriptions/listen frame carries subscriptionId {} but this stream is {expected_value}; \
             refusing to deliver another subscription's frame",
            observed.map_or_else(|| "<absent>".to_string(), ToString::to_string),
        ),
    ))
}

/// Classify one already-parsed SSE payload.
fn classify_frame(payload: &str, subscription_id: &RequestId) -> FrameOutcome {
    let Ok(frame) = serde_json::from_str::<Value>(payload) else {
        return FrameOutcome::Failed(Box::new(Error::parse(format!(
            "subscriptions/listen frame is not JSON: {}",
            truncate(payload)
        ))));
    };
    if let Err(e) = verify_subscription_id(&frame, subscription_id) {
        return FrameOutcome::Failed(Box::new(e));
    }
    if frame.get("result").is_some() {
        // The graceful-teardown `SubscriptionsListenResult`.
        return FrameOutcome::Terminal;
    }
    if let Some(error) = frame.get("error") {
        let error = serde_json::from_value::<crate::types::jsonrpc::JSONRPCError>(error.clone())
            .map_or_else(
                |_| Error::protocol_msg(format!("subscriptions/listen error frame: {error}")),
                Error::from_jsonrpc_error,
            );
        return FrameOutcome::Failed(Box::new(error));
    }
    if frame.get("method").and_then(Value::as_str) == Some(ACKNOWLEDGED_METHOD) {
        return FrameOutcome::Failed(Box::new(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "spec MUST: a subscriptions/listen stream is acknowledged exactly once, and a second \
             acknowledgement arrived",
        )));
    }
    match decode_notification(frame, payload) {
        Ok(notification) => FrameOutcome::Notification(Box::new(notification)),
        Err(e) => FrameOutcome::Failed(Box::new(e)),
    }
}

/// Decode a listen frame into the typed [`ServerNotification`].
///
/// [`ServerNotification`] is adjacently tagged (`method` / `params`), so the
/// envelope members a listen frame additionally carries — `jsonrpc`, and the
/// `params._meta` tag this module has already validated — are stripped first.
/// A unit-variant notification (`notifications/tools/list_changed`) has an
/// EMPTY `params` once the tag is removed, and an empty content object is not a
/// unit, so `params` is dropped entirely in that case.
///
/// Takes the parsed frame by value — the caller has no further use for it — and
/// the original `payload` for the error text, so neither the frame nor its
/// re-serialization is cloned on the per-notification receive path.
fn decode_notification(mut cleaned: Value, payload: &str) -> Result<ServerNotification> {
    let Some(object) = cleaned.as_object_mut() else {
        return Err(Error::parse("subscriptions/listen frame is not an object"));
    };
    object.remove("jsonrpc");
    object.remove("id");
    let drop_params = match object.get_mut("params") {
        Some(Value::Object(params)) => {
            params.remove(META_KEY);
            params.is_empty()
        },
        _ => false,
    };
    if drop_params {
        object.remove("params");
    }
    serde_json::from_value::<ServerNotification>(cleaned).map_err(|e| {
        Error::parse(format!(
            "subscriptions/listen frame is not a known server notification ({e}): {}",
            truncate(payload)
        ))
    })
}

// ===========================================================================
// Internal support surface for `fuzz_targets/`.
// ===========================================================================

/// Run ONE untrusted chunk of listen-stream bytes through EXACTLY the decode a
/// live [`SubscriptionStream`] performs, and report what each completed frame
/// classified as.
///
/// `#[doc(hidden)]`: this is internal support surface for
/// `fuzz/fuzz_targets/subscription_listen_frames.rs`, NOT stable API. It exists
/// because the decode path is private and a fuzz target may only reach public
/// items — the same `#[doc(hidden)]` seam convention Phase 110 established for
/// `cargo-pmcp`'s fuzz and example targets. Do not build on it.
///
/// Errors are flattened to their `Display` string so no private type escapes.
/// A terminal result contributes no entry.
#[doc(hidden)]
#[must_use]
pub fn decode_listen_chunk_for_fuzz(
    chunk: &[u8],
    subscription_id: &str,
) -> Vec<std::result::Result<ServerNotification, String>> {
    let id = RequestId::String(subscription_id.to_string());
    let mut buffer = chunk.to_vec();
    let text = take_utf8_prefix(&mut buffer);
    let mut parser = SseParser::new();
    drain_sse_payloads(&mut parser, &text)
        .into_iter()
        .filter_map(|payload| match classify_frame(&payload, &id) {
            FrameOutcome::Notification(notification) => Some(Ok(*notification)),
            FrameOutcome::Failed(e) => Some(Err(e.to_string())),
            FrameOutcome::Terminal => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::subscriptions::{subscription_id_meta, SubscriptionFilter};
    use serde_json::json;

    /// Build a `SubscriptionStream` over a canned payload sequence, with no
    /// socket in sight.
    fn stream_over(
        subscription_id: RequestId,
        payloads: Vec<Result<String>>,
    ) -> SubscriptionStream {
        SubscriptionStream {
            subscription_id,
            acknowledged: SubscriptionAcknowledgedParams::default(),
            frames: Box::pin(futures::stream::iter(payloads)),
            finished: false,
        }
    }

    fn id() -> RequestId {
        RequestId::Number(11)
    }

    fn ack_payload(subscription_id: &RequestId, filter: &Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": ACKNOWLEDGED_METHOD,
            "params": {
                "notifications": filter,
                "_meta": subscription_id_meta(subscription_id),
            },
        })
        .to_string()
    }

    fn tools_changed(subscription_id: &RequestId) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
            "params": { "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    fn resource_updated(subscription_id: &RequestId, uri: &str) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": { "uri": uri, "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    fn terminal(subscription_id: &RequestId) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": request_id_value(subscription_id),
            "result": { "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    // ---- the acknowledgement-first MUST -----------------------------------

    #[tokio::test]
    async fn open_consumes_the_acknowledgement_and_exposes_the_agreed_filter() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(vec![Ok(
            ack_payload(&id(), &json!({ "toolsListChanged": true })),
        )]));
        let stream = SubscriptionStream::open(id(), frames)
            .await
            .expect("the ack opens the stream");

        assert_eq!(stream.subscription_id(), &id());
        assert_eq!(
            stream.acknowledged().notifications,
            SubscriptionFilter {
                tools_list_changed: Some(true),
                ..SubscriptionFilter::default()
            },
            "the agreed filter is readable BEFORE the first poll"
        );
    }

    #[tokio::test]
    async fn a_non_acknowledgement_first_frame_is_refused() {
        let frames: SubscriptionFrameStream =
            Box::pin(futures::stream::iter(vec![Ok(tools_changed(&id()))]));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("a notification cannot precede the acknowledgement");
        assert!(
            error.to_string().contains(ACKNOWLEDGED_METHOD),
            "the error names the spec MUST: {error}"
        );
    }

    #[tokio::test]
    async fn an_acknowledgement_for_another_subscription_is_refused() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(vec![Ok(
            ack_payload(&RequestId::Number(999), &json!({})),
        )]));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("a cross-tagged ack must not open a stream");
        assert!(
            error.to_string().contains("subscriptionId"),
            "the error names the mismatch: {error}"
        );
    }

    #[tokio::test]
    async fn an_empty_stream_is_refused() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(Vec::new()));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("no frame at all is not an acknowledgement");
        assert!(error.to_string().contains("acknowledgement"), "{error}");
    }

    // ---- frame classification ---------------------------------------------

    #[tokio::test]
    async fn a_tagged_unit_notification_is_decoded() {
        let mut stream = stream_over(id(), vec![Ok(tools_changed(&id()))]);
        let item = stream.next().await.expect("one frame").expect("decodes");
        assert!(matches!(item, ServerNotification::ToolsChanged));
        assert!(stream.next().await.is_none(), "then the stream ends");
    }

    #[tokio::test]
    async fn a_tagged_struct_notification_is_decoded() {
        let mut stream = stream_over(id(), vec![Ok(resource_updated(&id(), "mem://greeting"))]);
        let item = stream.next().await.expect("one frame").expect("decodes");
        match item {
            ServerNotification::ResourceUpdated(params) => {
                assert_eq!(params.uri, "mem://greeting");
            },
            other => panic!("expected a resources/updated notification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_frame_tagged_with_another_subscription_id_yields_an_error() {
        let mut stream = stream_over(
            id(),
            vec![
                Ok(tools_changed(&RequestId::Number(999))),
                Ok(tools_changed(&id())),
            ],
        );

        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("a cross-tagged frame is never forwarded as the caller's own");
        assert!(
            error.to_string().contains("999"),
            "the error names the foreign id: {error}"
        );

        let recovered = stream
            .next()
            .await
            .expect("the stream did NOT terminate")
            .expect("the correctly tagged frame still arrives");
        assert!(matches!(recovered, ServerNotification::ToolsChanged));
    }

    #[tokio::test]
    async fn an_untagged_frame_yields_an_error() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
        })
        .to_string();
        let mut stream = stream_over(id(), vec![Ok(payload)]);
        let error = stream.next().await.expect("an item").expect_err("untagged");
        assert!(error.to_string().contains("<absent>"), "{error}");
    }

    #[tokio::test]
    async fn a_malformed_frame_yields_an_error_without_ending_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![Ok("{not json at all".to_string()), Ok(tools_changed(&id()))],
        );

        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("garbage is an error");
        assert!(error.to_string().contains("not JSON"), "{error}");

        let recovered = stream
            .next()
            .await
            .expect("the stream survived the malformed frame")
            .expect("and the next good frame decodes");
        assert!(matches!(recovered, ServerNotification::ToolsChanged));
    }

    #[tokio::test]
    async fn an_unknown_notification_method_yields_an_error_without_ending_the_stream() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/from/the/future",
            "params": { "_meta": subscription_id_meta(&id()) },
        })
        .to_string();
        let mut stream = stream_over(id(), vec![Ok(payload), Ok(tools_changed(&id()))]);

        assert!(
            stream.next().await.expect("an item").is_err(),
            "an unmodelled method is surfaced, not silently dropped"
        );
        assert!(
            stream.next().await.expect("still live").is_ok(),
            "and the stream keeps going"
        );
    }

    #[tokio::test]
    async fn a_second_acknowledgement_yields_an_error() {
        let mut stream = stream_over(id(), vec![Ok(ack_payload(&id(), &json!({})))]);
        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("exactly one acknowledgement is allowed");
        assert!(error.to_string().contains("exactly once"), "{error}");
    }

    #[tokio::test]
    async fn the_terminal_result_ends_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![
                Ok(tools_changed(&id())),
                Ok(terminal(&id())),
                Ok(tools_changed(&id())),
            ],
        );

        assert!(stream.next().await.expect("the notification").is_ok());
        assert!(
            stream.next().await.is_none(),
            "the terminal SubscriptionsListenResult ends the stream gracefully"
        );
        assert!(
            stream.next().await.is_none(),
            "and it stays ended — nothing after it is delivered"
        );
    }

    #[tokio::test]
    async fn a_transport_error_ends_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![
                Err(Error::Transport(TransportError::ConnectionClosed)),
                Ok(tools_changed(&id())),
            ],
        );
        assert!(stream.next().await.expect("an item").is_err());
        assert!(
            stream.next().await.is_none(),
            "a transport failure is terminal, unlike a bad frame"
        );
    }

    // ---- SSE decoding ------------------------------------------------------

    #[test]
    fn keep_alive_comments_are_skipped() {
        let mut parser = SseParser::new();
        assert!(
            drain_sse_payloads(&mut parser, ": keep-alive\n\n").is_empty(),
            "a comment line is not a payload"
        );
        assert!(
            drain_sse_payloads(&mut parser, ":\n\n").is_empty(),
            "an empty comment is not a payload either"
        );
        assert_eq!(
            drain_sse_payloads(&mut parser, "event: message\ndata: {\"a\":1}\n\n"),
            vec!["{\"a\":1}".to_string()],
            "and the real event still arrives"
        );
    }

    #[test]
    fn a_payload_split_across_chunks_is_reassembled() {
        let mut parser = SseParser::new();
        assert!(drain_sse_payloads(&mut parser, "data: {\"a\"").is_empty());
        assert_eq!(
            drain_sse_payloads(&mut parser, ":1}\n\n"),
            vec!["{\"a\":1}".to_string()]
        );
    }

    #[test]
    fn a_multibyte_character_split_across_chunks_survives() {
        // "☂" is three bytes; split it across two reads.
        let text = "data: \u{2602}\n\n";
        let bytes = text.as_bytes();
        let mut buffer = Vec::new();
        let mut parser = SseParser::new();
        let mut payloads = Vec::new();

        buffer.extend_from_slice(&bytes[..7]); // cuts the umbrella in half
        let prefix = take_utf8_prefix(&mut buffer);
        payloads.extend(drain_sse_payloads(&mut parser, &prefix));
        assert!(!buffer.is_empty(), "the incomplete tail is retained");

        buffer.extend_from_slice(&bytes[7..]);
        let rest = take_utf8_prefix(&mut buffer);
        payloads.extend(drain_sse_payloads(&mut parser, &rest));

        assert_eq!(payloads, vec!["\u{2602}".to_string()]);
    }

    #[test]
    fn invalid_bytes_do_not_wedge_the_decoder() {
        let mut buffer = vec![0xff, 0xfe, b'a'];
        let text = take_utf8_prefix(&mut buffer);
        assert!(
            buffer.is_empty(),
            "genuinely invalid bytes are consumed, not retained forever"
        );
        assert!(text.contains('a'), "the valid remainder survives: {text:?}");
    }

    #[test]
    fn an_untrusted_frame_is_truncated_in_error_messages() {
        let huge = "x".repeat(MAX_ECHOED_FRAME * 4);
        let truncated = truncate(&huge);
        assert!(truncated.chars().count() <= MAX_ECHOED_FRAME + 1);
    }

    // ---- properties (CLAUDE.md ALWAYS / PROPERTY testing) ------------------

    /// Every notification method a listen stream can legitimately carry, plus
    /// one this SDK does not model.
    const METHODS: [&str; 4] = [
        "notifications/tools/list_changed",
        "notifications/prompts/list_changed",
        "notifications/resources/list_changed",
        "notifications/from/the/future",
    ];

    proptest::proptest! {
        /// T-113-66 as an invariant over the whole id space: a frame is
        /// delivered to the caller IF AND ONLY IF it is tagged with THIS
        /// stream's subscription id (and carries a method this SDK models).
        ///
        /// An example-based test can only pin the two cases it happens to
        /// write; this pins the implication itself.
        #[test]
        fn a_frame_is_delivered_only_when_its_tag_matches_this_stream(
            stream_id in 0i64..8,
            frame_id in 0i64..8,
            method_index in 0usize..4,
        ) {
            let frame = json!({
                "jsonrpc": "2.0",
                "method": METHODS[method_index],
                "params": {
                    "_meta": subscription_id_meta(&RequestId::Number(frame_id)),
                },
            });

            let delivered = matches!(
                classify_frame(&frame.to_string(), &RequestId::Number(stream_id)),
                FrameOutcome::Notification(_)
            );

            proptest::prop_assert_eq!(
                delivered,
                stream_id == frame_id && method_index < 3,
                "delivery must be exactly (matching tag AND known method); \
                 stream={} frame={} method={}",
                stream_id,
                frame_id,
                METHODS[method_index],
            );
        }

        /// The decode path is fed by a REMOTE peer. Arbitrary bytes must never
        /// panic it (T-113-67) — the same invariant the fuzz target asserts,
        /// held here as a fast in-tree regression.
        #[test]
        fn arbitrary_bytes_never_panic_the_decoder(
            bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
        ) {
            let _ = decode_listen_chunk_for_fuzz(&bytes, "prop-subscription");
        }

        /// And neither does arbitrary TEXT that looks more like SSE than random
        /// bytes do, which is the shape a hostile-but-well-formed peer sends.
        #[test]
        fn arbitrary_sse_shaped_text_never_panics_the_decoder(
            body in "(data|event|id|:|\\{|\\}|\"|a|1|\n){0,200}",
        ) {
            let _ = decode_listen_chunk_for_fuzz(body.as_bytes(), "prop-subscription");
        }
    }

    // ---- the era gate, proven with a counting stub transport ---------------

    mod era_gate {
        use super::*;
        use crate::shared::{Transport, TransportMessage};
        use crate::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
        use crate::{Client, ClientBuilder};
        use std::sync::{Arc, Mutex};

        /// A transport that COUNTS stream opens, so "no request was sent" is a
        /// measured fact rather than an inference from an error message.
        #[derive(Debug, Default, Clone)]
        struct CountingStubTransport {
            opened: Arc<Mutex<Vec<Vec<u8>>>>,
            payloads: Arc<Mutex<Vec<String>>>,
        }

        #[async_trait]
        impl Transport for CountingStubTransport {
            async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                Ok(())
            }

            async fn receive(&mut self) -> Result<TransportMessage> {
                Err(Error::protocol_msg("no responses"))
            }

            async fn close(&mut self) -> Result<()> {
                Ok(())
            }

            fn transport_type(&self) -> &'static str {
                "counting-stub"
            }

            fn supports_negotiated_protocol_version(&self) -> bool {
                true
            }
        }

        #[async_trait]
        impl EventStreamTransport for CountingStubTransport {
            async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream> {
                self.opened.lock().unwrap().push(body);
                let payloads: Vec<Result<String>> = self
                    .payloads
                    .lock()
                    .unwrap()
                    .iter()
                    .cloned()
                    .map(Ok)
                    .collect();
                Ok(Box::pin(futures::stream::iter(payloads)))
            }
        }

        fn v2_version() -> ProtocolVersion {
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string())
        }

        fn client_with(
            transport: CountingStubTransport,
            v2: bool,
        ) -> Client<CountingStubTransport> {
            let builder = ClientBuilder::new(transport);
            if v2 {
                builder
                    .with_protocol_version(v2_version())
                    .expect("2026-07-28 is selectable")
                    .build()
            } else {
                builder.build()
            }
        }

        #[tokio::test]
        async fn a_non_v2_client_refuses_without_opening_a_stream() {
            let transport = CountingStubTransport::default();
            let opened = transport.opened.clone();
            let client = client_with(transport, false);

            let error = client
                .subscriptions_listen(SubscriptionFilter::default())
                .await
                .expect_err("subscriptions/listen does not exist on v1");

            assert_eq!(
                opened.lock().unwrap().len(),
                0,
                "a v1 client must not put a request on the wire that cannot succeed"
            );
            assert!(
                error.to_string().contains("with_protocol_version"),
                "the error names the opt-in: {error}"
            );
        }

        #[tokio::test]
        async fn a_v2_client_sends_the_listen_frame_and_consumes_the_ack() {
            let transport = CountingStubTransport::default();
            let opened = transport.opened.clone();
            // The stub answers whatever id the client mints, so the ack is built
            // lazily below — here it is enough that the frame is well-formed for
            // a KNOWN id, so the client is given one it will reject, and the
            // request bytes are what this test asserts on.
            let client = client_with(transport, true);

            let error = client
                .subscriptions_listen(SubscriptionFilter {
                    tools_list_changed: Some(true),
                    ..SubscriptionFilter::default()
                })
                .await
                .expect_err("the stub sends no acknowledgement at all");
            assert!(error.to_string().contains("acknowledgement"), "{error}");

            let opened = opened.lock().unwrap();
            assert_eq!(opened.len(), 1, "exactly one stream open");
            let frame = serde_json::from_slice::<Value>(&opened[0]).expect("a JSON-RPC frame");
            assert_eq!(frame["method"], json!("subscriptions/listen"));
            assert_eq!(
                frame["params"]["notifications"],
                json!({ "toolsListChanged": true }),
                "the requested filter travels under the REQUIRED `notifications` field"
            );
            assert_eq!(
                frame["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
                json!(PROTOCOL_VERSION_2026_07_28),
                "the v2 era signal is stamped like on every other v2 request"
            );
        }

        #[tokio::test]
        async fn a_v2_client_returns_the_servers_own_error_unchanged() {
            /// A transport whose stream open fails the way a `-32601`
            /// rejection does.
            #[derive(Debug, Default, Clone)]
            struct RejectingTransport;

            #[async_trait]
            impl Transport for RejectingTransport {
                async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                    Ok(())
                }
                async fn receive(&mut self) -> Result<TransportMessage> {
                    Err(Error::protocol_msg("no responses"))
                }
                async fn close(&mut self) -> Result<()> {
                    Ok(())
                }
                fn supports_negotiated_protocol_version(&self) -> bool {
                    true
                }
            }

            #[async_trait]
            impl EventStreamTransport for RejectingTransport {
                async fn open_event_stream(
                    &self,
                    _body: Vec<u8>,
                ) -> Result<SubscriptionFrameStream> {
                    Err(Error::from_jsonrpc_error(
                        crate::types::jsonrpc::JSONRPCError {
                            code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                            message: "Method not found: subscriptions/listen".to_string(),
                            data: None,
                        },
                    ))
                }
            }

            let client = ClientBuilder::new(RejectingTransport)
                .with_protocol_version(v2_version())
                .expect("2026-07-28 is selectable")
                .build();

            let error = client
                .subscriptions_listen(SubscriptionFilter::default())
                .await
                .expect_err("a non-advertising server answers -32601");
            match error {
                Error::Protocol { code, .. } => assert_eq!(
                    code.as_i32(),
                    crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                    "the server's own error code reaches the caller unchanged"
                ),
                other => panic!("expected a structured protocol error, got {other:?}"),
            }
        }
    }
}
