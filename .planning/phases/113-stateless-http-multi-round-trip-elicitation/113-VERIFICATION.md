---
phase: 113-stateless-http-multi-round-trip-elicitation
verified: 2026-07-26T20:24:46Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/5 must-haves verified
  gaps_closed:
    - "Gap item 1: ListenRegistry::register no longer blind-inserts on a duplicate (principal, RequestId) key — a LIVE duplicate is now refused with a typed ListenRejection::DuplicateSubscriptionId instead of evicting the incumbent (113-14, independently confirmed: entries.write() guard shared by occupancy-check + insert; two_callers/duplicate tests pass)"
    - "Gap item 2: ListenGuard::drop and the overflow-disconnect path are now generation-scoped (ListenEntry.generation / ListenGuard.generation / ListenRegistry.next_generation), so a late guard drop or a stale overflow disconnect can never reclaim a healthy successor at the same key (113-14, independently confirmed by reading remove_entry:725-730 and disconnect_overflowed:674-698, and by the passing a_guard_drop_cannot_reclaim_a_successor_at_the_same_key / a_stale_overflow_disconnect_cannot_evict_a_successor unit tests)"
    - "Gap item 4: the previously-untested same-principal id-reuse path now has a live-HTTP regression test (tests/v2_subscriptions.rs::same_principal_id_reuse_rejects_the_second_and_spares_the_first), independently re-run and passing (10/10 in tests/v2_subscriptions.rs)"
    - "Gap item 5: an actual libFuzzer campaign (not just cargo check) has now RUN against subscription_listen_frames — 20 000 runs, exit 0, fuzz/artifacts/subscription_listen_frames/ empty, recorded reproducibly in 113-FUZZ-EVIDENCE.md with commit SHA, toolchain, seed, and a branch-coverage proof (113-16)"
  gaps_remaining:
    - "Gap item 3 (SSE unbounded buffer) is NOT closed. SseParser::feed's bound check at src/shared/sse_parser.rs:249-261 is gated on '!self.buffer.contains(\"\\n\") && !data.contains(\"\\n\")', so ANY chunk carrying a newline bypasses it entirely, and EventBuilder::data (current_event.data, lines 329-336) accumulates across data: lines with NO bound of any kind. Independently reproduced against the built crate: feed(\"data: AAAAAAAA\\n\") x100,000 under with_max_buffer_size(64) accumulates 899,999 bytes with overflowed()==false; a single data: line of 1,000,000 bytes delivered with its terminating newline is accepted whole with overflowed()==false. max_buffer_size is therefore not an upper bound, and overflowed() cannot observe the condition it exists to detect. 113-15 closed only the narrow 'peer never sends ANY newline' case; the code review (113-REVIEW.md CR-01/CR-02) found the same defect and I reproduced both of its measurements exactly (899,999 and the 1,000,000-byte accept)."
  regressions:
    - "NEW regression introduced by 113-14 (code review CR-03, independently reproduced): ListenRegistry::register's duplicate check (src/server/subscriptions.rs:559-562, 'if entries.contains_key(&key)') asks only whether the key is OCCUPIED, never whether the incumbent's receiver is ALIVE. An ordinary client reconnect reusing the same subscription id after an ungraceful disconnect (mobile handoff, NAT rebind, LB reap, SIGKILL) is refused -32600 INVALID_REQUEST at HTTP 400 -- a non-retryable code for a transient server-state condition -- for the whole window until the ~15s keep-alive write fails and the stale ListenGuard finally unwinds. I reproduced this live with a temporary in-tree probe (open() a stream, drop its receiver to simulate a vanished client, then attempt to re-open the same (principal, id) while the guard is still alive): the second open() returned Err(DuplicateSubscriptionId). The probe was reverted after use (git status confirms a clean tree)."
