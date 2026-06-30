---
status: partial
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
source: [103-VERIFICATION.md]
started: 2026-06-30T00:00:00Z
updated: 2026-06-30T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Full browser PKCE redirect round-trip
expected: Run `examples/web-channel-client/client/build.sh`, serve the client and start the `server/` demo IdP, click Login → complete the bundled IdP consent → the page detects `?code=&state=` on return, validates state, exchanges the code for a bearer at `/oauth2/token`, stores the bearer in sessionStorage, and an authenticated MCP request (e.g. tools/list) succeeds.
result: [pending]

### 2. Tasks lifecycle visible in the UI
expected: In the running demo, invoke the `slow_summarize` long task → the visible ~500ms `tasks/get` poll loop updates the status line `Working → Completed` over a few seconds → `tasks/result` renders the content. Separately, invoke the task again and click Cancel mid-run → the task transitions to `Cancelled` via `tasks/cancel`.
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps
