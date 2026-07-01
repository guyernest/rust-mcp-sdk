---
status: complete
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
source: [103-VERIFICATION.md]
started: 2026-06-30T00:00:00Z
updated: 2026-07-01T01:26:39Z
---

## Current Test

[testing complete — both tests pass after 3 SDK fixes applied during UAT]

## Tests

### 1. Full browser PKCE redirect round-trip
expected: Run `examples/web-channel-client/client/build.sh`, serve the client and start the `server/` demo IdP, click Login → complete the bundled IdP consent → the page detects `?code=&state=` on return, validates state, exchanges the code for a bearer at `/oauth2/token`, stores the bearer in sessionStorage, and an authenticated MCP request (e.g. tools/list) succeeds.
result: pass
notes: "Full-page redirect → code+state returned → state validated → code exchanged for bearer → bearer stored in sessionStorage → Client::initialize over Fetch succeeded ('Connected. Ready to run the task.'). Passed only after fixes F1 (Instant panic) and F2 (wire codec) below. PKCE/OAuth logic itself was correct from the start."

### 2. Tasks lifecycle visible in the UI
expected: In the running demo, invoke the `slow_summarize` long task → the visible ~500ms `tasks/get` poll loop updates the status line `Working → Completed` over a few seconds → `tasks/result` renders the content. Separately, invoke the task again and click Cancel mid-run → the task transitions to `Cancelled` via `tasks/cancel`.
result: pass
notes: "Run 1: task created → 6× tasks/get 'working' over ~3s → 'completed' → tasks/result rendered the 3-point summary content. Run 2: created → 'working' → Cancel → 'cancelled' (confirmed by a follow-up tasks/get 'cancelled'), no result fetched. Passed only after fix F3 (SSE-tolerant transport) below."

## Fixes applied during UAT (root causes the automated verification missed)

All three were invisible to `wasm-pack build` (compiles fine) and to the native HTTP
tests (which use SSE-aware transports on a host target where `Instant` works) — they
only surface when a real browser drives `WasmHttpTransport` against the server.

### F1 — `std::time::Instant::now()` panics on wasm32 (BLOCKER, SDK)
`MiddlewareContext::default()` stamped `start_time: std::time::Instant`, and
`Client::send_request` builds one per request → every MCP request aborted in the
browser with "time not implemented on this platform".
Fix: added the `web-time` crate (drop-in `Instant`, `std` on native / `performance.now()`
on wasm) and switched `src/shared/middleware.rs` to `web_time::Instant`.

### F2 — WASM transport put the wrong shape on the wire (BLOCKER, SDK)
`WasmHttpTransport` serialized the untagged `TransportMessage` enum directly, so a
Request went out as `{"id":…,"request":…}` instead of a JSON-RPC frame — the server
rejected it with `-32700 "Unknown message type"`.
Fix: extracted the pure JSON-RPC codec (`serialize_message`/`parse_message`) into the
ungated `src/shared/transport.rs` as the single source of truth; `StdioTransport` now
delegates to it, and `WasmHttpTransport` uses it for both send and receive.

### F3 — WASM transport couldn't read SSE tool responses (BLOCKER, SDK)
The streamable-HTTP server answers `initialize` as `application/json` but streams
`tools/call` / `tasks/*` results as a single `text/event-stream` frame (regardless of
`Accept`). `WasmHttpTransport` fed the raw SSE text to `serde_json` → "expected value
at line 1 column 1".
Fix: `WasmHttpTransport::extract_jsonrpc_payload` now accepts BOTH a raw JSON body and
a single SSE `data:` frame before parsing.

### Secondary (logged, NOT fixed this pass — recommend follow-up)
- **A (MAJOR, example):** `WasmClient`'s all-`&mut self` async methods cause a wasm-bindgen
  "recursive use of an object" aliasing panic when a load-time auto-reconnect overlaps a
  user click. Recommend a re-entrancy/busy guard + disabling controls during in-flight ops.
- **B (MINOR, docs):** README quickstart hits the `/callback` 404 + extensionless-file
  download trap with plain `python3 -m http.server`. Recommend shipping a callback-aware
  static server snippet, or registering the redirect_uri as `/index.html`.

## Summary

total: 2
passed: 2
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none — all blocker gaps closed by F1/F2/F3; secondary items A/B tracked above]
