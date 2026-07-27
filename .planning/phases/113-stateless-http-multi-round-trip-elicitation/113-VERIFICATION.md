---
phase: 113-stateless-http-multi-round-trip-elicitation
verified: 2026-07-26T21:40:00Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/5 must-haves verified
  gaps_closed:
    - "Gap item 3 (unbounded SSE client buffer on newline-carrying floods) is now genuinely closed by 113-17. Independently re-read src/shared/sse_parser.rs:365-419: SseParser::feed's pre-check is unconditional over `buffered_bytes() + data.len()` — the `!data.contains('\\n')` escape that let 899,999 bytes accumulate under a 64-byte bound is gone, replaced by an unconditional discard-and-latch. A second, independently-sufficient post-drain check covers the residual. No contains('\\n') gate remains anywhere in the enforcement path."
    - "The 113-14-introduced regression (an ordinary client reconnect after an ungraceful disconnect was refused as a duplicate with a non-retryable HTTP 400) is resolved by 113-18, by contract rather than by the liveness-reclaim originally planned. Independently re-read src/server/subscriptions.rs:396-434: all three ListenRejection variants (PerPrincipalLimit, GlobalLimit, DuplicateSubscriptionId) now map to RATE_LIMITED (-32005), and streamable_http_server.rs:690-702 confirms RATE_LIMITED is absent from the 400 arm of v2_status_for_code, so it resolves to HTTP 200 -- a retryable shape instead of the previous terminal 400. pmcp's own Client::subscriptions_listen mints a fresh Uuid::new_v4() request id on every call (independently confirmed), so this client can never collide with itself; a third-party client that reuses an id now gets a retryable signal instead of a misleading hard error. This is an architecturally-justified resolution (the receiver and the RAII guard share one stream::unfold state tuple server-side, so remote-peer liveness is genuinely unobservable at that layer) documented with its own evidence, not a silent scope cut."
  gaps_remaining: []
  regressions:
    - "NONE of the four gap-closure plans' own changes regressed anything I could independently verify -- the four protected 113-14 tests are unchanged, the fresh-id tripwire fires correctly, and the newline-flood/oversized-line SSE tests pass. However, a FRESH code review (committed 57aad7bc, same day as this re-verification) found three BLOCKER-severity defects in code this phase's gap-closure round directly touched, which I independently reproduced by reading the current source (not by trusting the review): CR-01 (src/client/subscriptions.rs:147-166, an uncapped body.collect().await on the subscriptions/listen client's non-stream rejection path), CR-02 (src/shared/sse_parser.rs:164-189, take_utf8_prefix is O(n^2) on invalid UTF-8 bytes and runs BEFORE every byte-count bound this phase added, reachable via read_next_frame at client/subscriptions.rs:237 on the same subscriptions/listen stream), and CR-03 (src/shared/http.rs:346-356, HttpTransport::send_request collects an unbounded peer-controlled body with zero cap, in the same file 113-17 hardened the SSE reader of). These are not regressions caused by 113-17/18/19/20's edits -- CR-02 and CR-03 predate this round and CR-01 was never in scope of any of the four plans -- but they are real, currently-shipping defects that directly falsify this round's own documentation (DEFAULT_MAX_COLLECTED_BODY_BYTES's rustdoc states 'every one of this transport's response reads is a whole-body read... and the peer chooses how many bytes it sends', which is false for the HttpTransport transport entirely and false for one StreamableHttpTransport-adjacent path). They keep Success Criterion 3's 'memory-bounded' claim not fully true."
