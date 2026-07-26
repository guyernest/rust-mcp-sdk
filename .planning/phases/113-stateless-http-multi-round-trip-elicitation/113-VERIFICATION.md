---
phase: 113-stateless-http-multi-round-trip-elicitation
verified: 2026-07-26T04:38:53Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "On the v2 path, subscriptions/listen delivers change notifications through a collision-safe stream keyed on (principal, RequestId), and the client half (Client::subscriptions_listen) works correctly against it (HTTP-04, Success Criterion 3)"
    status: failed
    reason: >-
      ListenRegistry::register (src/server/subscriptions.rs:474) performs an unconditional
      HashMap::insert with no occupancy check, and ListenGuard::drop (line 410, via
      remove_entry at line 595) removes whatever entry currently sits at its key with no
      ownership/generation check. Two connections belonging to the SAME authenticated
      principal that reuse one JSON-RPC id (a realistic case: multiple browser tabs/client
      processes under one auth token, or any deployment where AuthContext::subject is empty/
      constant) silently destroy BOTH listen streams -- the second register() silently evicts
      the first (dropping its mpsc::Sender ends that stream immediately with no terminal frame
      and no overflow notice), and when either guard later drops it removes whatever the
      key currently maps to, which can be the survivor. I independently reproduced this live
      (temporary in-tree probe, reverted after use; see Behavioral Spot-Checks): after a second
      same-key register(), live_streams() reports 1 (not 2) and the first stream's receiver is
      already Disconnected -- before any fan-out even ran.

      This directly contradicts the module's own doc comment (lines 294-300), which claims the
      (principal, RequestId) pair "is the fix" for exactly this collision class (T-113-61) --
      it only closes the CROSS-principal half. Both existing tests that claim to cover this
      (`two_callers_same_request_id_do_not_cross` in tests/v2_subscriptions.rs:662 and
      `two_principals_sharing_request_id_one_do_not_cross` in
      src/server/subscriptions.rs:855) use two DIFFERENT principals (alice/bob), so neither
      exercises the same-principal path that actually fails. 113-10-SUMMARY.md's own claim
      ("(principal, RequestId)-keyed so callers reusing id `1` cannot cross-deliver") is
      unqualified and therefore false as stated -- it is true only across principals.

      A second, independent defect in the same success-criterion's "long-lived stream"
      mechanism: SseParser (src/shared/sse_parser.rs), the shared decoder Phase 113 newly
      wired to a long-lived, remote-fed consumer (client::subscriptions::drain_sse_payloads),
      has NO bound on its internal buffer. `feed()` (line 169) unconditionally
      `self.buffer.push_str(data)`s with no size check, while `SseConfig::max_buffer_size`
      is declared and defaulted to 1 MiB (lines 378, 393) but read nowhere else in the crate
      (grep -rn confirms exactly those two hits). A remote peer that never sends a newline, or
      whose fuzz target only build-checks (`cargo check --bin subscription_listen_frames`,
      113-13-SUMMARY.md line 124) rather than actually running libFuzzer against it, grows
      client memory without bound -- the DESIGNED steady state of an opt-in long-lived stream.
    artifacts:
      - path: "src/server/subscriptions.rs"
        issue: "register() (~line 474) blind-inserts on a duplicate (principal, RequestId) key; remove_entry() (~line 595) removes unconditionally; ListenGuard::drop (~line 410) has no generation/ownership check before calling remove_entry. No ListenRejection variant exists for a duplicate-key registration."
      - path: "src/shared/sse_parser.rs"
        issue: "feed() (line 169) grows `buffer: String` with no cap; SseConfig::max_buffer_size (line 378, defaulted line 393) is dead -- never read anywhere in src/."
    missing:
      - "Reject (or otherwise safely resolve) a duplicate (principal, RequestId) registration in ListenRegistry::register instead of silently overwriting the live entry."
      - "Scope ListenGuard::drop's removal to the entry it actually owns (e.g. a per-entry generation token) so a guard can never reclaim a successor's live entry at the same key -- this also closes the overflow-disconnect race (same root cause, review's CR-02)."
      - "Enforce SseConfig::max_buffer_size (or an equivalent) inside SseParser::feed and surface an overflow signal to the long-lived subscriptions/listen client consumer instead of allowing unbounded growth."
      - "A regression test opening two listen streams for ONE principal reusing one JSON-RPC id, asserting the first stream is not silently destroyed by the second registration."
      - "An actual libFuzzer run (not just `cargo check`) against subscription_listen_frames, matching the rigor already applied to fuzz_request_state (20 000 runs, 0 artifacts)."
