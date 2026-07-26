---
phase: 113-stateless-http-multi-round-trip-elicitation
reviewed: 2026-07-26T04:25:38Z
depth: standard
files_reviewed: 20
files_reviewed_list:
  - src/client/mod.rs
  - src/client/subscriptions.rs
  - src/error/mod.rs
  - src/server/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/subscriptions.rs
  - src/shared/sse_parser.rs
  - src/shared/streamable_http.rs
  - src/types/mod.rs
  - src/types/mrtr.rs
  - src/types/subscriptions.rs
  - examples/s47_v2_stateless_mrtr.rs
  - examples/s48_v2_mrtr_client.rs
  - examples/s49_v2_subscriptions_client.rs
  - fuzz/fuzz_targets/subscription_listen_frames.rs
  - tests/v2_mrtr.rs
  - tests/v2_subscriptions.rs
  - tests/v2_subscriptions_client.rs
  - Cargo.toml
  - fuzz/Cargo.toml
findings:
  critical: 3
  warning: 9
  info: 7
  total: 19
status: issues_found
---

# Phase 113: Code Review Report

**Reviewed:** 2026-07-26T04:25:38Z
**Depth:** standard
**Files Reviewed:** 20
**Status:** issues_found

## Summary

Scope was the delta `a721ede0..HEAD` — waves 5–7 (plans 113-10, 113-11, 113-12,
113-13): the server-side `subscriptions/listen` registry and route, the client-side
`SubscriptionStream`, the era gate for the two retired `resources/*` RPCs, the
SSE-parser char-boundary fix, three examples, one fuzz target and ~3.4 kLOC of tests.

The era gating is sound. `advertises_subscriptions` is genuinely a single predicate
read by both the `server/discover` projection and the listen route; the retirement
of `resources/subscribe` / `resources/unsubscribe` is enforced on both POST
entrypoints via one seam (`dispatch_request_or_retire`) and mirrored client-side
before any byte reaches the wire; the v1 paths are untouched. The SSE byte-index fix
is correct and complete — the remaining `find(':')`/`drain(..=line_end)` indexing all
lands on ASCII delimiters, so no other mixed byte/char indexing survives.

The concurrency and lifetime story in `ListenRegistry` does not hold up. The module's
own headline claim — that `ListenKey { principal, request_id }` makes id reuse safe —
is only true ACROSS principals. WITHIN a principal the registry blind-overwrites and
the RAII guard blind-removes, so two connections belonging to the same authenticated
subject that reuse one JSON-RPC id silently destroy each other's streams. The overflow
policy removes the map entry but does not release the concurrency permits it is
accounted against, so `live_streams()` and the semaphores drift apart. And the shared
SSE parser — now on a long-lived, remote-fed stream — still has no buffer bound at all,
while the `SseConfig::max_buffer_size` field that documents one is dead code.

Secondary theme: several "structural guarantee" claims in the doc comments are stronger
than the code. The reserved overflow slot is not reserved under concurrent fan-out, the
acknowledged filter is never validated client-side, the graceful-shutdown trigger has
zero callers anywhere in the repo, and the build-time load-balancer WARN fires on
v1-only servers that can never serve the route.

## Critical Issues

### CR-01: Same-principal JSON-RPC id reuse silently destroys both listen streams

**File:** `src/server/subscriptions.rs:474-481` (with `410-418`)

**Issue:** `ListenRegistry::register` inserts into a `HashMap` with no occupancy check:

```rust
self.entries.write().insert(
    key.clone(),
    ListenEntry { sender, filter, terminal },
);
```

`HashMap::insert` REPLACES on a duplicate key and drops the displaced `ListenEntry` —
including its `mpsc::Sender`. Dropping that sender ends the first subscriber's stream
immediately, with no terminal result and no overflow notice: the client just sees EOF.

