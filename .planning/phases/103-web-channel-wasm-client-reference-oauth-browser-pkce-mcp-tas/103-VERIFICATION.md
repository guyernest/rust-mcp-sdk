---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
verified: 2026-06-30T23:35:33Z
status: human_needed
score: 8/8 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Full browser PKCE redirect round-trip"
    expected: "Run examples/web-channel-client/client/build.sh, serve index.html, click Login, complete bundled IdP consent, confirm bearer stored in sessionStorage, and authenticated tools/list succeeds"
    why_human: "Requires a real browser + redirect navigation; the full-page redirect (window.location) and cookie/storage effects cannot be tested without a headless browser session. All code paths verified in isolation; only the end-to-end redirect orchestration requires a browser."
  - test: "Tasks lifecycle visible in browser UI (Working-to-Completed poll + Cancel button)"
    expected: "Invoke slow_summarize, observe ~500ms poll loop updating the task-status line Working, then Completed with result JSON. Click Cancel mid-run and confirm tasks/cancel returns Cancelled status."
    why_human: "Visual/interactive; the explicit poll loop timing and Cancel button interaction require a running browser session. The poll loop (setTimeout) and cancel wiring are code-verified; only the rendered output requires human observation."
---

# Phase 103: Web-Channel WASM Client Reference Verification Report