gaps:
  - truth: "On the v2 path, subscriptions/listen delivers change notifications through a collision-safe, memory-bounded long-lived stream, and the client half (Client::subscriptions_listen) works correctly against it, including ordinary reconnects (HTTP-04, Success Criterion 3)"
    status: partial
    reason: >-
      Three of the five previously-identified gap items are now genuinely closed (concurrent
      duplicate-registration collision safety, generation-scoped teardown, and an actual
      libFuzzer campaign). But the fourth gap item -- the unbounded client-side SSE buffer on
      the long-lived subscriptions/listen consumer -- remains open: the bound added by 113-15
      only fires when the incoming chunk contains no newline, so a peer that streams ordinary
      newline-terminated data: lines forever (the common case, not an edge case) still grows
      the client's heap without limit. In addition, closing the first three gap items introduced
      a new correctness regression: the duplicate-registration refusal that now protects against
      the original eviction bug also refuses an ORDINARY client reconnect (no malicious or
      duplicate intent at all) with a non-retryable HTTP 400 for up to ~15 seconds after an
      ungraceful disconnect. Both defects were independently reproduced against the built crate,
      not just read from the code review.
    artifacts:
      - path: "src/shared/sse_parser.rs"
        issue: "feed()'s bound check (lines 249-261) is skipped whenever either the buffer or the incoming chunk contains a newline; EventBuilder::data (lines 329-336) has no bound at all. Confirmed: 100,000x feed(\"data: AAAAAAAA\\n\") under a 64-byte bound accumulates 899,999 bytes with overflowed()==false."
      - path: "src/server/subscriptions.rs"
        issue: "register()'s duplicate check (lines 559-562) tests occupancy, not liveness -- entries.contains_key(&key) with no check on whether the incumbent's mpsc::Sender is closed. An ordinary reconnect after an ungraceful disconnect is refused as a duplicate. Confirmed with a live reverted probe: drop(receiver) then re-open the same key returns Err(DuplicateSubscriptionId)."
    missing:
      - "Bound the accumulated event (current_event.data), not only the line buffer -- e.g. check buffer.len() + current_event.data.len() + data.len() against max_buffer_size unconditionally (dropping the contains('\\n') escape), with an explicit bypass for the two whole-body feed call sites that legitimately need to accept a large complete body."
      - "A regression test that floods the parser with NEWLINE-CARRYING data (not just newline-free flood) and asserts overflowed() eventually latches."
      - "Treat a closed incumbent sender as a free key in ListenRegistry::register (e.g. entries.get(&key).is_some_and(|e| e.sender.is_closed()) => reclaim; else duplicate) so an ordinary reconnect is not confused with a live duplicate."
      - "A regression test that closes the receiver of a live entry, leaves its ListenGuard alive, and asserts a re-registration on the same key succeeds (the reconnect case), distinct from the existing duplicate_key_is_rejected_and_the_first_stream_survives test which never drops the receiver."
human_verification: []
---

# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation Verification Report

**Phase Goal:** v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto pmcp's existing `stateless()` branch (not a transport fork); multi-round-trip elicitation works end-to-end; and the pmcp `Client` is the v2-speaking counterpart, folding the Phase-106 host handlers into the v2 flow. v1 session behavior is untouched.
**Verified:** 2026-07-26T20:24:46Z
**Status:** gaps_found
**Re-verification:** Yes — after gap-closure plans 113-14, 113-15, 113-16 (three of the five prior gap items closed; one remains open; one new regression found)

## Publication-block context (not a codebase gap)