It gets worse. When the first stream's future finally unwinds, `ListenGuard::drop`
(line 410-418) runs `self.registry.remove_entry(&self.key)` unconditionally — and the
entry now at that key is the SECOND subscriber's. So the replacement stream is killed
too. Two well-behaved callers, both under every cap, and BOTH streams die.

The module doc at lines 294-300 asserts this class of collision is fixed:

> Keying on the request id ALONE cross-delivers between callers: different principals
> and **different connections** routinely reuse ids such as `1` […] The pair is the fix

Only the "different principals" half is fixed. `two_callers_same_request_id_do_not_cross`
(line 855) and `tests/v2_subscriptions.rs:662` both use DIFFERENT principals, so the
same-principal case is untested. `pmcp`'s own client happens to escape it because
`Client::subscriptions_listen` mints `RequestId::String(Uuid::new_v4())`, but every
other MCP SDK uses small integer ids, and one user legitimately runs several clients.

Impact escalates when `AuthContext::subject` is empty or constant (a token with no
`sub` claim, a shared service account): every caller then collapses onto ONE principal,
so any client can terminate any other client's subscription by picking their id — and
`MAX_LISTEN_STREAMS_PER_PRINCIPAL = 4` becomes a server-wide cap. Note that
`anonymous_principal()` is only used when `auth_context` is `None`, so an empty subject
is NOT routed to the anonymous fallback.

**Fix:** Reject a duplicate key instead of overwriting, and make the guard remove only
its own entry. A per-entry generation token is the smallest change that closes both
halves:

```rust
// In ListenEntry:
generation: u64,

// In ListenRegistry:
next_generation: std::sync::atomic::AtomicU64,

pub(crate) fn register(...) -> std::result::Result<ListenGuard, ListenRejection> {
    // ... permits as today ...
    let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
    {
        let mut entries = self.entries.write();
        // A duplicate (principal, id) is the CALLER's error, not a licence to
        // evict a live stream.
        if entries.contains_key(&key) {
            return Err(ListenRejection::DuplicateSubscriptionId);
        }
        entries.insert(key.clone(), ListenEntry { sender, filter, terminal, generation });
    }
    Ok(ListenGuard { key, generation, registry: Arc::clone(self), .. })
}

fn remove_entry(&self, key: &ListenKey, generation: u64) {
    let mut entries = self.entries.write();
    // Only MY entry — a successor at the same key must survive.
    if entries.get(key).is_some_and(|e| e.generation == generation) {
        entries.remove(key);
    }
}
```

`ListenRejection::DuplicateSubscriptionId` should answer `-32600 INVALID_REQUEST`
("a subscriptions/listen stream is already open for this id"), which is honest and
actionable. Add a regression test that opens two streams for ONE principal with the
same id and asserts the first still receives a fan-out.

### CR-02: `ListenGuard::drop` reclaims an entry it does not own after an overflow-disconnect

**File:** `src/server/subscriptions.rs:410-418`, `562-576`

**Issue:** This is reachable independently of CR-01's overwrite. `disconnect_overflowed`
removes the entry from the map but does NOT drop the guard (the guard lives in the SSE
stream future and is only dropped when that future unwinds). During the window between
the two, a new `register` for the same `(principal, request_id)` succeeds — the map slot
is free and the per-principal semaphore has a spare permit only if the overflowed stream
has already unwound, but the GLOBAL permit path can still admit it. When the old guard
finally drops:

```rust
impl Drop for ListenGuard {
    fn drop(&mut self) {
        self.registry.remove_entry(&self.key);   // <- removes the SUCCESSOR
        ...
    }
}
```

the successor's `ListenEntry` (and its sender) is destroyed, ending a healthy stream
with no terminal frame. Since the overflow policy explicitly tells the client to
"re-issue `subscriptions/listen`" (`LISTEN_OVERFLOW_NOTICE`, line 282), a compliant
client that reuses its id is walking directly into this.