**Phase Goal:** Ship a new dedicated, adaptable browser MCP client — `examples/web-channel-client/` — demonstrating (1) OAuth via browser PKCE and (2) the MCP Tasks lifecycle over plain HTTP Fetch (no SSE), built on the existing transport foundation, plus two small reusable additions to the existing `pmcp` crate (a wasm-safe PKCE helper + a fixed `WasmHttpTransport`), shipped in a new pmcp release.
**Verified:** 2026-06-30T23:35:33Z
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A wasm-safe PKCE helper (`generate_code_verifier`, `code_challenge_s256`, `generate_state`) exists in `pmcp`, ungated, and compiles on both host and wasm32 | VERIFIED | `src/shared/pkce.rs` exists; `pub mod pkce;` in `src/shared/mod.rs` at line 22 with no `#[cfg]` gate; `pub use shared::pkce::...` in `src/lib.rs:105` ungated; `cargo build -p pmcp --target wasm32-unknown-unknown --features wasm` exits 0; host lib tests pass (6 tests) |
| 2 | `getrandom` is a cross-target `[dependencies]` entry (not wasm-only), proving the ungated PKCE module links on host (HIGH-1) | VERIFIED | `Cargo.toml:89` has `getrandom = "0.4"` in the cross-target `[dependencies]` table, confirmed ABOVE the first `[target.` section at line 119; wasm32-target entry at line 132 retains `features = ["wasm_js"]` |
| 3 | `WasmHttpTransport::receive()` no longer hard-errors; it returns the response buffered by `send()` via a `PendingSlot` one-slot buffer | VERIFIED | `src/shared/pending_slot.rs` exists; old error string "HTTP transport requires send() before receive()" is absent from `wasm_http.rs` (grep count = 0); `wasm_http.rs` uses `self.pending.put(response)` in `send()` and `self.pending.take()` in `receive()` |
| 4 | `PendingSlot::put` returns `Result` and errors on a double-send (MEDIUM-4); send-store-receive contract proven by host `cargo test` | VERIFIED | `src/shared/pending_slot.rs` contains four tests; `cargo test -p pmcp --lib pending_slot` exits 0 with 4 passing; `put` signature is `-> Result<()>` at line 65; occupied-slot guard verified by `pending_slot_put_on_occupied_is_error` test |
| 5 | The example is split into a `client/` wasm cdylib crate (no native HTTP deps) and a `server/` native crate (HIGH-2) | VERIFIED | `examples/web-channel-client/client/Cargo.toml` declares `crate-type = ["cdylib"]` and `pmcp` with `default-features = false, features = ["wasm"]`; `cargo tree --target wasm32-unknown-unknown` for the client crate shows zero tokio/hyper/axum entries; server Cargo.toml is a separate package with `features = ["full"]` |
| 6 | The browser WasmClient uses `Client<WasmHttpTransport>` (high-level) and drives `call_tool_with_task -> tasks_get -> tasks_result + tasks_cancel` over Fetch | VERIFIED | `examples/web-channel-client/client/src/lib.rs:137` declares `client: Option<Client<WasmHttpTransport>>`; `invoke_task` calls `client.call_tool_with_task`; `poll_task` calls `client.tasks_get`; `task_result` calls `client.tasks_result`; `cancel_task` calls `client.tasks_cancel` |
| 7 | The demo server drives a real Working-to-Completed delayed task, bearer validation, and tasks/cancel; wire shapes mirror `tests/tool_as_task_lifecycle_http.rs` | VERIFIED | `tests/web_channel_long_task_http.rs` — 3 tests pass under `--features "streamable-http,http-client"`: `long_task_completes_over_http` (Working + -32002 before delay, Completed + non-empty tasks/result after), `task_cancel_over_http`, `demo_server_requires_bearer`; frozen contract test `tool_as_task_lifecycle_http.rs` still green (2 passed) |
| 8 | pmcp version bumped to 2.11.0 with a CHANGELOG entry covering the PKCE helper and WasmHttpTransport fix | VERIFIED | `Cargo.toml:3` has `version = "2.11.0"`; `CHANGELOG.md:8` has `## [2.11.0] - 2026-06-30` with Added/Fixed/Changed sections |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/shared/pkce.rs` | PKCE helper: `generate_code_verifier`, `code_challenge_s256`, `generate_state` | VERIFIED | Exports all three functions; uses `getrandom::fill`; no unwrap/expect in production code; 200 lines with full rustdoc |
| `src/shared/pending_slot.rs` | Target-agnostic one-slot buffer, `pub(crate)` | VERIFIED | 192 lines; `PendingSlot` with `put` -> `Result<()>` and `take`; four host-target tests; `pub(crate) mod pending_slot` in `src/shared/mod.rs:17` |
| `src/shared/wasm_http.rs` | `WasmHttpTransport::receive()` no longer hard-errors; uses `PendingSlot` | VERIFIED | `pending: PendingSlot` field at line 53; `send()` calls `self.pending.put(response)` at line 208; `receive()` calls `self.pending.take()` at line 214 |
| `tests/pkce_helper.rs` | RFC 7636 vector + property (proptest) + roundtrip tests | VERIFIED | Contains `pkce_rfc7636_vector`, `pkce_verifier_charset_len`, `pkce_challenge_deterministic`, `pkce_base64url_roundtrip`; uses `proptest!`; 5 tests pass |
| `fuzz/fuzz_targets/pkce_helper.rs` | cargo-fuzz target: verifier-to-challenge roundtrip never panics | VERIFIED | `#![no_main]` + `fuzz_target!` exercising `code_challenge_s256` + base64url decode; registered in `fuzz/Cargo.toml:119-120`; builds via `cargo build --bin pkce_helper` |
| `examples/web-channel-client/client/src/lib.rs` | WASM client: PKCE flow + high-level Client task helpers | VERIFIED | 16.3K; `#[cfg(target_arch = "wasm32")]` gated; uses `Client<WasmHttpTransport>`; calls `generate_code_verifier/code_challenge_s256/generate_state`; sessionStorage helpers at line 96; bearer in `extra_headers` at line 264 |
| `examples/web-channel-client/client/Cargo.toml` | wasm cdylib crate, `pmcp features=["wasm"]`, no native HTTP | VERIFIED | `crate-type = ["cdylib"]`; `pmcp` with `default-features = false, features = ["wasm"]`; no hyper/axum/tokio direct deps; verified clean wasm32 build tree |
| `examples/web-channel-client/client/main.js` | `?code=&state=` redirect detection + explicit 500ms setTimeout poll loop | VERIFIED | Line 127: `setTimeout(pollOnce, POLL_INTERVAL_MS)` with `POLL_INTERVAL_MS = 500`; redirect detection at line 77; full-page redirect at line 69 |
| `examples/web-channel-client/client/index.html` | Login UI + task status line + Cancel button | VERIFIED | `<button id="login-btn">Login (PKCE redirect)</button>` at line 24; `<div id="task-status">` at line 36; `<button id="cancel-btn" disabled>Cancel task</button>` at line 33 |
| `examples/web-channel-client/server/src/main.rs` | Native demo server: long task + merged OAuth2 routes + bearer | VERIFIED | 17.6K; `InMemoryOAuthProvider`; `axum::Router::merge` for IdP route composition; `InMemoryTaskStore`; background updater with race-narrowed owner discovery; documents MEDIUM-6 single-user limitation |
| `examples/web-channel-client/server/Cargo.toml` | Separate native crate, `pmcp features=["full"]` | VERIFIED | Own package `web-channel-demo-server`; `pmcp` with `features = ["full"]`; separate from client cdylib |
| `examples/web-channel-client/README.md` | Documents channel adaptation + single-user concurrency limitation | VERIFIED | "How the pieces fit (adapting this as a web-app channel)" at line 84; "Known limitation — single-user / no concurrency for the delayed task" at line 116 |
| `tests/web_channel_long_task_http.rs` | Integration test: Working->Completed, cancel, bearer-required | VERIFIED | 19.1K; 3 tests pass: `long_task_completes_over_http`, `task_cancel_over_http`, `demo_server_requires_bearer` |
| `tests/web_channel_oauth_route_merge_spike.rs` | Durable proof: merged MCP+OAuth router on one origin | VERIFIED | 9.1K; `merged_mcp_and_oauth2_routes_respond_on_one_origin` test passes; asserts both `/oauth2/authorize` and `/oauth2/token` respond on same origin |
| `Cargo.toml` | `version = "2.11.0"` | VERIFIED | Line 3 |
| `CHANGELOG.md` | `## [2.11.0]` entry | VERIFIED | Line 8 with Added/Fixed/Changed sections covering PKCE helper, WasmHttpTransport fix, and getrandom relocation |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/lib.rs` | `src/shared/pkce.rs` | `pub use shared::pkce::{...}` | VERIFIED | Line 105, ungated (no `#[cfg]`) |
| `src/shared/pkce.rs` | `getrandom::fill` | CSPRNG byte fill | VERIFIED | `getrandom::fill(&mut buf)` in `random_bytes()` |
| `Cargo.toml [dependencies]` | host build getrandom linkage | cross-target entry | VERIFIED | Line 89, confirmed above first `[target.` section |
| `WasmHttpTransport::send` | `WasmHttpTransport::receive` | `self.pending` one-slot `PendingSlot` | VERIFIED | `wasm_http.rs:208` puts; `wasm_http.rs:214` takes |
| `examples/.../client/src/lib.rs` | pmcp pkce helper | `generate_code_verifier / code_challenge_s256 / generate_state` | VERIFIED | `lib.rs:32` import; used at lines 179-181 |
| `examples/.../client/src/lib.rs` | `Client<WasmHttpTransport>` | bearer in `extra_headers` | VERIFIED | `WasmHttpConfig { extra_headers: vec![("Authorization", "Bearer {token}")] }` at line 264 |
| `examples/.../client/main.js` | wasm `poll_task` | 500ms `setTimeout` loop | VERIFIED | `setTimeout(pollOnce, POLL_INTERVAL_MS)` at lines 127 and 141 |
| `Cargo.toml version` | `CHANGELOG.md 2.11.0 entry` | release version bump | VERIFIED | `version = "2.11.0"` and `## [2.11.0] - 2026-06-30` both present |
| `fuzz/Cargo.toml [[bin]]` | `fuzz/fuzz_targets/pkce_helper.rs` | cargo-fuzz registration | VERIFIED | `name = "pkce_helper"` at line 119; binary builds |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| PKCE helper tests pass (host target) | `cargo test -p pmcp --lib pkce` | 6 passed | PASS |
| pkce_helper integration tests pass | `cargo test -p pmcp --test pkce_helper` | 5 passed (proptest included) | PASS |
| PendingSlot contract tests pass | `cargo test -p pmcp --lib pending_slot` | 4 passed | PASS |
| Web channel long task integration | `cargo test --test web_channel_long_task_http --features "streamable-http,http-client"` | 3 passed | PASS |
| OAuth route merge spike test | `cargo test --test web_channel_oauth_route_merge_spike --features "streamable-http,http-client"` | 1 passed | PASS |
| Frozen contract unregressed | `cargo test --test tool_as_task_lifecycle_http --features "streamable-http,http-client"` | 2 passed | PASS |
| pmcp host build | `cargo build -p pmcp --features full` | exit 0 | PASS |
| pmcp wasm build | `cargo build -p pmcp --target wasm32-unknown-unknown --no-default-features --features wasm` | exit 0 | PASS |
| client wasm check | `cargo check --manifest-path .../client/Cargo.toml --target wasm32-unknown-unknown` | exit 0 | PASS |
| server native check | `cargo check --manifest-path .../server/Cargo.toml` | exit 0 | PASS |
| fuzz target builds | `cd fuzz && cargo build --bin pkce_helper` | exit 0 | PASS |
| getrandom cross-target placement | grep: line 89 before first `[target.` (line 119) | confirmed | PASS |
| Old hard-error removed from wasm_http | `grep -c "HTTP transport requires send() before receive()" src/shared/wasm_http.rs` | 0 | PASS |
| No SSE in client/server | grep for EventSource/text/event-stream | 0 matches in source | PASS |
| examples/wasm-client untouched | `git log 855f3e30..HEAD -- examples/wasm-client/` | no commits | PASS |
| task_dispatch.rs/tool_as_task_lifecycle_http.rs untouched by phase 103 | git log check | no phase 103 touches | PASS |