ROADMAP.md marks the phase `[~]` (implemented, not `[x]` complete) and REQUIREMENTS.md carries
all seven requirement IDs (HTTP-01..05, CLNT-01, CLNT-02) as `[~]` "Implemented — pending final
schema". This is a disclosed, human-granted exception recorded in `113-SPEC-RECHECK.md`: as of
this re-verification (2026-07-26) `schema/2026-07-28` still does not exist upstream (`draft` was
used instead), so the three v2 error-code constants are pinned to pre-final values under a
written developer exception. This verification does **not** treat the unflipped checkboxes as a
gap and does **not** recommend flipping them — that decision has its own binding
re-verification date already on file and is outside this phase's control.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A v2 HTTP request completes with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch, while v1 session behavior is unchanged (HTTP-01) | ✓ VERIFIED | Untouched by this gap-closure round. Re-ran `cargo test --test v2_stateless_http --features full`: 23/23 pass, including `v1_session_unchanged`, `v2_requires_no_session_id`, `response_id_always_from_live_request`. |
| 2 | A handler returns `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd; a client retry carrying `inputResponses` + the echoed `requestState` resumes the operation correctly (HTTP-02, HTTP-03) | ✓ VERIFIED | Untouched by this gap-closure round. Re-ran `cargo test --test v2_mrtr --features full`: 27/27 pass (all `client_server_mrtr_*` and `sep_2322_*` scenarios), real Client↔Server HTTP round trips. |
| 3 | On the v2 path `resources/subscribe`/`unsubscribe` and the GET stream are removed, notifications arrive over a **collision-safe, memory-bounded** `subscriptions/listen` stream, and the client half (`Client::subscriptions_listen`) works **correctly, including ordinary reconnects** (HTTP-04) | ✗ **FAILED** | Retirement mechanism still sound and unaffected. Registry collision-safety for CONCURRENT duplicate registrations is now genuinely fixed (generation-scoped teardown, independently verified). But the client-side SSE buffer bound remains unenforced for the common case (newline-carrying floods), independently reproduced at 899,999 bytes accumulated under a 64-byte bound with `overflowed()==false`; and the fix for the registry collision introduced a new regression where an ordinary reconnect is refused as a duplicate, independently reproduced live. See Gap in frontmatter. |
| 4 | SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response ids are always derived from the live request (HTTP-05) | ✓ VERIFIED | Untouched by this gap-closure round. Covered by the same `v2_stateless_http` 23/23 pass above (`last_event_id_ignored` class tests). |
| 5 | The pmcp `Client` speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfills MRTR `input_required` by producing `inputResponses`, folding the Phase-106 host handlers (sampling/elicitation/roots) into the v2 flow (CLNT-01, CLNT-02) | ✓ VERIFIED | Untouched by this gap-closure round except for `SubscriptionStream`'s shared `SseParser` dependency (see truth 3 for that sub-mechanism's own status). Re-ran `cargo test --test v2_subscriptions_client --features full`: 7/7 pass. MRTR-proper client loop unaffected — `v2_mrtr` 27/27 above. |

**Score:** 4/5 truths verified

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| HTTP-01 | 113-04 | v2 stateless era gate, no session/handshake | SATISFIED* | `sessions_active` gate + `tests/v2_stateless_http.rs` (23/23 pass, re-run this verification) |
| HTTP-02 | 113-02/03/06/09/11 | `input_required` + AEAD `requestState` | SATISFIED* | `src/server/request_state.rs`, `tests/v2_mrtr.rs` (27/27 pass, re-run this verification) |
| HTTP-03 | 113-02/06/09/11 | Client retry resumes via echoed `requestState` | SATISFIED* | `client_server_mrtr_three_rounds`, `sep_2322_request_state_incomplete_then_complete` (both re-run, pass) |
| HTTP-04 | 113-10/13/14/15/16 | `subscriptions/listen` stream, retirement of legacy RPCs | **BLOCKED** | Concurrent-collision safety now genuinely fixed (113-14, verified). Buffer-bound and reconnect-liveness defects remain (113-15 incomplete; 113-14 introduced a new regression) — both independently reproduced live against the built crate |
| HTTP-05 | 113-08 | SSE resumability off on v2, id-replay regression | SATISFIED* | `tests/v2_stateless_http.rs` id-invariant tests (re-run, pass) |
| CLNT-01 | 113-05/13 | Client speaks v2 | SATISFIED* | `src/client/mod.rs` `with_protocol_version` etc.; `client_server_mrtr_no_session_no_handshake` (pass) |
| CLNT-02 | 113-07/11 | Client fulfills MRTR via Phase-106 host handlers | SATISFIED* | `fold_input_requests` → `self.host_registry` (Phase-106 `ClientHostRegistry`); `client_server_mrtr_*` (pass) |

\* All seven requirements carry `[~]` (implemented, pending final schema) in `.planning/REQUIREMENTS.md`
under the recorded `113-SPEC-RECHECK.md` exception (re-confirmed on 2026-07-26: no `schema/2026-07-28`
directory exists upstream). This is a correctly and honestly disclosed EXTERNAL blocking factor and is
not treated as an independent gap by this verification. **HTTP-04 is additionally BLOCKED for a second,
independent reason** — the confirmed correctness defects above — that is unrelated to the schema-pending
caveat and will not resolve when the final schema publishes.

No orphaned requirements: REQUIREMENTS.md's Phase 113 row set (HTTP-01..05, CLNT-01, CLNT-02) exactly
matches the seven IDs declared across the plans' frontmatter (113-01..113-16; the three gap-closure plans
113-14/15/16 all declare `requirements: [HTTP-04]`, consistent with them targeting Success Criterion 3
only).

### Required Artifacts (delta from previous verification; full baseline list unchanged)

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/server/subscriptions.rs` (`ListenRegistry`) | Collision-safe AND liveness-aware `subscriptions/listen` registry | ⚠️ **PARTIAL** | Concurrent duplicate-registration collision now genuinely fixed (generation-scoped `remove_entry`/`disconnect_overflowed`, independently read and confirmed at lines 674-698, 715-730). Duplicate check (lines 556-562) is occupancy-only, not liveness-aware — an ordinary reconnect is refused as a duplicate (new regression, independently reproduced) |
| `src/shared/sse_parser.rs` | Bounded SSE decoder | ✗ **DEFECT UNCLOSED** | `SseConfig::max_buffer_size` is now read (no longer fully dead code, `SseParser::new()` sources it), but the bound only fires when the chunk carries no newline; `EventBuilder::data` (the actual accumulator for a multi-line event) has zero bound. Independently reproduced: 899,999 bytes accumulated under a 64-byte bound with `overflowed()==false` |
| `fuzz/fuzz_targets/subscription_listen_frames.rs` | An actually-run libFuzzer campaign | ✓ VERIFIED | `113-FUZZ-EVIDENCE.md` records a real 20,000-run campaign (commit SHA, toolchain, seed, artifacts-empty proof); not independently re-run in this verification (nightly toolchain, ~1 min build) but the evidence is specific and checkable, and the file/commit references resolve |
| `tests/v2_subscriptions.rs` | Live regression test for same-principal id reuse | ✓ VERIFIED | `same_principal_id_reuse_rejects_the_second_and_spares_the_first` exists and passes (10/10 in the suite, re-run this verification) |