**Fix:** The generation-token `remove_entry` in CR-01 closes this too. Do not fix CR-01
without also fixing this, or a duplicate-id rejection will just move the failure.

### CR-03: `SseParser` accumulates unbounded remote bytes; `SseConfig::max_buffer_size` is dead code

**File:** `src/shared/sse_parser.rs:169-205` (buffer), `src/shared/sse_parser.rs:372-398`
(dead config)

**Issue:** `feed` does `self.buffer.push_str(data)` and only ever drains up to a `\n`.
A peer that streams bytes containing no newline — or a single event whose payload is
arbitrarily large — grows `buffer` without limit until the process is OOM-killed. On a
`subscriptions/listen` stream that is the DESIGNED steady state: the connection is
long-lived and every byte comes from the remote server.

`SseConfig` declares the bound and never enforces it:

```rust
/// Maximum buffer size for incomplete lines
pub max_buffer_size: usize,   // set to 1 MiB in Default, referenced NOWHERE else
```

`grep -rn max_buffer_size src/` returns exactly two hits, both in the struct definition
and its `Default`. Nothing reads it.

This is in scope because this phase (a) added a second, long-lived untrusted consumer of
this parser (`client::subscriptions::drain_sse_payloads`), and (b) shipped
`feed_never_panics_on_arbitrary_text` and a fuzz target whose stated invariant is
"a hostile or merely broken frame must not take down a client". Unbounded growth takes
the client down; it just does it with an allocator abort instead of a panic. Note the
client-side `PayloadState::bytes` IS bounded (at most 3 trailing bytes), so the parser
buffer is the only unbounded accumulator on the path.

**Fix:** Enforce the documented cap inside `feed`, and give the parser a way to report
the overrun rather than silently truncating:

```rust
pub struct SseParser {
    buffer: String,
    max_buffer_size: usize,   // from SseConfig; default 1 MiB
    overflowed: bool,
    ...
}

pub fn feed(&mut self, data: &str) -> Vec<SseEvent> {
    if self.overflowed {
        return Vec::new();
    }
    if self.buffer.len().saturating_add(data.len()) > self.max_buffer_size {
        tracing::warn!(
            target: "mcp.sse",
            limit = self.max_buffer_size,
            "SSE line exceeded the buffer bound; discarding the parser state"
        );
        self.overflowed = true;
        self.buffer.clear();
        self.current_event = EventBuilder::new();
        return Vec::new();
    }
    self.buffer.push_str(data);
    // ... existing loop ...
}
```

Then surface `overflowed` to `sse_payload_stream` so the subscription stream ends with a
transport error instead of going quiet, and add a property test asserting
`parser.buffer.len() <= max_buffer_size` for arbitrary chunk sequences.

## Warnings

### WR-01: Overflow-disconnect drops the registry entry but leaks both concurrency permits

**File:** `src/server/subscriptions.rs:562-576`

**Issue:** `disconnect_overflowed` removes the entry from `entries` but the
`OwnedSemaphorePermit`s live in `ListenGuard`, which is only dropped when the SSE stream
future unwinds. That future only unwinds after the receiver drains the buffered frames —
and the subscriber overflowed precisely because it stopped reading, so under TCP
backpressure the frames sit there and the guard is held until the socket times out.

The observable result is that `live_streams()` (and therefore `Debug for ListenRegistry`)
reports 0 while all `MAX_LISTEN_STREAMS_TOTAL` global permits are still held, and every
new `subscriptions/listen` is refused with "too many concurrent subscriptions/listen
streams on this server". Operators reading the count will conclude the cap is broken.

**Fix:** Make permit release part of the disconnect, not only of the guard. Move the
permits into `ListenEntry` (the guard then holds only the key + registry handle for the
disconnect-by-client path), so `disconnect_overflowed`'s `remove` releases them together
with the sender:

```rust
struct ListenEntry {
    sender: tokio::sync::mpsc::Sender<ListenFrame>,
    filter: SubscriptionFilter,
    terminal: String,
    _principal_permit: tokio::sync::OwnedSemaphorePermit,
    _global_permit: tokio::sync::OwnedSemaphorePermit,
}
```

and assert the invariant in a test: after `disconnect_overflowed`, `live_streams() == 0`
AND a fresh `register` for a new principal succeeds.

### WR-02: The "reserved" overflow slot is not reserved under concurrent fan-out

**File:** `src/server/subscriptions.rs:259-268`, `527-545`

**Issue:** The documented invariant is that the last channel slot is reserved so an
overflowed subscriber always receives `LISTEN_OVERFLOW_NOTICE`. The reservation is
implemented as a check-then-send under a READ lock:

```rust
if entry.sender.capacity() <= 1 { overflowed.push(key.clone()); continue; }
// ... other threads can be here at the same time ...
entry.sender.try_send(ListenFrame::Message(frame.to_string()))
```

`Server::send_notification` takes `&self`, so two tasks holding the server through
different `Arc`s can be inside `fan_out` concurrently. Both can observe `capacity() == 2`
and both send, leaving capacity 0. The subsequent `disconnect_overflowed` then does
`let _ = entry.sender.try_send(Comment(...))` — which fails, is discarded, and the
subscriber's stream simply ends with no explanation. The inline comment at line 538-540
("`Full` cannot happen: the capacity check above ran under the same read lock") is
incorrect: a read lock does not exclude other readers.

**Fix:** Either reserve the slot with a real permit
(`sender.reserve_owned()` held in `ListenEntry` and consumed by `disconnect_overflowed`),
or downgrade the doc claim to "best effort" and remove the "cannot happen" comment. The
permit approach is preferable since the notice is the only signal the client gets.

### WR-03: The build-time instance-local WARN fires on v1-only servers that can never serve the route

**File:** `src/server/mod.rs:4750-4761`

**Issue:** The warning is gated only on capability advertisement:

```rust
if crate::types::subscriptions::advertises_subscriptions(&self.capabilities) {
    tracing::warn!(target: "mcp.subscriptions",
        "a subscription-delivered capability is advertised, so subscriptions/listen \
         will be SERVED; its registry is INSTANCE-LOCAL, ...");
}
```

