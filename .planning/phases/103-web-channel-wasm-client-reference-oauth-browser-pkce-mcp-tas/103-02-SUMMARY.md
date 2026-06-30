---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
plan: 02
subsystem: transport
tags: [wasm, fetch, transport, http, browser, mcp-client, pending-slot]

# Dependency graph
requires:
  - phase: 102-http-task-dispatch
    provides: HTTP tasks/* dispatch the browser Client will drive over Fetch
  - phase: 103 (plan 01)
    provides: pub mod pkce in src/shared/mod.rs (this plan appends pending_slot alongside it)
provides:
  - WasmHttpTransport::{send,receive} now correlate correctly so the high-level Client works over browser Fetch (D-08 / WEBCH-02)
  - PendingSlot — a target-agnostic, host-testable one-slot pending-response buffer (pub(crate)) with an occupied-slot error guard
affects: [web-channel-client example, browser PKCE flow, tasks-over-fetch poll loop, pmcp release packaging D-08]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "One-shot Fetch transport: send() POSTs + buffers the parsed response, receive() pops it (adapts the bidirectional-stream Transport trait to request/response)"
    - "Extract one-shot correlation plumbing into a target-agnostic, NOT-cfg-gated module so the contract is host-testable under plain cargo test instead of wasm-only compile checks"

key-files:
  created:
    - src/shared/pending_slot.rs
  modified:
    - src/shared/wasm_http.rs
    - src/shared/mod.rs

key-decisions:
  - "PendingSlot kept pub(crate) (LOW-7) with a module-level #![allow(clippy::redundant_pub_crate)] because unreachable_pub and redundant_pub_crate are mutually exclusive for items in a pub(crate) module — pub(crate) is the unreachable_pub-clean choice"
  - "Transport-contract proof (MEDIUM-5) modeled in pending_slot.rs via a canned do_request output flowing through put->take, since wasm_http.rs is #![cfg(target_arch=wasm32)] and cannot host in-file host tests"

patterns-established:
  - "Pattern 1: one-shot HTTP transport correlation via a one-slot PendingSlot (send buffers, receive pops)"
  - "Pattern 2: put() returns Result and errors on an occupied slot — no silent overwrite of a buffered response (MEDIUM-4)"

requirements-completed: [WEBCH-02]

# Metrics
duration: 18min
completed: 2026-06-30
---

# Phase 103 Plan 02: Fix WasmHttpTransport send/receive correlation Summary

**Repaired the broken WasmHttpTransport so the high-level Client runs over browser Fetch — send() now POSTs and buffers the parsed response in a host-testable one-slot PendingSlot (put errors on a double-send), receive() returns it.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-06-30
- **Completed:** 2026-06-30
- **Tasks:** 2 (both TDD)
- **Files modified:** 3 (plan files) + 2 doc-unblock files

## Accomplishments
- Fixed the central D-08/WEBCH-02 defect: `WasmHttpTransport::send()` previously POSTed and DISCARDED the response and `receive()` hard-errored, so `Client::initialize()` (send-then-loop-receive correlation) failed immediately over Fetch. It now correlates correctly.
- Added `src/shared/pending_slot.rs`: a target-agnostic, NOT-cfg-gated `PendingSlot` one-slot buffer. `put()` returns `Result` and ERRORS on an already-occupied slot (MEDIUM-4, no silent overwrite); `take()` errors on an empty slot (receive-before-send).
- Wired `pub(crate) mod pending_slot;` into `src/shared/mod.rs` ungated, so the correlation contract is proven by a host-target `cargo test`, not a wasm-only compile check (LOW-7: `pub(crate)`, not `pub`).
- Proved four GREEN host tests: put-then-take, empty-take-errors, occupied-put-errors (MEDIUM-4), and `transport_send_stores_and_receive_returns` (the send→store→receive transport contract via a canned `do_request` seam, MEDIUM-5) — all without real Fetch/`window`.
- Preserved free bearer injection: `do_request`/`do_http_request` still inject `extra_headers`, `mcp-session-id`, and parse the response. The `WasmHttpClient` raw wrapper is untouched (backward compat).

## Task Commits

Tasks 1 and 2 share the single new file `pending_slot.rs` (impl + inline `#[cfg(test)]` proofs) and were committed atomically as one TDD feat:

1. **Task 1 + Task 2: PendingSlot + send/receive fix + host-target proofs** - `bc30a01a` (feat)

**Plan metadata:** _(this SUMMARY + STATE/ROADMAP)_ — see final docs commit.

## Files Created/Modified
- `src/shared/pending_slot.rs` - NEW. Target-agnostic one-slot `PendingSlot` (put returns Result, errors on occupied; take errors on empty) + 4 host `#[cfg(test)]` proofs.
- `src/shared/wasm_http.rs` - Replaced the struct's ad-hoc field with `pending: PendingSlot`; `send()` = `pending.put(do_request(&msg).await?)`, `receive()` = `pending.take()`, `close()` = Ok. Removed the old hard-error string. `WasmHttpClient` raw wrapper untouched.
- `src/shared/mod.rs` - Added ungated `pub(crate) mod pending_slot;` (alongside the existing `pub mod pkce;` from 103-01).
- `src/shared/pkce.rs` - (deviation) one-token doc backtick fix (`` `IdP` ``) to unblock a pre-existing rust-1.95 `doc_markdown` gate error from 103-01.
- `src/lib.rs` - (deviation) one-token doc backtick fix (`` `StdioTransport` ``) to unblock a pre-existing rust-1.95 `doc_markdown` gate error from 103-01.

## Decisions Made
- **PendingSlot stays `pub(crate)` + module-level `#![allow(clippy::redundant_pub_crate)]`.** With `-D warnings`, `redundant_pub_crate` wants bare `pub` while `unreachable_pub` wants `pub(crate)` for items in a `pub(crate)` module — they are mutually exclusive. `pub(crate)` satisfies LOW-7 and `unreachable_pub`; the redundancy lint is silenced with a documented `// Why`.
- **MEDIUM-5 contract proof lives in `pending_slot.rs`.** Because `wasm_http.rs` carries `#![cfg(target_arch = "wasm32")]`, an in-file host test is impossible; the `transport_send_stores_and_receive_returns` test models the EXACT send→store→receive sequence (canned `do_request` output → `put` → `take` returns it) with a doc cross-reference to `WasmHttpTransport::send`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Unblock two pre-existing rust-1.95 `doc_markdown` gate errors from plan 103-01**
- **Found during:** Task 1 (running `make quality-gate` before commit)
- **Issue:** The shared pre-commit/CI gate failed on two `doc_markdown` errors in files landed by 103-01 (`src/shared/pkce.rs:103` "IdP", `src/lib.rs:103` "StdioTransport") — these are not my files but the workspace-wide gate blocked my commit.
- **Fix:** Added backticks around the two doc tokens (`` `IdP` ``, `` `StdioTransport` ``). Minimal one-token doc edits; no behavior change.
- **Files modified:** src/shared/pkce.rs, src/lib.rs
- **Verification:** `make quality-gate` EXIT=0 after the fix.
- **Committed in:** bc30a01a (part of Task 1 commit)

**2. [Rule 3 - Blocking] `clippy::redundant_pub_crate` on PendingSlot**
- **Found during:** Task 1 (`make quality-gate`)
- **Issue:** `pub(crate)` items inside the `pub(crate)` module tripped `redundant_pub_crate` under `-D warnings`; switching to `pub` then tripped `unreachable_pub` (the two are mutually exclusive here).
- **Fix:** Kept `pub(crate)` (LOW-7 / `unreachable_pub`-clean) and added a `// Why`-annotated module-level `#![allow(clippy::redundant_pub_crate)]`.
- **Files modified:** src/shared/pending_slot.rs
- **Verification:** `make quality-gate` EXIT=0.
- **Committed in:** bc30a01a (part of Task 1 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking gate issues)
**Impact on plan:** Both were required to pass the shared quality gate before committing this plan's files. No scope creep — the doc fixes are one-token, the lint allow is documented. All acceptance criteria met.

## Issues Encountered
- `TransportMessage` does not derive `PartialEq`, so test "same message" assertions compare via `serde_json::to_value(...)` equality. Resolved cleanly with a small `same_message` test helper.
- `PendingSlot` needed `#[derive(Clone)]` because the enclosing `WasmHttpTransport` derives `Clone`; added (and `TransportMessage` is `Clone`).

## Optional / Future (additive, not the green proof)
- An end-to-end `wasm-pack test --headless --firefox` check of the live Fetch path can be run from the example/phase verification later. The GREEN proof of the buffer correlation AND the send→store→receive contract already lives here in host `cargo test -p pmcp --lib pending_slot` (4/4 passing).

## Known Stubs
None — the fix is fully wired; no placeholder/empty data paths introduced.

## Self-Check: PASSED
- `src/shared/pending_slot.rs` exists (FOUND)
- Commit `bc30a01a` exists (FOUND)
- `cargo test -p pmcp --lib pending_slot` → 4 passed
- `cargo build -p pmcp --features full` → exit 0 (host unregressed)
- `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` → exit 0
- `grep -c "HTTP transport requires send() before receive()" src/shared/wasm_http.rs` → 0
- `pub(crate) mod pending_slot;` present and ungated in src/shared/mod.rs
- `make quality-gate` → EXIT=0

## Next Phase Readiness
- WEBCH-02 transport fix complete: the high-level `Client` + all four typed task helpers can now run over browser Fetch.
- Ready for the example-level plans (PKCE orchestration WEBCH-03, bundled demo server WEBCH-04/05, poll loop WEBCH-06) that build `Client<WasmHttpTransport>`.

---
*Phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas*
*Completed: 2026-06-30*