### Key Link Verification (delta)

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `ListenRegistry::register` | occupancy check + insert | one shared `entries.write()` guard | ✓ WIRED | Read at src/server/subscriptions.rs:556-574; confirmed no check-then-act window for CONCURRENT registrations |
| `ListenGuard::drop` / `disconnect_overflowed` | `ListenRegistry::remove_entry` | generation-scoped comparison | ✓ WIRED | Read at lines 674-698 (`disconnect_overflowed`) and 715-730 (`remove_entry`); both compare `generation` before removing |
| `SseParser::feed` | `SseConfig::max_buffer_size` | bound check in `feed` | ⚠️ PARTIAL | Wired, but the check is bypassed whenever either operand contains a newline — the common case for real SSE traffic, not an edge case |
| `subscriptions/listen` reconnect | `ListenRegistry::register`'s duplicate check | `entries.contains_key(&key)` | ✗ **DEFECTIVE** | No liveness check — an ordinary reconnect after an ungraceful disconnect collides with the SAME logic meant to reject a malicious/accidental duplicate |

### Behavioral Spot-Checks (this verification's independent reproductions)

| Behavior | Command | Result | Status |
|---|---|---|---|
| Newline-carrying SSE flood grows the parser unbounded | temporary `#[test]` in `tests/tmp_verify_sse_bound.rs` (added, run, reverted — `git status` clean after): `SseParser::with_max_buffer_size(64)`, `feed("data: AAAAAAAA\n")` x100,000, then a dispatch | `dispatched event data len = 899999`, `overflowed = false` | ✗ **CONFIRMS gap item 3 still open (review CR-01)** — exact match to the review's independent measurement |
| A single oversized complete SSE line is accepted whole | same temp file, `feed("data: " + "B"x1,000,000 + "\n\n")` under a 64-byte bound | 1 event, `data.len() == 1_000_000`, `overflowed() == false` | ✗ **CONFIRMS review CR-02** |
| Ordinary reconnect after receiver drop is refused as a duplicate | temporary `#[tokio::test]` inserted into `src/server/subscriptions.rs`'s existing `entry_ownership` test module (added, run, reverted — `git diff`/`git status` clean after via `git checkout --`): open a stream, `drop(receiver)` (simulating a vanished client, guard still alive), then `open()` the SAME `(principal, id)` again | `reconnect result = Some(DuplicateSubscriptionId)` — assertion failed as expected, proving the reconnect is refused | ✗ **CONFIRMS review CR-03, a new regression from 113-14** |
| `tests/v2_subscriptions.rs` (full suite, incl. the new gap-4 regression test) | `cargo test --test v2_subscriptions --features full` | 10 passed; 0 failed | ✓ PASS |
| `tests/v2_subscriptions_client.rs` | `cargo test --test v2_subscriptions_client --features full` | 7 passed; 0 failed | ✓ PASS |
| `sse_parser` unit tests | `cargo test --lib --features full -- sse_parser` | 20 passed; 0 failed | ✓ PASS (green suite — expected, since every bounding test only feeds newline-free chunks, per review IN-03) |
| `v2_stateless_http` (regression check, truths 1/4) | `cargo test --test v2_stateless_http --features full` | 23 passed; 0 failed | ✓ PASS |
| `v2_mrtr` (regression check, truths 2/5) | `cargo test --test v2_mrtr --features full` | 27 passed; 0 failed | ✓ PASS |
| Debt-marker scan across all seven gap-closure-touched files | `grep -n -E "TBD\|FIXME\|XXX"` on each of the seven files in the `d3b54221..HEAD` diff | no matches | ✓ PASS |
| `cargo clippy --lib --tests --features full` | full workspace clippy | clean, zero warnings | ✓ PASS |

