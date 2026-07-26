---
phase: 113-stateless-http-multi-round-trip-elicitation
reviewed: 2026-07-26T09:40:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - src/server/subscriptions.rs
  - src/server/streamable_http_server.rs
  - tests/v2_subscriptions.rs
  - src/shared/sse_parser.rs
  - src/shared/http.rs
  - src/client/subscriptions.rs
  - fuzz/fuzz_targets/subscription_listen_frames.rs
findings:
  critical: 3
  warning: 6
  info: 4
  total: 13
status: issues_found
---

# Phase 113: Code Review Report (gap closure)

**Reviewed:** 2026-07-26T09:40:00Z
**Depth:** standard
**Files Reviewed:** 7
**Scope:** `d3b54221..HEAD`, non-`.planning/` paths (plans 113-14, 113-15, 113-16)
**Status:** issues_found

## Summary

Three plans claimed to close CR-01, CR-02 and CR-03 from the previous review. Two of
those claims hold. One does not, and it does not hold in a way the shipped test suite
cannot see.

**CR-01 / CR-02 (113-14) — CLOSED, with one new regression.** The generation scheme is
sound. `next_generation` is drawn under the same `entries.write()` guard that performs
the insert (`src/server/subscriptions.rs:556-574`), it is strictly monotonic per
registry and never reused, and both teardown paths compare it before removing
(`remove_entry:725-730`, `disconnect_overflowed:674-698`). Because a generation is
globally unique within a registry, a match implies entry identity — there is no ABA
window; the only theoretical reuse is `u64` wraparound. The occupancy check and the
insert genuinely share one write guard, so two concurrent registrations cannot both see
the key free. Lock ordering is clean: `per_principal` is scoped and released before
`entries` is taken in `register`, and `remove_entry` releases `entries` before
`prune_principal` takes `per_principal`, so there is no inversion. The `-32600` →
HTTP 400 claim is verified end to end (`ListenRejection::code:391-398` →
`listen_rejection_response:2692-2715` → `v2_dispatch_response_status:834-845` →
`v2_status_for_code:690-702`), and `tests/v2_subscriptions.rs:681-762` pins it live.
What the fix did NOT consider is the ordinary reconnect: the duplicate check asks only
"is the key occupied", never "is the incumbent still alive" (CR-03 below).

**CR-03 (113-15) — NOT CLOSED.** The bound added to `SseParser` covers only `buffer`,
i.e. one unterminated *line*. `EventBuilder::data` still accumulates across `data:`
lines with no bound at all (`src/shared/sse_parser.rs:329-336`), and the new discard
branch is gated on `!data.contains('\n')` (`:249-261`), so a peer that simply includes
newlines never trips it. I compiled the crate and measured this: with
`SseParser::with_max_buffer_size(64)`, feeding `"data: AAAAAAAA\n"` 100,000 times
accumulated **899,999 bytes** into one in-progress event with `overflowed() == false`.
The same construction at 200,000 iterations of `"data:x\n"` reached 399,999 bytes. Heap
growth is linear in stream lifetime and entirely peer-controlled — which is exactly the
CR-03 vulnerability, on exactly the long-lived `subscriptions/listen` path the plan
named. Separately, a single SSE line of 1,000,000 bytes is accepted and emitted under a
64-byte bound (`overflowed() == false`) whenever the chunk carrying it also carries the
terminating newline, so `max_buffer_size` is not an upper bound on `buffer` and
`overflowed()` does not reliably detect the condition its two consumers were added to
observe.

Both bounding tests and both new doctests only ever feed newline-free chunks. That is
precisely why a green suite (I ran `cargo test --lib ... sse_parser`: 20/20 pass) is
compatible with the vulnerability still being open.

**113-16 (fuzz).** The target now runs two bounds and three invariants. Invariant 3 is a
tautology — `overflowed` has one write site and no clearing path — and it is duplicated
as an in-tree proptest. Invariant 2 was fixed for the `\u`-escape false positive by
disabling itself for any input containing a backslash, which is far broader than
necessary. The property the plan exists to defend — that memory stays bounded — is not
asserted by the fuzz target at all, because the seam exposes no size. And
`decode_listen_chunks_for_fuzz` is `pub` in a `pub mod`, so the undeclared scope
expansion into `src/client/subscriptions.rs` is a real public-API widening, not just a
paperwork issue.