`assemble_subscriptions_listen` returns `-32601` unless `era == Era::V2`, and the era can
only be V2 if the server opted in via `with_supported_protocol_versions`. The default
accept list is v1-only (`src/types/protocol/context.rs:30-31`: "falling back to the
v1-only `default_accept_list`"). So every EXISTING pmcp server that advertises
`tools.listChanged` — which is the common case — now emits an alarming, load-balancer-
flavoured WARN at startup that is factually false for it ("will be SERVED" — it will not).

**Fix:** Add the era condition that the route itself enforces:

```rust
if crate::types::protocol::context::is_v2_opted_in(&self.supported_protocol_versions)
    && crate::types::subscriptions::advertises_subscriptions(&self.capabilities)
{
    tracing::warn!(...);
}
```

`is_v2_opted_in` already exists and is used at `src/server/mod.rs:1417`.

### WR-04: Concurrency and buffering limits are hardcoded with no configuration surface

**File:** `src/server/subscriptions.rs:268`, `271`, `278`;
`src/server/streamable_http_server.rs:2547`

**Issue:** `LISTEN_CHANNEL_CAPACITY = 64`, `MAX_LISTEN_STREAMS_PER_PRINCIPAL = 4`,
`MAX_LISTEN_STREAMS_TOTAL = 64` and `LISTEN_KEEP_ALIVE_INTERVAL = 15s` are all
`pub(crate) const` with no builder or config knob (`ListenRegistry::with_limits` is
private and used only by unit tests). A deployment with more than 64 legitimate
subscribers cannot serve them, and one behind a proxy with a 10 s idle timeout cannot
lower the keep-alive interval. Combined with `anonymous_principal()`'s per-stream
counter (documented at lines 345-358), an unauthenticated deployment's ONLY bound is
this fixed 64, exhaustible by a single client — and cheaply, since
`resolve_agreed_filter` happily serves an EMPTY agreed filter, so
`{"notifications":{}}` × 64 occupies every slot while receiving nothing.

**Fix:** Thread the three limits (and the keep-alive interval) through
`StreamableHttpServerConfig` / `ServerBuilder`, defaulting to today's values, and pass
them into `ListenRegistry::with_limits`. Document the unauthenticated exhaustion vector
in the `subscriptions_listen` rustdoc as an operational requirement (put the route behind
auth or a reverse-proxy connection limit), rather than only in a module-private comment.

### WR-05: The client never validates that the agreed filter is a subset of what it requested, nor filters incoming frames

**File:** `src/client/subscriptions.rs:328-405`, `467-501`

**Issue:** `SubscriptionStream::open` deserializes the acknowledgement's `notifications`
into `self.acknowledged` and exposes it via `acknowledged()` as authoritative, but never
compares it to the filter the caller passed to `Client::subscriptions_listen`. Likewise
`classify_frame` forwards any tagged, decodable `ServerNotification` regardless of
whether the caller asked for that kind. `examples/s49_v2_subscriptions_client.rs:139-143`
then asserts on `acknowledged().notifications` as if it were verified.

The module doc enumerates five wire-contract checks the client performs; the "never a
superset" MUST (stated at `src/types/subscriptions.rs:180-183` and
`src/server/streamable_http_server.rs:256-258`) is not among them and is enforced only
server-side. A buggy or hostile server can therefore claim agreement it was not asked
for and push `notifications/resources/updated` for URIs the caller never named, and the
SDK will hand them to the application as legitimate subscription traffic.

**Fix:** Keep the requested filter on the stream and enforce both halves:

```rust
pub(crate) async fn open(
    subscription_id: RequestId,
    requested: SubscriptionFilter,
    mut frames: SubscriptionFrameStream,
) -> Result<Self> {
    // ... existing ack parsing ...
    if !acknowledged.notifications.is_subset_of(&requested) {
        return Err(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "spec MUST: the agreed filter is never a superset of the request",
        ));
    }
    ...
}
```

and in `classify_frame`, yield `FrameOutcome::Failed` for a notification whose
`subscription_kind_of` is not covered by the agreed filter. `SubscriptionFilter::covers`
already exists; it needs to be reachable from the client module (or mirrored).

### WR-06: `rejection_error` collects an unbounded response body from an untrusted server

**File:** `src/client/subscriptions.rs:131-150`

**Issue:**

```rust
let collected = match body.collect().await { ... };
```

There is no size limit. `MAX_ECHOED_FRAME` / `truncate` bound only what is echoed into
the error MESSAGE — by then the whole body is already resident. A server (or an
intermediary error page) that answers `subscriptions/listen` with a non-`text/event-stream`
content type and a multi-gigabyte body exhausts client memory. The doc comment at line 66-71
explicitly reasons about hostile unbounded strings for the echo path but not for the read.

**Fix:** Bound the read the same way the server bounds request bodies
(`read_body_with_limit`):

```rust
use http_body_util::{BodyExt, Limited};

const MAX_REJECTION_BODY: usize = 64 * 1024;

let collected = match Limited::new(body, MAX_REJECTION_BODY).collect().await {
    Ok(collected) => collected.to_bytes(),
    Err(e) => return Error::Transport(TransportError::Request(e.to_string())),
};
```

### WR-07: `close_subscription_streams` — the only graceful-teardown trigger — has zero callers

**File:** `src/server/mod.rs:859-861`, `src/server/subscriptions.rs:584-591`

**Issue:** `grep -rn close_subscription_streams src/ tests/ examples/` returns only the
definition and two doc references. Nothing in `StreamableHttpServer`'s shutdown path
calls it, no integration test exercises it, and `examples/s49_v2_subscriptions_client.rs`
uses `http.abort()` instead. Consequences:

- the documented "server shutdown" closure trigger (the only one that emits a terminal
  `SubscriptionsListenResult`) is never exercised end-to-end;
- `ListenEntry::terminal` — a pre-built `String` per live stream — is dead weight in
  every deployment that does not hand-wire the call;
- the client's `FrameOutcome::Terminal` arm is covered only by canned unit-test payloads,
  never by a real server frame.

Also note `close_all` does not mark the registry closed, so a stream registered a
microsecond after shutdown begins is never told to close.

**Fix:** Call `close_subscription_streams()` from `StreamableHttpServer`'s shutdown /
`Drop` path (or from whatever graceful-shutdown signal handler the transport owns), and
add an integration test in `tests/v2_subscriptions_client.rs` that opens a stream, calls
it, and asserts the client's stream ends with `None` after receiving the terminal result.
Consider an `AtomicBool closed` on `ListenRegistry` so post-shutdown registrations are
refused.

### WR-08: The fuzz target's cross-delivery invariant can report a false crash

**File:** `fuzz/fuzz_targets/subscription_listen_frames.rs:30-40`

**Issue:** Invariant 2 asserts that a delivered notification implies the raw bytes
contained the literal subscription id:

```rust
let text = String::from_utf8_lossy(data);
assert!(text.contains(SUBSCRIPTION_ID), "...");
```

JSON permits `\u` escapes, so `"fuzz-subscription-4f1c9a2e"` decodes to the correct
id while the raw bytes do not contain the literal. libFuzzer will eventually find this
and report it as a crash in the decoder, which it is not. A fuzz target that produces
false positives gets muted.

**Fix:** Compare against the DECODED value rather than the raw bytes — e.g. re-parse each
delivered frame, or relax the assertion to the substring that survives escaping:

```rust
// The id after JSON unescaping is what matters; a `\uXXXX`-escaped spelling is a
// legitimate encoding of the same id, not a cross-tag escape.
if outcomes.iter().any(std::result::Result::is_ok) {
    let text = String::from_utf8_lossy(data);
    let unescaped_hit = serde_json::from_slice::<serde_json::Value>(data)
        .map(|v| v.to_string().contains(SUBSCRIPTION_ID))
        .unwrap_or(false);
    assert!(text.contains(SUBSCRIPTION_ID) || unescaped_hit, "...");
}
```

### WR-09: Silent `unwrap_or_else` fallbacks can emit a spec-violating terminal frame

**File:** `src/server/streamable_http_server.rs:2702-2720`

**Issue:** `listen_terminal_result_frame` degrades twice without a trace:

```rust
serde_json::to_value(result).unwrap_or_else(|_| json!({})),        // line 2710
...
serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())  // line 2720
```

The first fallback produces a result with NO `_meta.subscriptionId`, which the spec marks
REQUIRED; the client's `verify_subscription_id` then rejects the frame as a cross-tag
error instead of closing the stream gracefully. The second produces a literal `{}` frame,
which the client classifies as an untagged-frame error. Both are practically unreachable,
but the fallback silently converts an internal error into a protocol violation attributed
to the server.

**Fix:** Return `Option<String>` (or `Result`) and log at `error!` on failure; skip
registering the terminal frame rather than storing a malformed one. A stream with no
terminal frame simply ends, which is already one of the three documented closure modes.

## Info

### IN-01: `-32005 RATE_LIMITED` is answered with HTTP 200

**File:** `src/server/streamable_http_server.rs:2909`, `690-702`

**Issue:** The concurrency-cap refusal uses `RATE_LIMITED`, which falls into
`v2_status_for_code`'s `_ => StatusCode::OK` arm. A cap refusal therefore ships as
HTTP 200 with a JSON-RPC error body. Proxies, load balancers and metrics pipelines that
key on status cannot see the throttle.