---

### Scope Fence Verification

| Fence | Status | Evidence |
|-------|--------|----------|
| `examples/wasm-client` UNCHANGED | VERIFIED | No commits to `examples/wasm-client/` from the phase 103 commit range (855f3e30..HEAD) |
| `tasks/*` wire contract (task_dispatch.rs, tests/tool_as_task_lifecycle_http.rs) UNCHANGED | VERIFIED | `tool_as_task_lifecycle_http.rs` last modified by commit 270fafe8 (Phase 102); `task_dispatch.rs` last modified 9f9d19b5 (Phase 102). Frozen contract test still passes green (2/2) |
| No SSE | VERIFIED | Zero matches for EventSource / text/event-stream in client lib.rs, main.js, and server main.rs; README explicitly documents "no SSE" |
| Two reusable pieces extend existing `pmcp` crate only (NOT a new published crates/ library) | VERIFIED | `src/shared/pkce.rs` and repaired `src/shared/wasm_http.rs` are in the root `pmcp` crate; no new entry in `crates/` |
| pmcp version bumped to 2.11.0 with CHANGELOG | VERIFIED | `Cargo.toml:3 = "2.11.0"`, `CHANGELOG.md:8 = ## [2.11.0] - 2026-06-30` |

---

### Probe Execution

No `scripts/*/tests/probe-*.sh` probes declared for this phase. Integration tests run directly above.