human_verification: []
---

# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation Verification Report

**Phase Goal:** v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto pmcp's existing `stateless()` branch (not a transport fork); multi-round-trip elicitation works end-to-end; and the pmcp `Client` is the v2-speaking counterpart, folding the Phase-106 host handlers into the v2 flow. v1 session behavior is untouched.
**Verified:** 2026-07-26T04:38:53Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A v2 HTTP request completes with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch, while v1 session behavior is unchanged (HTTP-01) | ✓ VERIFIED | `sessions_active(state, era)` (src/server/streamable_http_server.rs:433) is the single predicate gating all four session sites (line 447, 1643, 1838, 3012, 3480). Live-HTTP test suite `tests/v2_stateless_http.rs` (23 tests) run and PASSED: `no_session_id_on_v2`, `v2_requires_no_session_id`, `v2_ignores_inbound_session_id`, `v1_session_unchanged`, `v1_get_delete_unchanged`, `v2_delete_405`, `v2_get_405` all green against a STATEFUL default config (proving the gate is per-request era, not build-time). |
| 2 | A handler returns `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd; a client retry carrying `inputResponses` + the echoed `requestState` resumes the operation correctly (HTTP-02, HTTP-03) | ✓ VERIFIED | `src/server/request_state.rs` (1748 lines) mints/verifies an AEAD (`ring` ChaCha20-Poly1305) token with `principal‖method‖param-digest` AAD, TTL, key-id rotation, `Verdict` incl. `Expired(Continuation)`. `tests/v2_mrtr.rs` (28 tests, all passed) covers `sep_2322_request_state_incomplete_then_complete`, `sep_2322_reject_tampered_state`, `client_server_mrtr_three_rounds`, round-trip against a real Client+Server over HTTP with no session/handshake. |
| 3 | On the v2 path `resources/subscribe`/`unsubscribe` and the GET stream are **removed**, notifications arrive over a **collision-safe**, opt-in `subscriptions/listen` stream (`subscriptionId` tagging), and the client half (`Client::subscriptions_listen`) works (HTTP-04) | ✗ **FAILED** | Era-gated retirement of the legacy RPCs is sound and independently confirmed (`dispatch_request_or_retire`, `reject_if_retired_on_v2`, both wired). **But** the collision-safety the mechanism claims does not hold: `ListenRegistry::register`/`ListenGuard::drop` blind-insert/blind-remove with no per-entry ownership check, so two connections from the SAME principal reusing one JSON-RPC id destroy each other's streams -- independently reproduced live (see Behavioral Spot-Checks). `SseParser`, now driving the long-lived client-side listen consumer, has an unbounded buffer (`SseConfig::max_buffer_size` is dead code). See Gap in frontmatter. |
| 4 | SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response ids are always derived from the live request (HTTP-05) | ✓ VERIFIED | `resumability_active` era gate (mirrors `sessions_active`); `envelope_for_live_request(payload, live_id)` scopes the id invariant to direct responses. Tests `last_event_id_ignored`, `v1_resumability_unchanged`, `response_id_always_from_live_request`, `response_id_concurrent_callers_do_not_cross`, `cached_payload_is_reenveloped_with_live_id`, `v1_replayed_event_retains_original_id` all ran and PASSED. |
| 5 | The pmcp `Client` speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfills MRTR `input_required` by producing `inputResponses`, folding the Phase-106 host handlers (sampling/elicitation/roots) into the v2 flow (CLNT-01, CLNT-02) | ✓ VERIFIED | `Client::with_protocol_version`, per-request `_meta` emission, `send_with_mrtr`'s bounded gather-then-resend loop, `fold_input_requests` routed through the SAME `ClientHostRegistry` (`self.host_registry`) that carries the Phase-106 sampling/elicitation/roots handlers. `client_server_mrtr_*` integration tests (in `tests/v2_mrtr.rs`) exercise a REAL Client against a REAL Server end-to-end and passed. |

**Score:** 4/5 truths verified

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| HTTP-01 | 113-04 | v2 stateless era gate, no session/handshake | SATISFIED* | `sessions_active` gate + `tests/v2_stateless_http.rs` (23/23 pass) |
| HTTP-02 | 113-02/03/06/09/11 | `input_required` + AEAD `requestState` | SATISFIED* | `src/server/request_state.rs`, `tests/v2_mrtr.rs` (28/28 pass) |
| HTTP-03 | 113-02/06/09/11 | Client retry resumes via echoed `requestState` | SATISFIED* | `client_server_mrtr_three_rounds`, `sep_2322_request_state_incomplete_then_complete` (both pass) |
| HTTP-04 | 113-10/13 | `subscriptions/listen` stream, retirement of legacy RPCs | **BLOCKED** | Retirement mechanism sound; **collision-safety and buffer-bound defects confirmed live** (CR-01/CR-02/CR-03 in `113-REVIEW.md`, independently reproduced) |
| HTTP-05 | 113-08 | SSE resumability off on v2, id-replay regression | SATISFIED* | `tests/v2_stateless_http.rs` id-invariant tests (pass) |
| CLNT-01 | 113-05/13 | Client speaks v2 | SATISFIED* | `src/client/mod.rs` `with_protocol_version` etc.; `client_server_mrtr_no_session_no_handshake` (pass) |
| CLNT-02 | 113-07/11 | Client fulfills MRTR via Phase-106 host handlers | SATISFIED* | `fold_input_requests` → `self.host_registry` (Phase-106 `ClientHostRegistry`); `client_server_mrtr_*` (pass) |

\* All seven requirements carry `[~]` (implemented, pending final schema) in `.planning/REQUIREMENTS.md`, per a written, human-granted developer exception recorded in `113-SPEC-RECHECK.md` (`## Recorded Exception`, granted by Guy Ernest, 2026-07-24/25). No `schema/2026-07-28` directory exists upstream as of the re-verification on 2026-07-26 (three v2 error-code constants `-32020/-32021/-32022` are pre-final values under exception). This is a correctly and honestly disclosed EXTERNAL blocking factor with a binding re-verification obligation already recorded — it is not treated as an independent gap by this verification, since the codebase's own status marking already reflects it accurately and there is nothing further for the codebase to do until 2026-07-28. **HTTP-04 is additionally BLOCKED for a second, independent reason** (the confirmed correctness defect above) that is unrelated to the schema-pending caveat and will not resolve when the final schema publishes.