**Fix:** Add `ec::RATE_LIMITED => StatusCode::TOO_MANY_REQUESTS` to `v2_status_for_code`,
and pin it in the existing status-mapping test.

### IN-02: The global-cap refusal message leaks server-wide capacity state to unauthenticated callers

**File:** `src/server/subscriptions.rs:340`

**Issue:** `"too many concurrent subscriptions/listen streams on this server"` tells an
anonymous caller that the SERVER (not their own quota) is saturated — a free saturation
oracle for an attacker probing the 64-slot bound.

**Fix:** Return one indistinguishable message for both `ListenRejection` variants on the
wire and keep the discriminated text in `tracing` only.

### IN-03: Anonymous principals share a namespace with authenticated subjects

**File:** `src/server/subscriptions.rs:359-362`

**Issue:** `anonymous_principal()` returns `anon#N` as a plain `String` into the same
`ListenKey::principal` field an `AuthContext::subject` occupies. An auth provider whose
subjects happen to look like `anon#5` collides with the anonymous namespace.

**Fix:** Make the principal an enum (`Principal::Anonymous(u64)` /
`Principal::Subject(String)`) so the two namespaces are type-separated, or prefix
authenticated subjects (`sub:{subject}`).

### IN-04: `take_utf8_prefix` discards a legitimately incomplete tail after an invalid byte

**File:** `src/client/subscriptions.rs:236-253`

**Issue:** The `Err(_)` arm lossily decodes and clears the ENTIRE buffer. For
`[0xff, 0xE2, 0x98]` (one invalid byte followed by two thirds of `'☂'`), the valid
incomplete tail is destroyed and the character arriving in the next chunk is corrupted
into a second replacement char. Only reachable after invalid bytes are already present,
so the practical impact is limited to garbled diagnostics.

**Fix:** Decode lossily only up to `valid_up_to() + error_len()`, then re-run the
incomplete-tail logic on the remainder.

### IN-05: `subscriptions/listen` performs no scope/permission check beyond authentication

**File:** `src/server/streamable_http_server.rs:2877`

**Issue:** `AuthContext.scopes` is ignored; any authenticated principal receives every
`listChanged` notification and any `resources/updated` for any URI it names. Where tool or
resource visibility is scope-gated elsewhere in the server, the stream is a side channel
for change/timing information about resources the principal cannot read.

**Fix:** Document the trust boundary in `Client::subscriptions_listen` / the module docs,
and consider an optional `AuthContext`-aware filter hook on the registry.

### IN-06: `required-features` for `s49_v2_subscriptions_client` over-constrains

**File:** `Cargo.toml:620-622`

**Issue:** The example is declared `required-features = ["streamable-http", "http-client"]`
but nothing on its path needs `http-client` (which pulls in `reqwest`);
`client::subscriptions` is gated on `streamable-http` alone. The example's own header also
says `--features full`, which is a third spelling.

**Fix:** Drop `http-client` from the required features and make the header's run command
match (`--features streamable-http`).

### IN-07: Self-admitted deferred work in `Cargo.toml` (CLAUDE.md zero-SATD)

**File:** `Cargo.toml:594-604`

**Issue:** "Renaming these two to the next free slots is **deferred**: their paths are
pinned by the Phase-113 plan's artifact contract" is a deferral note carried in a shipped
manifest. CLAUDE.md states "Zero SATD (Self-Admitted Technical Debt) comments" as a
non-negotiable. The `sNN_` example prefix is now ambiguous (`s47`, `s48`, `s49` each name
two different examples), which will confuse anyone following the numbered sequence.

**Fix:** Either rename the three new examples to free slots now (the plan artifact record
is a `.planning/` document, not a compatibility surface), or move the rationale into
`.planning/phases/113-.../deferred-items.md` — where it already lives — and leave only a
neutral one-line pointer in `Cargo.toml`.

---

_Reviewed: 2026-07-26T04:25:38Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