---

### Anti-Patterns Found

| File | Pattern | Severity | Assessment |
|------|---------|----------|------------|
| `src/shared/pkce.rs` test block | `generate_code_verifier().expect(...)` | Info | SAFE — all `.expect()` calls are inside `#[cfg(test)]` at line 148+; production code uses `?` propagation only |
| No TBD/FIXME/XXX markers | — | — | Clean: zero debt markers found in any phase 103 file |
| No placeholder/TODO/HACK | — | — | Clean: doc comment "not available on wasm32" at pkce.rs:8 is descriptive prose, not a debt marker |

---

### Human Verification Required

All automated checks pass. Two items require a real browser to confirm:

#### 1. Full Browser PKCE Redirect Round-Trip

**Test:** Run `examples/web-channel-client/client/build.sh` (requires wasm-pack), then `cargo run --manifest-path examples/web-channel-client/server/Cargo.toml` in a separate terminal, serve the client directory on `http://127.0.0.1:8080`, open `index.html`, click "Login (PKCE redirect)", complete the bundled IdP consent screen, observe the redirect return URL with `?code=&state=` query params, and confirm the bearer is stored and the tools/list call succeeds.

**Expected:** Token exchange completes, `sessionStorage` holds the bearer, "Logged in and connected." status appears, the Invoke Task button becomes enabled.

**Why human:** The full-page redirect (`window.location = authorizeUrl`) navigates the browser to the IdP and back. The code-path for reading `?code=&state=` from the returned URL (`URLSearchParams`, state CSRF check, Fetch POST to `/oauth2/token`) is implemented and code-verified, but the end-to-end round-trip with a real browser redirect cannot be exercised by grep or compile checks.

#### 2. Tasks Lifecycle Visible in Browser UI

**Test:** After login, click "Invoke Task", observe the task-status line cycle through `working` states at ~500ms intervals, then display `completed` with the result JSON. In a separate run, click "Invoke Task" then click "Cancel task" before completion; confirm the status shows `cancelled`.

**Expected:** Poll loop visibly updates the `Task status:` div every ~500ms. Cancel button transitions task to Cancelled. Result JSON appears in the `#result` div on completion.

**Why human:** The setTimeout-based poll loop, the Cancel button interaction, and the visual task-status updates require a running browser session. The `pollOnce()` function, `onCancel()` handler, and all wasm bindings are code-verified; only the rendered DOM interaction requires human observation.

---

### Gaps Summary

No automated gaps. All 8 must-have truths are VERIFIED by the codebase. The 2 human_verification items are deliberate browser-only behaviors documented in the VALIDATION.md as "Manual-Only Verifications" — they require a running browser and were never expected to be automatable in this phase.

---

_Verified: 2026-06-30T23:35:33Z_
_Verifier: Claude (gsd-verifier)_