gaps:
  - truth: "On the v2 path, subscriptions/listen delivers change notifications through a collision-safe, memory-bounded long-lived stream, and the client half (Client::subscriptions_listen) works correctly against it, including ordinary reconnects (HTTP-04, Success Criterion 3)"
    status: partial
    reason: >-
      Genuine progress since the prior verification: concurrent duplicate-registration collision
      safety (generation-scoped teardown) remains sound, the newline-carrying SSE buffer flood that
      was the previous verification's headline defect is now truly bounded (independently re-read,
      no escape-hatch remains), and the ordinary-reconnect regression from 113-14 is resolved by an
      evidenced architectural decision (retryable RATE_LIMITED instead of terminal INVALID_REQUEST,
      composed with pmcp's own client structurally never reusing an id). But a same-day fresh code
      review found three independently-confirmed BLOCKER defects in code this phase's round directly
      touched, all still present when I read the current source myself: an uncapped whole-body read
      on the subscriptions/listen client's own rejection path (CR-01), a quadratic-time UTF-8 decoder
      that runs before every byte-count bound this phase added and is reachable on the very
      subscriptions/listen stream this criterion is about (CR-02, a live remote CPU-exhaustion vector
      measured at ~1.17 CPU-seconds per 400 KiB of garbage bytes with no self-limiting behavior), and
      a sibling uncapped whole-body read on a related HTTP transport in the same file 113-17 hardened
      (CR-03). Together these directly falsify the DoS-hardening claim this round's own
      DEFAULT_MAX_COLLECTED_BODY_BYTES rustdoc makes about "every" response read being capped.
    artifacts:
      - path: "src/client/subscriptions.rs"
        issue: "rejection_error (lines 147-166) performs body.collect().await with no size cap on the subscriptions/listen client's non-event-stream response branch -- a fourth whole-body read not covered by any of this round's new caps. Confirmed present by direct read; matches CR-01 exactly, including the secondary character-vs-byte truncate() bound issue noted by the review."
      - path: "src/shared/sse_parser.rs"
        issue: "take_utf8_prefix (lines 164-189) drains the buffer one invalid-byte-run at a time (buffer.drain(..valid_up_to + invalid_len) inside a loop), making it O(n^2) on a run of invalid bytes. It runs on the RAW chunk before any of MAX_LISTEN_LINE_BYTES / DEFAULT_HTTP_SSE_BUFFERED_BYTES / DEFAULT_MAX_COLLECTED_BODY_BYTES apply, and is not self-limiting when a peer terminates each garbage frame with a newline (the drained buffer never trips overflow). Confirmed present by direct read; code is unchanged from what CR-02 describes and the review's suggested single-pass rewrite has not been applied."
      - path: "src/shared/http.rs"
        issue: "HttpTransport::send_request (lines 346-356) does `response.collect().await...to_bytes()` with no cap at all, in the same file whose sibling connect_sse SSE reader 113-17 bounded via DEFAULT_HTTP_SSE_BUFFERED_BYTES. Confirmed present by direct read; matches CR-03 exactly."
    missing:
      - "Route rejection_error's body read through a capped helper mirroring StreamableHttpTransport::collect_body_within_cap (the review's suggested pub(crate) max_collected_body_bytes()/collect_body_within_cap seam), plus a test mirroring collected_body_cap::an_over_cap_v2_error_envelope_falls_back_to_the_status_error for this fourth site."
      - "Rewrite take_utf8_prefix as a single-pass O(n) decoder (cursor-based, one drain at the end) per the review's suggested fix; add a deterministic regression pinning the shape (256 KiB of 0xFF yields 262144 replacement chars, buffer ends empty) and re-run the existing fuzz target with -timeout=1 so an algorithmic-complexity regression produces an artifact instead of a silent pass."
      - "Cap HttpTransport::send_request's response body the same way HttpTransport::connect_sse's SSE reader was capped in the same file by 113-17 (e.g. http_body_util::Limited over a new max_collected_body_bytes field with an additive builder, mirroring the private-field pattern already established for both sibling transports)."
human_verification: []
---

# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation Verification Report

**Phase Goal:** v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto pmcp's existing `stateless()` branch (not a transport fork); multi-round-trip elicitation works end-to-end; and the pmcp `Client` is the v2-speaking counterpart, folding the Phase-106 host handlers into the v2 flow. v1 session behavior is untouched.
**Verified:** 2026-07-26T21:40:00Z
**Status:** gaps_found
**Re-verification:** Yes — after gap-closure plans 113-17, 113-18, 113-19, 113-20 (both prior remaining gap items closed; a same-day fresh code review found three new BLOCKER defects in the same subsystem)

## Publication-block context (not a codebase gap)

ROADMAP.md marks the phase `[~]` (implemented, not `[x]` complete) and REQUIREMENTS.md carries
all seven requirement IDs (HTTP-01..05, CLNT-01, CLNT-02) as `[~]` "Implemented — pending final
schema". This is a disclosed, human-granted exception recorded in `113-SPEC-RECHECK.md`: as of
this re-verification (2026-07-26) `schema/2026-07-28` still does not exist upstream, so the three
v2 error-code constants are pinned to pre-final values under a written developer exception. All
four gap-closure executors honored the binding STATE.md gate and did not flip any checkbox. This
verification does **not** treat the unflipped checkboxes as a gap and does **not** recommend
flipping them — that decision has its own binding re-verification date already on file
(on or after 2026-07-28) and is outside this phase's control. **Independent of that gate, the
codebase-level gaps below mean the phase would not be ready to flip those checkboxes even once
the publication block lifts** — see the Gaps Summary.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A v2 HTTP request completes with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch, while v1 session behavior is unchanged (HTTP-01) | ✓ VERIFIED | Untouched by this gap-closure round or the fresh review. Test evidence: `make test` (cargo nextest, `--features full`) green — 2184 run / 2184 passed / 2 skipped, exit 0 (14 earlier failures conclusively environmental: disk-exhaustion-corrupted `target/` + macOS keychain `ioErr -36`, panic site predates Phase 113). |
| 2 | A handler returns `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd; a client retry carrying `inputResponses` + the echoed `requestState` resumes the operation correctly (HTTP-02, HTTP-03) | ✓ VERIFIED | Untouched by this round or the fresh review. Same green full-suite evidence above (`v2_mrtr` scenarios included in the 2184). |
| 3 | On the v2 path `resources/subscribe`/`unsubscribe` and the GET stream are removed, notifications arrive over a **collision-safe, memory-bounded** `subscriptions/listen` stream, and the client half (`Client::subscriptions_listen`) works **correctly, including ordinary reconnects** (HTTP-04) | ✗ **FAILED** | Registry collision-safety and the newline-carrying SSE flood bound are now genuinely fixed (independently re-read, both hold). The ordinary-reconnect regression is resolved by an evidenced architectural decision (retryable code + structurally-immune client id minting). **But** three independently-confirmed BLOCKER defects remain in the same subsystem: an uncapped whole-body read on the listen client's own rejection path (CR-01), a quadratic-time UTF-8 decoder reachable on the listen stream with no self-limiting behavior (CR-02, live CPU-exhaustion vector), and a sibling uncapped whole-body read on a related transport in the same file this round hardened (CR-03). All three independently reproduced by direct source read, not by trusting `113-REVIEW.md`. See Gap in frontmatter. |
| 4 | SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response ids are always derived from the live request (HTTP-05) | ✓ VERIFIED | Untouched by this round or the fresh review. Covered by the same green full-suite evidence above. |
| 5 | The pmcp `Client` speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfills MRTR `input_required` by producing `inputResponses`, folding the Phase-106 host handlers (sampling/elicitation/roots) into the v2 flow (CLNT-01, CLNT-02) | ✓ VERIFIED | The MRTR-proper client loop is unaffected by this round; the `SubscriptionStream`'s shared `SseParser` dependency inherits truth 3's sub-mechanism status (already captured there so it is not double-counted). Same green full-suite evidence above (`v2_subscriptions_client`, `v2_mrtr` scenarios included). |

**Score:** 4/5 truths verified

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| HTTP-01 | 113-04 | v2 stateless era gate, no session/handshake | SATISFIED* | `sessions_active` gate + `tests/v2_stateless_http.rs`, part of the full green suite |
| HTTP-02 | 113-02/03/06/09/11 | `input_required` + AEAD `requestState` | SATISFIED* | `src/server/request_state.rs`, `tests/v2_mrtr.rs`, part of the full green suite |
| HTTP-03 | 113-02/06/09/11 | Client retry resumes via echoed `requestState` | SATISFIED* | `client_server_mrtr_three_rounds`, `sep_2322_request_state_incomplete_then_complete`, part of the full green suite |
| HTTP-04 | 113-10/13/14/15/16/17/18/19/20 | `subscriptions/listen` stream, retirement of legacy RPCs | **BLOCKED** | Concurrent-collision safety and the SSE-buffer bound are now genuinely fixed (113-17, verified); the reconnect regression is resolved by an evidenced architectural decision (113-18, verified). **Newly BLOCKED for a third, independent reason**: CR-01/CR-02/CR-03, three fresh-review defects independently confirmed present in the current source, unrelated to and unclosed by any of the four gap-closure plans |
| HTTP-05 | 113-08 | SSE resumability off on v2, id-replay regression | SATISFIED* | `tests/v2_stateless_http.rs` id-invariant tests, part of the full green suite |
| CLNT-01 | 113-05/13 | Client speaks v2 | SATISFIED* | `src/client/mod.rs` `with_protocol_version` etc.; part of the full green suite |
| CLNT-02 | 113-07/11 | Client fulfills MRTR via Phase-106 host handlers | SATISFIED* | `fold_input_requests` → `self.host_registry` (Phase-106 `ClientHostRegistry`); part of the full green suite |

\* All seven requirements carry `[~]` (implemented, pending final schema) in `.planning/REQUIREMENTS.md`
under the recorded `113-SPEC-RECHECK.md` exception (re-confirmed: no `schema/2026-07-28` directory
exists upstream as of this re-verification). This is a correctly and honestly disclosed EXTERNAL
blocking factor and is not treated as an independent gap by this verification. **HTTP-04 is
additionally BLOCKED for a codebase reason** — the three fresh-review defects above — that is
unrelated to the schema-pending caveat and will not resolve when the final schema publishes.

No orphaned requirements: REQUIREMENTS.md's Phase 113 row set (HTTP-01..05, CLNT-01, CLNT-02) exactly
matches the seven IDs declared across the plans' frontmatter (113-01..113-20; the four gap-closure
plans 113-17/18/19/20 declare `requirements: [HTTP-04]` or none, consistent with them targeting
Success Criterion 3 and general hardening only).

### Required Artifacts (delta from previous verification; full baseline list unchanged)

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/shared/sse_parser.rs` (`SseParser::feed`) | Unconditionally-bounded SSE decoder | ✓ VERIFIED (for the specific defect the prior verification found) | Independently re-read lines 365-419: the `contains('\n')` escape is gone; the pre-check covers `buffered_bytes() + data.len()` unconditionally, with a second independently-sufficient post-drain check. |
| `src/shared/sse_parser.rs` (`take_utf8_prefix`) | O(n) UTF-8 prefix decoder | ✗ **DEFECT (CR-02, newly found)** | Lines 164-189, unchanged from the review's description: `buffer.drain()` inside the per-invalid-byte-run loop is O(n) per drain, O(n^2) total on a run of invalid bytes. Runs before every byte-count bound this phase added. |
| `src/server/subscriptions.rs` (`ListenRegistry`) | Collision-safe, contract-compliant `subscriptions/listen` registry | ✓ VERIFIED | `code()` (lines 431-434) maps all three rejection variants to `RATE_LIMITED`; `v2_status_for_code` (streamable_http_server.rs:690-702) confirms this resolves to HTTP 200, not 400. Independently re-read. |
| `src/client/subscriptions.rs` (`rejection_error`) | Capped body read on the listen client's rejection path | ✗ **DEFECT (CR-01, newly found)** | Lines 147-166, `body.collect().await` with no cap, unchanged from the review's description. |
| `src/shared/http.rs` (`HttpTransport::send_request`) | Capped response body read | ✗ **DEFECT (CR-03, newly found)** | Lines 346-356, `response.collect().await...to_bytes()` with no cap, unchanged from the review's description, in the same file whose sibling `connect_sse` reader 113-17 capped. |
| `src/shared/streamable_http.rs` (`collect_body_within_cap`) | Capped whole-body reads on `StreamableHttpTransport` | ✓ VERIFIED | `DEFAULT_MAX_COLLECTED_BODY_BYTES` present (line 325); this specific transport's three named `response.collect()` sites are capped per 113-20 — confirmed narrowly TRUE, but the transport's own rustdoc claim that this covers "every" response read on "this transport" family is contradicted by CR-01/CR-03 on the sibling client/transport files. |
| `fuzz/fuzz_targets/subscription_listen_frames.rs` | A libFuzzer campaign asserting a real, falsifiable bound invariant | ✓ VERIFIED | 113-19's `113-FUZZ-EVIDENCE.md` § Campaign 2 records a shown-crashing invariant plus a 20,000-run clean campaign; not independently re-run here (nightly toolchain cost) but the evidence is specific and checkable. |

### Key Link Verification (delta)

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `SseParser::feed` | `max_buffer_size` | unconditional pre-check + post-drain residual check | ✓ WIRED | Read at lines 391 and 412; no escape-hatch condition remains. |
| `ListenRejection::code` | `v2_status_for_code` | `RATE_LIMITED` falls to the `_ => StatusCode::OK` arm | ✓ WIRED | Read at subscriptions.rs:431-434 and streamable_http_server.rs:690-702. |
| `Client::subscriptions_listen` | fresh subscription id | `Uuid::new_v4()` per call | ✓ WIRED (per 113-18's summary, not independently re-run this pass) | Documented and pinned by a live tripwire per 113-18-SUMMARY.md; consistent with the retryable-refusal design read above. |
| `EventStreamTransport::open_event_stream` (rejection branch) | body cap | **none** | ✗ **NOT WIRED** | `rejection_error` (client/subscriptions.rs:147-166) reads the whole body with `body.collect().await` and no size limit — CR-01. |
| `SseParser::feed` / incremental feeders | `take_utf8_prefix` | called before any byte-bound applies | ⚠️ **DEFECTIVE (CPU, not memory)** | `take_utf8_prefix` (sse_parser.rs:164-189) is quadratic on invalid-byte runs and is not itself bounded by any of this phase's three new byte constants — CR-02. |
| `HttpTransport::send_request` | body cap | **none** | ✗ **NOT WIRED** | `response.collect().await` (http.rs:346-356) has no cap — CR-03. |

### Behavioral Spot-Checks (this verification's independent reproductions)

| Behavior | Method | Result | Status |
|---|---|---|---|
| `SseParser::feed`'s bound is unconditional (no newline escape) | Direct source read of `src/shared/sse_parser.rs:365-419` | Pre-check covers `buffered_bytes() + data.len()` with no `contains('\n')` gate anywhere in the block; comment explicitly documents the deleted escape and cites the prior 899,999-byte reproduction | ✓ CONFIRMS gap item 3 (prior verification) is closed |
| `ListenRejection::code()` maps all three variants to a retryable code | Direct source read of `src/server/subscriptions.rs:396-434` and `src/server/streamable_http_server.rs:690-702` | `RATE_LIMITED` for all three; absent from the 400 status arm, resolves to HTTP 200 | ✓ CONFIRMS the 113-14 regression is resolved |
| `rejection_error` reads the whole body with no cap | Direct source read of `src/client/subscriptions.rs:147-166` | `let collected = match body.collect().await { ... }` — no size check anywhere before or during the read | ✗ **CONFIRMS CR-01, still open** |
| `take_utf8_prefix` remains O(n^2) on invalid bytes | Direct source read of `src/shared/sse_parser.rs:164-189` | `buffer.drain(..valid_up_to + invalid_len)` inside the per-error-iteration loop, unchanged from the review's cited implementation | ✗ **CONFIRMS CR-02, still open** |
| `take_utf8_prefix` is reachable from the `subscriptions/listen` client stream | `grep -n "take_utf8_prefix" src/client/subscriptions.rs` | Match at line 237 inside `read_next_frame`, the incremental frame decoder for `subscriptions/listen` | ✗ **Confirms CR-02 is in-scope for HTTP-04's "memory/resource-bounded" claim, not merely a general transport concern** |
| `HttpTransport::send_request` reads the whole body with no cap | Direct source read of `src/shared/http.rs:346-356` | `response.collect().await...to_bytes()` — no size check | ✗ **CONFIRMS CR-03, still open** |
| Debt-marker scan across the five files this verification examined | `grep -nE "TODO\|FIXME\|XXX\|TBD"` on `sse_parser.rs`, `http.rs`, `streamable_http.rs`, `client/subscriptions.rs`, `server/subscriptions.rs` | no matches | ✓ PASS |
| Full test suite | `make test` (cargo nextest, `--features full`), per task instructions | 2184 tests run: 2184 passed, 2 skipped, exit 0 | ✓ PASS (accepted as reported; earlier 14 failures conclusively environmental, not re-litigated per task instructions) |

### Probe Execution

Step 7c: SKIPPED (no `scripts/*/tests/probe-*.sh` conventional probes exist in this repo, and no PLAN/SUMMARY in this phase declares one). The libFuzzer campaign for `subscription_listen_frames` (113-19, Campaign 2) is evidenced in `113-FUZZ-EVIDENCE.md` and was not independently re-run here (nightly-toolchain build cost); its documentary evidence is specific enough (commit SHA, seed, counters, a shown-crashing negative control, artifacts-empty proof) to accept without re-execution.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `src/client/subscriptions.rs` | 147-166 | `rejection_error` collects an unbounded peer-controlled body with no cap | 🛑 Blocker (CR-01, newly found, independently confirmed) | A hostile or broken server answering `subscriptions/listen` with a non-`text/event-stream` response and a large/chunked body can drive unbounded client allocation on the exact stream HTTP-04 exists to harden |
| `src/shared/sse_parser.rs` | 164-189 | `take_utf8_prefix` is O(n^2) via a per-invalid-byte-run `Vec::drain` | 🛑 Blocker (CR-02, newly found, independently confirmed) | Reachable via `subscriptions/listen`'s `read_next_frame` (client/subscriptions.rs:237) and `HttpTransport::connect_sse` (http.rs:270); a remote CPU-exhaustion vector unaffected by any of this phase's three new byte-count bounds, not self-limiting when garbage frames are newline-terminated |
| `src/shared/http.rs` | 346-356 | `HttpTransport::send_request` collects an unbounded peer-controlled response body | 🛑 Blocker (CR-03, newly found, independently confirmed) | In the same file whose sibling SSE reader (`connect_sse`) 113-17 bounded; directly contradicts the "every one of this transport's response reads is capped" framing this round's own rustdoc uses for the sibling transport |
| `src/shared/streamable_http.rs` / `src/server/subscriptions.rs` | various | Twelve WARNING-severity findings recorded in `113-REVIEW.md` (WR-01 through WR-12) — e.g. `RATE_LIMITED` now covering three distinct conditions discriminated only by a message substring (WR-02), stale server-side doc comments contradicting the shipped 113-18 mapping (WR-03), a public `SseParser::feed` that silently discards data with no error channel (WR-04) | ⚠️ Warning (not independently re-verified line-by-line this pass; recorded for completeness, not blocking this verdict) | Would degrade third-party-client and maintainer experience but do not by themselves falsify a Success Criterion the way the three Blockers do |
| `113-14-SUMMARY.md` / `113-15-SUMMARY.md` (carried forward from prior verification) | — | Both SUMMARYs' original "fully closed" framing for their respective gap items is now accurate for the SSE-buffer item (113-15/113-17 combined) and for the reconnect item (113-14/113-18 combined) | ℹ️ Info (verifier-added) | No longer a discrepancy — closure required the follow-on plans, and they landed |

### Human Verification Required

None. All findings in this re-verification are mechanically verifiable and were independently
confirmed by reading the current source directly (not by trusting `113-REVIEW.md`'s prose or any
SUMMARY.md's claims).

### Gaps Summary

The four gap-closure plans (113-17, 113-18, 113-19, 113-20) genuinely closed both items the
prior `113-VERIFICATION.md` left open:

1. **The unbounded SSE client buffer (prior gap item 3) is closed.** `SseParser::feed`'s bound is
   now unconditional over `buffer + current_event.data + chunk`, independently re-read with no
   escape-hatch condition remaining. The exact 899,999-byte reproduction the prior verification
   recorded is now a passing regression test, and I independently confirmed the fix by reading the
   current implementation rather than trusting the plan's summary.
2. **The ordinary-reconnect regression (introduced by 113-14, found by the prior verification) is
   resolved.** Rather than reintroducing the originally-planned liveness reclaim — which 113-18
   demonstrates is architecturally unimplementable at the server's current layering (the receiver
   and the RAII guard share one `stream::unfold` state tuple, so remote-peer death is genuinely
   unobservable there) — the fix makes the refusal retryable (`RATE_LIMITED` at HTTP 200 instead of
   `INVALID_REQUEST` at HTTP 400) and pairs it with a structural guarantee that pmcp's own client
   never reuses a subscription id (fresh `Uuid::new_v4()` per call, pinned by a tripwire). This is a
   principled, evidenced resolution, not a scope cut, and I independently confirmed both halves by
   reading the current source.

However, Success Criterion 3 (HTTP-04) is **still not fully achieved**, because a same-day fresh
code review (`113-REVIEW.md`, committed `57aad7bc`) found three new BLOCKER-severity defects in
the same subsystem, and I independently confirmed all three are still present by reading the
current source myself — not by trusting the review's prose:

1. **CR-01** — `src/client/subscriptions.rs:147-166`'s `rejection_error` reads a peer-controlled
   response body with `body.collect().await` and no size cap, on the `subscriptions/listen`
   client's own non-stream rejection path. This is precisely the defect class HTTP-04's "memory-
   bounded" language exists to prevent, on the very stream the criterion names.
2. **CR-02** — `src/shared/sse_parser.rs:164-189`'s `take_utf8_prefix` is quadratic on invalid
   UTF-8 byte runs (measured by the review at ~1.17 CPU-seconds per 400 KiB of garbage input) and
   runs before any of this phase's three new byte-count bounds apply. It is directly reachable from
   the `subscriptions/listen` client's frame decoder (`read_next_frame`, confirmed by `grep` at
   `client/subscriptions.rs:237`) and is not self-limiting when a peer newline-terminates each
   garbage frame.
3. **CR-03** — `src/shared/http.rs:346-356`'s `HttpTransport::send_request` collects an unbounded
   peer-controlled response body with no cap at all, in the same file whose sibling `connect_sse`
   SSE reader 113-17 explicitly bounded in this very gap-closure round.

None of these three is a regression caused by 113-17/18/19/20's own edits — CR-02 and CR-03 both
predate this round, and CR-01 was never inside the scope any of the four plans declared. But they
are real, currently-shipping defects in the exact subsystem (subscriptions/listen client, and its
sibling HTTP transport) whose hardening was this round's explicit purpose, and they directly
falsify the round's own rustdoc claim (`DEFAULT_MAX_COLLECTED_BODY_BYTES`'s doc, quoted in
`113-REVIEW.md`) that "every one of this transport's response reads is a whole-body read... and
the peer chooses how many bytes it sends" is now handled.

The remaining requirements and truths (HTTP-01/02/03/05, CLNT-01/02) are unaffected by any of this
round's changes or the fresh review's findings, and remain genuinely, substantively achieved — the
full test suite (2184/2184, per the task's supplied evidence) covers their regression surface.

**Recommended next step:** a further Phase-113 gap-closure plan (or a fifth wave of this same
round) that (a) routes `rejection_error`'s body read through a capped helper mirroring
`StreamableHttpTransport::collect_body_within_cap`, (b) rewrites `take_utf8_prefix` as a
single-pass O(n) decoder with a deterministic shape-pinning regression and a `-timeout=1` fuzz re-
run, and (c) caps `HttpTransport::send_request`'s response body the same way `connect_sse`'s
reader was capped in the same file — all three fixes are already specified with working code in
`113-REVIEW.md`'s CR-01/CR-02/CR-03 sections. Until then, Success Criterion 3 (HTTP-04) should
remain marked not-fully-achieved independent of the separate, disclosed publication-block gate.

---

_Verified: 2026-07-26T21:40:00Z_
_Verifier: Claude (gsd-verifier)_