**Verified as claimed, no finding:** the two whole-body `feed` call sites
(`src/shared/streamable_http.rs:528` and `:1150`) are behaviourally unchanged. Each
builds a fresh per-body parser and feeds a complete SSE body; a well-formed body carries
newlines and skips the branch, and a >1 MiB body with no newline at all produced no
events before the change and produces no events after it. There is no path where
overflow truncates-and-emits a partial frame *as if complete*: the discard branch
returns `Vec::new()` and resets `current_event` before any dispatch. (The distinct
problem is that oversized content reaches the caller *without* the branch firing — CR-02.)

## Critical Issues

### CR-01: The SSE line bound does not bound the parser — `data:` accumulation is still unbounded remote-driven heap growth

**File:** `src/shared/sse_parser.rs:249-261` (the new bound), `:329-336` (the unbounded
accumulator); reached from `src/client/subscriptions.rs:188` and `src/shared/http.rs:173`
**Status:** NEW as a defect of the gap-closure fix — plan 113-15 claims this vector is
closed and it is not. The accumulator itself is pre-existing.

**Issue:** The bound is only ever consulted when the incoming chunk contains no newline:

```rust
if self.buffer.len().saturating_add(data.len()) > self.max_buffer_size
    && !self.buffer.contains('\n')
    && !data.contains('\n')
{ /* discard + latch */ }
```

An SSE event is only dispatched on a blank line (`process_line:303-305`). Until then
every `data:` line is appended to `self.current_event.data`:

```rust
"data" => {
    if self.current_event.data.is_empty() { self.current_event.data = value.to_string(); }
    else { self.current_event.data.push('\n'); self.current_event.data.push_str(value); }
},
```

Nothing bounds `current_event.data`. A peer that streams `"data: A\n"` forever and never
sends the blank line grows the client's heap without limit, and because every chunk
carries `\n` the new discard branch is never entered and `overflowed()` never latches —
so neither `listen_overflow` (`src/client/subscriptions.rs:320-332`) nor
`report_sse_line_overflow` (`src/shared/http.rs:82-96`) ever ends the stream.

Measured against the built crate (`SseParser::with_max_buffer_size(64)`):

| input | result |
|---|---|
| `feed("data: AAAAAAAA\n")` x 100 000 | `event.data.len() == 899_999`, `overflowed() == false` |
| `feed("data:x\n")` x 200 000 | `event.data.len() == 399_999`, `overflowed() == false` |