### Probe Execution

Step 7c: SKIPPED (no `scripts/*/tests/probe-*.sh` conventional probes exist in this repo, and no PLAN/SUMMARY in this phase declares one). The libFuzzer campaign for `subscription_listen_frames` (113-16) is evidenced in `113-FUZZ-EVIDENCE.md` and was not independently re-run here (nightly-toolchain build cost); its documentary evidence is specific enough (commit SHA, seed, counters, artifacts-empty proof with exact commands) to accept without re-execution, per the Behavioral Spot-Checks above having already independently confirmed the underlying defect the campaign was meant to help catch.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `src/shared/sse_parser.rs` | 249-261, 329-336 | Bound check skipped whenever a newline is present; `current_event.data` has no bound at all | 🛑 Blocker | Unbounded client memory growth on the long-lived `subscriptions/listen` consumer for ordinary (newline-carrying) traffic — folded into the Gap above (HTTP-04) |
| `src/server/subscriptions.rs` | 559-562 | Duplicate-registration check tests occupancy, not liveness | 🛑 Blocker (new, introduced by 113-14) | An ordinary client reconnect is refused with a non-retryable `-32600`/HTTP 400 for up to the ~15s keep-alive window — folded into the Gap above (HTTP-04) |
| `src/server/subscriptions.rs` | 552-562 | `register`'s duplicate-rejection early return skips `prune_principal` | ⚠️ Warning | A per-principal semaphore map entry can be orphaned (slow leak bounded by distinct principals, not request volume); review WR-06, independently confirmed `prune_principal` has exactly one caller (`ListenGuard::drop`) |
| `src/client/subscriptions.rs` | 631-696 | `decode_listen_chunk_for_fuzz`/`decode_listen_chunks_for_fuzz` are `pub` (only `#[doc(hidden)]`, not `cfg`-gated) inside a fully public module path | ⚠️ Warning | Real public-API surface widening not declared in 113-16's `files_modified`; review WR-05, independently confirmed via source read |
| `fuzz/fuzz_targets/subscription_listen_frames.rs` | 108-120 | The one fuzz invariant that would catch CR-01/CR-02 (bounded memory) is never asserted; the asserted "latch never clears" invariant is a tautology | ℹ️ Info | The 20,000-run campaign is real (gap item 5 genuinely closed) but structurally cannot have caught the still-open buffer-bound defect; review WR-03 |
| `113-14-SUMMARY.md` / `113-15-SUMMARY.md` | — | Both SUMMARYs claim their respective gap items fully closed | ℹ️ Info (verifier-added) | 113-14's claim holds for gap items 1/2/4 but the fix's interaction with reconnects was not tested (CR-03). 113-15's claim ("Closed verification gap item 3") is CONTRADICTED by evidence — the fix narrows the vulnerability without closing it |