No orphaned requirements: REQUIREMENTS.md's Phase 113 row set (HTTP-01..05, CLNT-01, CLNT-02) exactly matches the seven IDs declared across the 13 plans' frontmatter.

### Required Artifacts (representative sample; full list in plan frontmatter)

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/server/streamable_http_server.rs` (`sessions_active`) | Single era-gated predicate for all session sites | ✓ VERIFIED | Wired at 5 call sites; live HTTP tests pass |
| `src/server/request_state.rs` | AEAD `requestState` codec | ✓ VERIFIED | 1748 lines, real `ring` AEAD, property tests, fuzz target (`fuzz_request_state`, 20 000 runs / 0 crashes) |
| `src/types/mrtr.rs` | MRTR wire types + splice/param-digest | ✓ VERIFIED | Used by both client (`splice_mrtr_params`) and server ingress/egress |
| `src/server/subscriptions.rs` (`ListenRegistry`) | Collision-safe `subscriptions/listen` registry | ⚠️ **STUB-LIKE DEFECT** | Exists, compiles, passes its own (incomplete) test suite — but the documented collision-safety invariant does not hold for same-principal id reuse (see Gap) |
| `src/shared/sse_parser.rs` | Bounded SSE decoder | ⚠️ **PARTIAL** | Parses correctly (char-boundary fix verified); `SseConfig::max_buffer_size` bound is declared but dead code — unbounded growth on the new long-lived consumer |
| `src/client/subscriptions.rs` (`SubscriptionStream`) | Client-side listen stream | ✓ VERIFIED (with caveats) | Wired, tested (`tests/v2_subscriptions_client.rs`, 7/7 pass); does not validate agreed-filter-is-subset (WR-05, review) and reads an unbounded rejection body (WR-06, review) — both Warning-level |
| `examples/s47_v2_stateless_mrtr.rs`, `s48_v2_mrtr_client.rs`, `s49_v2_subscriptions_client.rs` | Runnable demonstrations | ✓ VERIFIED | All three build clean under `--features full` |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `Client::call_tool_mrtr` | `send_with_mrtr` → `mrtr_round_step` → `fold_input_requests` | direct call chain | ✓ WIRED | Confirmed by reading + `client_server_mrtr_three_rounds` passing |
| `fold_input_requests` | `self.host_registry` (Phase-106 `ClientHostRegistry`) | `answer_host_elicitation`/`answer_mrtr_sampling`/`answer_host_roots` | ✓ WIRED | Same registry field used by v1 host dispatch — folding confirmed, not just plumbing |
| HTTP POST handler | `sessions_active(state, era)` | direct predicate call at 5 sites | ✓ WIRED | grep + live test confirmation |
| `subscriptions/listen` route | `ListenRegistry::register` | `dispatch_request_or_retire` → `assemble_subscriptions_listen` | ✓ WIRED (but internally defective — see Gap) | Route reaches the registry; the registry's own concurrency contract is broken |
| `ServerBuilder::build()` | build-time load-balancer WARN | `advertises_subscriptions(&self.capabilities)` | ⚠️ PARTIAL | WARN fires on capability advertisement alone, not gated on `is_v2_opted_in` (WR-03, review) — false alarm on v1-only servers; Warning, not Blocker |
| `StreamableHttpServer` shutdown | `Server::close_subscription_streams` | (none found) | ✗ NOT WIRED | `grep -rn close_subscription_streams src/ tests/ examples/` returns only the definition and doc references — zero callers (WR-07, review, independently confirmed) |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `input_required` result (`resultType`, `inputRequests`, `requestState`) | handler `_meta` MRTR signal | `RequestHandlerExtra` accessor → AEAD mint in `request_state.rs` | Yes — real crypto, not a placeholder | ✓ FLOWING |
| `subscriptions/listen` ack + fan-out frames | `ServerNotification` | `Server::send_notification` → `ListenRegistry::fan_out` → per-entry `filter.covers()` | Yes, but delivery integrity is compromised under the CR-01/02 collision | ⚠️ FLOWING-BUT-UNSAFE |
| Client `MrtrOutcome` | server response body | real HTTP round trip in `tests/v2_mrtr.rs` client-server tests | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| HTTP-01/HTTP-05 live-HTTP acceptance suite | `cargo test --test v2_stateless_http --features full` | 23 passed; 0 failed | ✓ PASS |
| MRTR end-to-end (HTTP-02/03, CLNT-02, sep-2322 manifest) | `cargo test --test v2_mrtr --features full` | 28 passed; 0 failed (incl. `manifest_maps_every_pinned_scenario`) | ✓ PASS |
| `subscriptions/listen` server-side | `cargo test --test v2_subscriptions --features full` | 9 passed; 0 failed | ✓ PASS (does not cover same-principal collision) |
| `subscriptions/listen` client-side | `cargo test --test v2_subscriptions_client --features full` | 7 passed; 0 failed | ✓ PASS (does not cover same-principal collision) |
| **CR-01 live reproduction** | temporary in-tree `#[tokio::test]` probe added to `src/server/subscriptions.rs`'s existing test module, calling `ListenRegistry::register` twice with the SAME principal + id, then reverted (file diff clean after revert — confirmed via `git status --short`) | `live_streams()` after 2nd register = **1** (not 2); first stream's `try_recv()` = **`Err(Disconnected)`** even before any `fan_out` delivery attempt reached it | ✗ **CONFIRMS CR-01** |
| Example builds | `cargo build --example {s47_v2_stateless_mrtr,s48_v2_mrtr_client,s49_v2_subscriptions_client} --features full` | all 3: `Finished` | ✓ PASS |
| Debt-marker scan (TBD/FIXME/XXX) across all phase-113-touched files | `grep -n -E "TBD\|FIXME\|XXX" $(git diff --name-only 0c598639..HEAD -- src/ tests/ examples/ fuzz/ Cargo.toml)` | no matches | ✓ PASS (no blocker-class debt markers) |