This is the same denial-of-service the prior CR-03 named ("a peer that streams bytes
... grows `buffer` without limit until the process is OOM-killed"), unchanged except
that the attacker must now include newline bytes.

**Fix:** Bound the accumulated event, not only the line. The cheapest correct form
reuses the existing latch so both consumers keep working unmodified:

```rust
// in SseParser::feed, before push_str — one bound covering BOTH accumulators
let in_flight = self.buffer.len().saturating_add(self.current_event.data.len());
if in_flight.saturating_add(data.len()) > self.max_buffer_size {
    self.overflowed = true;
    self.buffer.clear();
    self.current_event = EventBuilder::new();
    return Vec::new();
}
```

Note this deliberately drops the `contains('\n')` guard, which CR-02 shows is what makes
the bound unenforceable; a legitimately large *complete* body then needs its call sites
to pass a bound that admits it (`SseParser::with_max_buffer_size(body.len().max(default))`
at the two whole-body sites, or a `feed_whole_body` entry point that bypasses the check).
Add a regression test that asserts the newline-carrying flood latches:

```rust
#[test]
fn a_newline_carrying_flood_is_bounded_too() {
    let mut parser = SseParser::with_max_buffer_size(64);
    for _ in 0..10_000 { let _ = parser.feed("data: AAAAAAAA\n"); }
    assert!(parser.overflowed(), "an unterminated EVENT is as unbounded as an unterminated LINE");
}
```

### CR-02: `max_buffer_size` is not an upper bound and `overflowed()` misses the condition it exists to detect

**File:** `src/shared/sse_parser.rs:121-129` (field doc), `:155-159` (contract),
`:249-263` (enforcement)
**Status:** NEW — introduced by plan 113-15.

**Issue:** When the incoming chunk contains a newline the bound check is skipped
entirely and `self.buffer.push_str(data)` runs unconditionally. So:

* `buffer` transiently reaches `max_buffer_size + data.len()`, contradicting the field
  doc "Upper bound on `buffer`, i.e. on ONE unterminated SSE line";
* a single SSE line arbitrarily larger than the bound is parsed and emitted,
  contradicting `with_max_buffer_size`'s stated contract "A chunk that would push the
  buffer past it while completing no line at all is DISCARDED";
* `overflowed()` stays `false`, so the two observers built on it in this same diff —
  `listen_overflow` and `report_sse_line_overflow` — do not fire.

Measured: `SseParser::with_max_buffer_size(64).feed("data: " + "B"x1_000_000 + "\n\n")`
returns one event with `data.len() == 1_000_000` and `overflowed() == false`. The
property test at `:762-781` already encodes the looseness (`buffer.len() <= max(8,
chunk.len())`) without the docs being corrected to match.

The practical ceiling is `max_buffer_size + one transport chunk` (hyper's adaptive read
buffer, up to a few hundred KiB), so this is not by itself an OOM — but it is a
security control that does not enforce what its callers are told it enforces, and it is
the mechanism CR-01 walks through.

**Fix:** Drop the `contains('\n')` conditions (see CR-01's patch) so the bound is
unconditional, and correct the two doc blocks to state the real guarantee. If the
whole-body call sites need to keep accepting oversized complete bodies, give them an
explicit escape hatch rather than making the bound conditional on peer-chosen framing:

```rust
/// Feed a COMPLETE body in one call, bypassing the incremental line bound.
/// Only safe where the body was already read into memory under a separate size cap.
pub fn feed_complete_body(&mut self, body: &str) -> Vec<SseEvent> { /* old unbounded path */ }
```

### CR-03: The duplicate-id refusal has no liveness check, so an ordinary reconnect is answered `-32600` at HTTP 400

**File:** `src/server/subscriptions.rs:559-562`
**Status:** NEW — introduced by plan 113-14.

**Issue:**

```rust
let mut entries = self.entries.write();
if entries.contains_key(&key) {
    return Err(ListenRejection::DuplicateSubscriptionId);
}
```

The check asks whether the key is *occupied*, never whether the incumbent is *alive*.
`ListenGuard` is dropped only when the SSE stream future unwinds
(`src/server/streamable_http_server.rs:2970-2976`), which for an ungraceful client
disconnect — mobile handoff, NAT rebind, LB reap, `SIGKILL`ed client — does not happen
until the keep-alive write fails (`LISTEN_KEEP_ALIVE_INTERVAL`, 15 s) plus TCP
retransmit. During that whole window a client that reconnects with the same
`subscriptionId` — which the spec defines as the JSON-RPC request id, and which every
SDK that uses small integer ids or a stable id will reuse — is refused.

Two things make that worse than a plain race:

1. The refusal is `-32600 INVALID_REQUEST` at HTTP 400. That is the JSON-RPC code for a
   structurally malformed request and the HTTP class for "do not retry this". The actual
   condition is transient server state. A client SDK reading the code correctly will
   surface a hard protocol error rather than backing off and retrying.
2. `ListenRejection::message()` says "a subscriptions/listen stream is already open for
   this subscription id", which is false from the client's point of view — its stream is
   not open, it just lost the socket.

The pre-fix behaviour (blind eviction) was the CR-01 bug, so reverting is not the
answer; the missing piece is that an incumbent whose receiver is gone is not a live
stream at all. Note also the still-open WR-01 interaction: after an overflow disconnect
the entry is removed but the permits are not, so the `LISTEN_OVERFLOW_NOTICE`'s advice
to "re-issue subscriptions/listen" can instead hit `PerPrincipalLimit`. The gap-closure
diff did not make WR-01 worse, but CR-03 sits on the same reconnect path.

**Fix:** Treat a closed channel as a free key — this cannot evict a live stream, because
a live stream's receiver is held by its own SSE future:

```rust
let mut entries = self.entries.write();
match entries.get(&key) {
    // The incumbent's receiver is gone: its stream is already dead and only its
    // guard has yet to unwind. Reclaiming it here cannot disconnect anybody.
    Some(existing) if existing.sender.is_closed() => { entries.remove(&key); },
    Some(_) => return Err(ListenRejection::DuplicateSubscriptionId),
    None => {},
}
```

The late guard drop is already harmless — `remove_entry` is generation-scoped, so it
will not reclaim the successor. Add a test that closes the receiver, leaves the guard
alive, and asserts the same key re-registers:

```rust
#[tokio::test]
async fn a_reconnect_takes_over_a_key_whose_stream_is_already_dead() {
    let registry = Arc::new(ListenRegistry::new());
    let (_stale_guard, rx) = open(&registry, "solo", 1, tools_only()).expect("A registers");
    drop(rx); // the client vanished; only the guard has yet to unwind
    assert!(open(&registry, "solo", 1, tools_only()).is_ok(), "a reconnect is not a duplicate");
}
```

If takeover is judged too aggressive, the minimum is to stop answering a transient
server-state condition with a non-retryable client-error code: use `RATE_LIMITED`
(`-32005`) or add a `SUBSCRIPTION_ID_IN_USE` code mapped to `409 Conflict` in
`v2_status_for_code`, and say "still open" rather than "already open".

## Warnings

### WR-01: The `!self.buffer.contains('\n')` term in the bound check is dead, and its comment claims a role it does not have

**File:** `src/shared/sse_parser.rs:244-251`
**Status:** NEW.

**Issue:** At every entry to `feed`, `self.buffer` cannot contain `\n`. The drain loop
runs `while let Some(line_end) = self.buffer.find('\n')` and only exits when there is
none (`:266-295`), and the overflow branch clears the buffer outright (`:254`). So
`!self.buffer.contains('\n')` is unconditionally `true` — a dead predicate, and an O(n)
scan of up to 1 MiB on every chunk. The comment above it states:

> The "contains a `\n`" condition is what keeps a single legitimately large COMPLETE
> body working

That role belongs entirely to `!data.contains('\n')`. A reader auditing the bound is
told the buffer term is load-bearing; it is not, and the misattribution is what conceals
CR-02.

**Fix:** Delete the buffer term. Once CR-01/CR-02 are fixed the `data` term goes too;
if it is retained in the interim, replace the comment with the accurate statement:

```rust
// `buffer` never contains a `\n` here (the drain loop below runs to exhaustion),
// so only `data` can complete a line. Skipping the bound when it can is what keeps
// a single legitimately large COMPLETE body working.
```

### WR-02: `SseConfig` is still not configuration — only its `Default` is read

**File:** `src/shared/sse_parser.rs:474-481` (doc), `:145` (the only read)
**Status:** NEW (the doc claim), pre-existing (the dead struct).

**Issue:** The gap-closure diff answers "`max_buffer_size` is dead code" by reading
`SseConfig::default().max_buffer_size`. Nothing anywhere constructs an `SseConfig` and
plumbs it into a parser (`grep -rn SseConfig src/ tests/ examples/` returns only the
definition, the two `::default()` reads, and test assertions). A user who writes
`SseConfig { max_buffer_size: 4096, ..Default::default() }` still gets no behaviour
change, yet the field's new doc reads as though the struct configures the parser. The
same is true of `retry`, `compression` and `headers`, none of which is read anywhere.

**Fix:** Either give the struct a real consumer —

```rust
impl SseParser {
    #[must_use]
    pub fn from_config(config: &SseConfig) -> Self {
        Self::with_max_buffer_size(config.max_buffer_size)
    }
}
```

— or state plainly in the field doc that `SseConfig` is a value type not yet wired into
any transport and that `SseParser::with_max_buffer_size` is the only way to change the
bound.

### WR-03: The fuzz target's Invariant 3 is a tautology, and the property the plan exists to defend is never asserted

**File:** `fuzz/fuzz_targets/subscription_listen_frames.rs:108-120`; duplicated at
`src/client/subscriptions.rs:1220-1239`
**Status:** NEW.

**Issue:** `SseParser::overflowed` is a `bool` with exactly one write site
(`self.overflowed = true` at `sse_parser.rs:253`) and no path that clears it — `reset`
explicitly does not. "The latch never clears" therefore cannot fail for any input, at
any bound, ever. It is asserted twice (fuzz campaign plus in-tree proptest) and
documented at length in three places.

Meanwhile the invariant that would have caught CR-01 and CR-02 — that parser memory
stays bounded — is not asserted anywhere in the fuzz target, because
`decode_listen_chunks_for_fuzz` returns only outcomes and flags, never a size. And the
`max_buffer_size = 8` pass of `MAX_BUFFER_SIZES` puts the parser into the
discarded/latched state on essentially the first 16-byte chunk of nearly every input —
a state `read_next_frame` exits immediately in production — so roughly half the campaign
budget is spent on inputs that produce no outcomes and no checkable assertions.

**Fix:** Replace Invariant 3 with a bound assertion, which requires the seam to report
a size:

```rust
// in decode_listen_chunks_for_fuzz, alongside `overflowed`
peak_parser_bytes.push(parser.buffered_bytes()); // new pub(crate) accessor: buffer + current_event.data

// in the fuzz target
assert!(
    peak_parser_bytes.iter().all(|n| *n <= max_buffer_size),
    "the parser retained {peak_parser_bytes:?} bytes under a {max_buffer_size}-byte bound",
);
```

### WR-04: Invariant 2's backslash escape hatch is scoped to the whole input, not to the frame it protects

**File:** `fuzz/fuzz_targets/subscription_listen_frames.rs:100-106`
**Status:** NEW.

**Issue:**

```rust
if outcomes.iter().any(std::result::Result::is_ok) && !text.contains('\\') {
    assert!(text.contains(SUBSCRIPTION_ID), "...");
}
```

`text` is the lossy decode of the *entire* input. One `0x5C` byte anywhere — in a
completely unrelated frame, in trailing garbage, inside a comment line the parser
discards — suppresses the cross-delivery assertion for every frame in that run.
libFuzzer preferentially retains inputs that reach new coverage, and inputs containing
backslashes reach the JSON-escape decoder, so the fraction of runs where the invariant
is actually checked will fall over a long campaign. The prior review's suggested fix
(compare against the DECODED id) does not have this property and is directly available
here: `outcomes` already carries typed `ServerNotification`s.

**Fix:** Assert on the decoded value instead of gating on the raw bytes:

```rust
for outcome in outcomes.iter().flatten() {
    let tagged = serde_json::to_value(outcome).ok()
        .and_then(|v| v["params"]["_meta"][SUBSCRIPTION_ID_META_KEY].as_str().map(str::to_owned));
    assert_eq!(
        tagged.as_deref(), Some(SUBSCRIPTION_ID),
        "a notification was delivered that is not tagged with this subscription's id",
    );
}
```

### WR-05: `decode_listen_chunks_for_fuzz` widens the crate's public API surface

**File:** `src/client/subscriptions.rs:666-696`
**Status:** NEW, and an undeclared scope expansion — plan 113-16 did not list
`src/client/subscriptions.rs` in `files_modified`.

**Issue:** `pub mod subscriptions;` (`src/client/mod.rs:48`) inside `pub mod client;`
(`src/lib.rs:27`) makes this a fully public item. `#[doc(hidden)]` hides it from
rustdoc; it does not restrict visibility and does not exempt it from semver. Every
downstream crate can call it, and its signature bakes in three things the SDK should not
be committing to: a `&[&[u8]]` chunk model, an unvalidated `max_buffer_size: usize`
(`0` is accepted and makes the parser latch on the first non-empty chunk), and errors
flattened to `String` `Display` output. It also silently drops terminal frames, so a
caller who mistook it for a decode API would lose stream-close signals.

`cargo-fuzz` passes `--cfg fuzzing`, so there is a strictly better option that keeps
this out of the public API for every normal build.

**Fix:**

```rust
#[cfg(any(fuzzing, test))]
#[doc(hidden)]
#[must_use]
pub fn decode_listen_chunks_for_fuzz(/* ... */) { /* ... */ }
```

and add an `unexpected_cfgs` allowance for `fuzzing` in `Cargo.toml` `[lints.rust]` if
the crate denies it. The in-crate proptests continue to compile under `cfg(test)`.

### WR-06: `register`'s new early return skips `prune_principal`, so a per-principal semaphore can be orphaned

**File:** `src/server/subscriptions.rs:552-562`, `:738-746`
**Status:** NEW — the `DuplicateSubscriptionId` arm is the first rejection path that can
return after the `per_principal` map entry has been created and then leave no live
guard behind to prune it.

**Issue:** `prune_principal` is called only from `ListenGuard::drop` (`:479`). Consider
this interleaving, all of which is reachable:

1. B enters `register` for principal P, clones P's `Arc<Semaphore>` (strong count 3:
   map + A's permit + B's local).
2. B reads `entries` and sees A's entry — duplicate.
3. A's guard drops: `remove_entry`, then `drop(permits)` (count 2), then
   `prune_principal` — which sees count 2 and does not prune.
4. B returns `Err(DuplicateSubscriptionId)`; its local `Arc` drops (count 1).

The map now holds an entry nothing will ever remove. Growth is bounded by the number of
distinct authenticated subjects rather than by request volume, so this is a slow leak
rather than a vector — but it defeats the stated purpose of `prune_principal` ("so the
map does not grow without bound").

**Fix:** Prune on the rejection paths too:

```rust
let principal_permit = match principal_semaphore.try_acquire_owned() {
    Ok(permit) => permit,
    Err(_) => { self.prune_principal(&key.principal); return Err(ListenRejection::PerPrincipalLimit); },
};
// ... and in the duplicate arm:
if entries.contains_key(&key) {
    drop(entries);
    drop(principal_permit);
    self.prune_principal(&key.principal);
    return Err(ListenRejection::DuplicateSubscriptionId);
}
```

## Info

### IN-01: `decode_listen_chunk_for_fuzz` is now an unused fuzz seam that remains public

**File:** `src/client/subscriptions.rs:631-638`

**Issue:** No fuzz target calls the singular seam any more — `subscription_listen_frames.rs`
uses the plural one. Its only callers are two in-crate proptests
(`:1208`, `:1217`), which can reach the private decode path directly. It stays `pub`
in a public module, so the crate now exports two `#[doc(hidden)]` fuzz seams, one of
which no fuzz target uses.

**Fix:** Fold it into WR-05's `#[cfg(any(fuzzing, test))]` treatment, or delete it and
have the two proptests call `decode_listen_chunks_for_fuzz(&[bytes], id, MAX_LISTEN_LINE_BYTES)`.

### IN-02: The flood test re-types the 1 MiB literal it claims to source from `SseConfig`

**File:** `src/shared/sse_parser.rs:604-612`

**Issue:** `a_newlineless_flood_cannot_grow_the_buffer_past_the_bound` hardcodes
`1024 * 1024` in both the assertion and its message, while
`new_takes_its_bound_from_the_sse_config_default` exists specifically to establish that
the number lives in exactly one place. Changing `SseConfig::default()` would leave this
test asserting a stale bound and still passing.

**Fix:** `let bound = SseConfig::default().max_buffer_size;` and use it in both places.

### IN-03: Every bounding test and doctest feeds only newline-free chunks

**File:** `src/shared/sse_parser.rs:597-700`, `:762-781`; `src/client/subscriptions.rs:1040-1066`,
`:1096-1123`; `src/shared/http.rs:427-459`

**Issue:** Ten new tests exercise the bound. Every single one drives it with
`"x".repeat(N)` or a `[b'x'; 16]` chunk — no newline anywhere. That is the one input
class the enforcement handles, which is why the suite is green while CR-01 and CR-02 are
open. The one property test that does generate `\n`
(`a_bounded_feed_never_panics_on_arbitrary_text`) asserts only `buffer.len() <= max(8,
chunk.len())`, which is satisfied by design and says nothing about `current_event`.

**Fix:** Add at least one newline-carrying flood case per feeder (see CR-01's suggested
regression test) and one oversized-complete-line case (CR-02).

### IN-04: A source file references a planning artifact by bare filename

**File:** `fuzz/fuzz_targets/subscription_listen_frames.rs:6`

**Issue:** "The recorded campaign lives in `113-FUZZ-EVIDENCE.md`" — no path, and the
file lives under `.planning/phases/113-.../`, which is not shipped in the published
crate. A reader of the crate source cannot resolve it.

**Fix:** Either give the full repo-relative path or drop the reference and inline the
one-line campaign command the target should be run with.

---

_Reviewed: 2026-07-26T09:40:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