### Human Verification Required

None. All findings in this re-verification are mechanically verifiable and were independently
reproduced against the built crate (not inferred from the code review or from SUMMARY claims).

### Gaps Summary

Three of the five gap items from the previous `113-VERIFICATION.md` are genuinely closed by this
round of gap-closure plans. Plan 113-14's generation-scoped teardown for `ListenRegistry` is sound
engineering — I independently confirmed the occupancy-check-and-insert atomicity, the generation
comparison in both teardown paths, and the passing live regression test for the previously-untested
same-principal path. Plan 113-16's fuzz campaign genuinely ran (20,000 iterations, zero crashes,
specific and checkable evidence) where the prior state was only a `cargo check`.

However, Success Criterion 3 (HTTP-04) remains **not achieved**, for two independently-confirmed
reasons:

1. **Gap item 3 (unbounded SSE client buffer) is not closed.** Plan 113-15's fix only bounds the
   case where a peer never sends a newline at all — an artificial edge case. The realistic
   attack/failure mode (a peer that streams ordinary `data:` lines forever without ever completing
   the event with a blank line) still grows the client's heap without limit, because the bound
   check is skipped whenever a newline is present and the actual accumulator for multi-line event
   data (`EventBuilder::data`) has no bound of any kind. I reproduced the exact byte counts the
   code review measured (899,999 bytes at a 64-byte bound; a single 1,000,000-byte line accepted
   whole), confirming this is not a theoretical concern.

2. **Plan 113-14 introduced a new regression while fixing gap items 1/2/4.** The duplicate-key
   refusal that correctly protects against the original silent-eviction bug does not distinguish
   a genuine live duplicate from an ordinary reconnect after an ungraceful disconnect. I
   reproduced this live: dropping a stream's receiver (simulating a vanished client while its
   `ListenGuard` has not yet unwound) and then reconnecting on the same `(principal, id)` is
   refused with `DuplicateSubscriptionId` — a non-retryable HTTP 400 — even though no duplicate
   stream is actually alive.

Neither defect is deferred to a later milestone phase (Phases 114-119 do not touch this
mechanism), and neither is covered by the phase's disclosed publication-block exception (that
exception covers only the three v2 error-code constants pending the final schema, not these two
correctness defects).

The remaining requirements and truths (HTTP-01/02/03/05, CLNT-01/02) are unaffected by this
round's changes and remain genuinely, substantively achieved — re-confirmed by re-running their
test suites in this verification, not merely by trusting the prior report.

Recommended next step: a further Phase-113 gap-closure plan that (a) bounds `EventBuilder::data`
unconditionally in `SseParser::feed` rather than only `buffer`, dropping the `contains('\n')`
escape (with an explicit bypass constructor for the two whole-body call sites that legitimately
need to accept a large complete body), and (b) makes `ListenRegistry::register`'s duplicate check
liveness-aware (e.g. treat a closed incumbent sender as a free key) so an ordinary reconnect is
never confused with a live duplicate, plus regression tests for both.

---

_Verified: 2026-07-26T20:24:46Z_
_Verifier: Claude (gsd-verifier)_