### Probe Execution

Step 7c: SKIPPED (no `scripts/*/tests/probe-*.sh` conventional probes exist in this repo, and no PLAN/SUMMARY in this phase declares one).

### Anti-Patterns Found

(Independently re-verified against `113-REVIEW.md`; CR-01/CR-02/CR-03 already folded into the Gap above. Remaining findings from the code review are corroborated but classified as Warning/Info, not Blocker, per the adversarial framework's decision tree — they do not independently flip a Success-Criterion truth.)

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `src/server/subscriptions.rs` | 474-481, 410-418, 595-597 | Blind `HashMap::insert`/`remove` with no per-entry ownership check (CR-01/CR-02) | 🛑 Blocker | Same-principal id reuse silently destroys both listen streams; folded into the Gap above (HTTP-04) |
| `src/shared/sse_parser.rs` | 169-205, 378 | Unbounded buffer growth; documented cap (`SseConfig::max_buffer_size`) is dead code (CR-03) | 🛑 Blocker | Unbounded client memory growth on the new long-lived `subscriptions/listen` consumer; folded into the Gap above (HTTP-04) |
| `src/server/subscriptions.rs` | 562-576 | Overflow-disconnect releases the map entry but not its concurrency permits (WR-01) | ⚠️ Warning | `live_streams()` and the semaphore accounting drift apart under backpressure |
| `src/server/subscriptions.rs` | 519-546 | "Reserved overflow slot" check-then-send race under concurrent fan-out (WR-02) | ⚠️ Warning | The reservation is not atomic; inline comment claiming it "cannot happen" is incorrect |
| `src/server/mod.rs` | 4750-4759 | Build-time WARN keyed on capability advertisement alone, not era opt-in (WR-03) | ⚠️ Warning | False alarm on existing v1-only servers that can never serve the route |
| `src/server/subscriptions.rs` | 268-278 | Hardcoded concurrency/buffer limits, no config surface (WR-04) | ⚠️ Warning | Operators cannot raise the 64-stream cap or lower the keep-alive interval |
| `src/client/subscriptions.rs` | 328-405 | Client never validates the agreed filter is a subset of the request, nor filters incoming frames by kind (WR-05) | ⚠️ Warning | A buggy/hostile server can push notifications outside the agreed filter and the client treats them as legitimate |
| `src/client/subscriptions.rs` | 131-150 | `rejection_error` collects an unbounded response body (WR-06) | ⚠️ Warning | Untrusted server/intermediary can exhaust client memory via an oversized error body |
| `src/server/mod.rs` / `src/server/subscriptions.rs` | 859-861 / 584-591 | `close_subscription_streams` — the only graceful-shutdown trigger — has zero callers anywhere in the repo (WR-07, independently confirmed via grep) | ⚠️ Warning | The documented "server shutdown" closure trigger is dead code; only client-disconnect and overflow-disconnect actually fire |
| `fuzz/fuzz_targets/subscription_listen_frames.rs` | 30-40 | Cross-delivery invariant can false-positive on JSON `\u` escaping (WR-08) | ℹ️ Info | Fuzz target could report a spurious crash |
| `src/server/streamable_http_server.rs` | 2702-2720 | Silent `unwrap_or_else` fallbacks on terminal-frame serialization (WR-09) | ℹ️ Info | An internal serialization failure degrades into a spec-violating frame rather than a logged error |
| `Cargo.toml` | 594-604 | Self-admitted deferred-rename note ("Renaming these two...is deferred") in a shipped manifest (IN-07) | ℹ️ Info | Does not match the strict `TBD/FIXME/XXX` gate (no literal marker), but is SATD in spirit per CLAUDE.md's zero-SATD policy |
| `113-13-SUMMARY.md` | 124 | `cargo check --bin subscription_listen_frames` recorded as the fuzz evidence, not an actual `cargo fuzz run` | ℹ️ Info (verifier-added) | The CLAUDE.md "ALWAYS fuzz testing" requirement for this specific target was build-checked, not executed — unlike `fuzz_request_state`'s real 20 000-run campaign |

### Human Verification Required

None. All must-haves in this phase are either mechanically verifiable (and were verified, including one live reproduction) or already correctly gated by the phase's own recorded developer exception (spec-pending status), which has its own binding re-verification procedure already on file.

### Gaps Summary

Four of the five ROADMAP Success Criteria are genuinely and substantively achieved — HTTP-01 (stateless era gate), HTTP-02/HTTP-03 (MRTR requestState + client resume loop), HTTP-05 (SSE-resumability-off + id-replay regression), and CLNT-01/CLNT-02 (v2 client + Phase-106 host-handler folding) are all backed by real, wired, passing code and live-HTTP integration tests — not stubs, and not just SUMMARY claims. The `[~]` (implemented-pending-final-schema) status these requirements carry in REQUIREMENTS.md is an honest, deliberately human-authorized posture (a recorded exception with a binding re-verification date of 2026-07-28), not an implementation gap, and this verification does not second-guess that authorized decision.

Success Criterion 3 (HTTP-04, `subscriptions/listen`) is where the SUMMARY narrative overstates what shipped. The era-gated retirement of `resources/subscribe`/`unsubscribe` is sound. But the stream's core collision-safety claim — the very reason `ListenKey` was changed from an id alone to `(principal, RequestId)` — is only half true: it prevents cross-principal collisions, and both tests that claim to prove this ("two_callers_same_request_id_do_not_cross", "two_principals_sharing_request_id_one_do_not_cross") use different principals, so the same-principal case ships completely untested and, on independent live reproduction, is confirmed broken: a second registration under one principal silently destroys the first stream with no terminal frame, and a guard drop can reclaim a live successor's entry. A second, independent defect in the same mechanism — an unbounded client-side SSE buffer whose documented cap is dead code — undermines the "long-lived stream" viability the criterion depends on. Both were found by the 2026-07-26 code review and independently re-confirmed here by direct source reading and a live, reverted reproduction test; neither is deferred to a later milestone phase (Phases 114-119 do not touch this mechanism).

Recommended next step: a Phase-113 gap-closure plan targeting `ListenRegistry::register`/`ListenGuard::drop` (reject-on-duplicate + generation-scoped removal) and `SseParser::feed` (enforce `SseConfig::max_buffer_size`), plus the regression test that exercises the previously-untested same-principal path.

---

_Verified: 2026-07-26T04:38:53Z_
_Verifier: Claude (gsd-verifier)_
