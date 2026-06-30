# Phase 103: Web-channel WASM client reference (OAuth browser-PKCE + MCP Tasks) - Context

**Gathered:** 2026-06-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship a new dedicated browser MCP client example — `examples/web-channel-client/` —
that the pmcp.run dev team can lift to build a web-application "channel." It
demonstrates two advanced MCP features the existing `examples/wasm-client` lacks,
both drivable from the browser today over plain HTTP/Fetch:

1. **OAuth via browser PKCE** — real PKCE redirect flow, tokens in the browser,
   bearer threaded into Fetch request headers.
2. **MCP Tasks lifecycle over plain HTTP** — `call(task) → poll tasks/get →
   tasks/result` (plus `tasks/cancel`) against a real `StreamableHttpServer`
   using request/response Fetch (NO SSE).

**Shape (LOCKED by ROADMAP):** NEW dedicated example, built on the existing
transport foundation — reuse `examples/wasm-client/src/lib.rs`,
`src/shared/wasm_http.rs`, `src/shared/wasm_websocket.rs`; do NOT rewrite the
transport layer; do NOT modify `examples/wasm-client` (it stays the minimal demo).

**Scope refinement from this discussion:** The ROADMAP fence says "do NOT promote
to a published `crates/` library this phase (example only)." That fence bars a
**new crate**. This phase intentionally adds two small, reusable pieces to the
**existing `pmcp` crate** (a wasm-safe PKCE helper + a `WasmHttpTransport`) and
ships them in a new `pmcp` release — this is compatible with the fence (extends
`pmcp`, not a new crate) and is the deliberate strategic win: "call MCP from a
web application" becomes a turnkey SDK capability for the pmcp.run team, not code
buried in an example.

</domain>

<decisions>
## Implementation Decisions

### OAuth port strategy (browser PKCE)
- **D-01:** Browser-specific orchestration lives **in the example**
  (`examples/web-channel-client/`): full-page redirect, reading `?code=&state=`
  on return, Fetch-based token exchange, threading the bearer token into the
  transport's request headers. The existing `src/client/auth.rs` / `oauth.rs` are
  `reqwest` + `tokio::net::TcpListener` (loopback-redirect) bound and
  `#[cfg(not(target_arch = "wasm32"))]`; the browser model is structurally
  different, so do NOT cfg-port those modules wholesale.
- **D-02:** Extract the **pure PKCE crypto primitives** (code_verifier generation,
  S256 code_challenge, state/nonce) into a **wasm-safe helper in the `pmcp` crate**
  shared by native (loopback) and browser flows. Ship in a new `pmcp` release.
  **Complexity guardrail (user-stated):** keep it low-complexity — if making the
  existing `oauth.rs` primitives `cfg`-clean for wasm drags in `reqwest`/`tokio`
  transitively, ship a minimal standalone helper rather than force a messy port.
- **D-03:** SDK scope is **crypto helper only** — do NOT add a full
  `#[cfg(wasm32)]` browser-PKCE orchestrator (auth-URL builder / callback handler)
  to `pmcp` this phase. That orchestration stays demonstrated in the example.
  (The reusable Client-over-Fetch win comes via the transport in D-08, not an auth client.)

### Demo provider & server packaging
- **D-04:** Bundle a **self-contained demo `StreamableHttpServer`** in the example,
  using the SDK's `src/server/auth/mock.rs` + `oauth2` as the IdP. Runs fully
  offline — no external accounts, no network, no secrets — so the demo is
  reproducible and CI-testable (satisfies ALWAYS coverage + criterion 3). No real
  external provider (Google/GitHub/Auth0) required to run the demo.
- **D-05:** The bundled server exposes **one simulated long task** (a
  `TaskSupport::Required` tool that transitions `Working → Completed` over a few
  seconds via the `TaskStore`), so the browser actually polls `tasks/get` several
  times before `tasks/result` and `tasks/cancel` is demonstrable. (Not the
  instant/synchronous shape of `s46`, which would complete on the first poll.)
  The wire shapes must still mirror `s46` / `tests/tool_as_task_lifecycle_http.rs`
  (Phase 101 froze the `tasks/*` contract; do NOT change it).

### Token storage & redirect
- **D-06:** Use **sessionStorage** (via `web-sys`) for the `code_verifier` + OAuth
  `state` (must survive the redirect round-trip) and for the resulting bearer
  token. Origin-scoped, cleared on tab close — good demo security default,
  synchronous API. (IndexedDB noted as the production durability upgrade path but
  NOT implemented this phase.)
- **D-07:** Drive the authorization redirect via **full-page redirect**
  (`window.location = authorize_url`); the auth server redirects back to the
  registered redirect URI (the example's `index.html`); on load, JS detects
  `?code=&state=` and resumes the flow. (Not a popup/postMessage flow.)

### Tasks transport & polling
- **D-08:** Add a new **`WasmHttpTransport` implementing the `Transport` trait**
  to the `pmcp` SDK under `src/shared/` (symmetric with the existing
  `WasmWebSocketTransport`; likely built atop the existing `WasmHttpClient` Fetch
  code in `src/shared/wasm_http.rs`, which today is a raw request method, NOT a
  `Transport`). This lets the **high-level `Client`** and its typed task helpers
  (`call_tool_with_task`, `tasks_get` / `tasks_result` / `tasks_cancel`) run over
  browser Fetch — the core reusable "call MCP from a web app" win. Ships in the
  same new `pmcp` release as the PKCE helper.
- **D-09:** The browser polls `tasks/get` on a **fixed ~500ms interval**
  (`setTimeout` / `gloo-timers`) until terminal status, updating a visible status
  line (`Working → Completed`) and exposing a **Cancel button** (drives
  `tasks/cancel`) in `index.html`. (Not exponential backoff; not a hidden
  auto-poll helper — the explicit poll loop is part of what the demo teaches.)

### Claude's Discretion
- Example crate layout, `build.sh`, `index.html`/`main.js`/`style.css` structure:
  mirror `examples/wasm-client`'s build setup (build script + `index.html` demo)
  unless a better pattern emerges during planning.
- Error / expired-token / state-mismatch UX surfacing in the demo UI.
- Exact name of the simulated long-task tool and its argument shape.
- Specific wasm crates for crypto (e.g. `getrandom` js backend + `sha2` + base64url)
  vs `web-sys` SubtleCrypto for the helper internals — pick the lower-complexity,
  smaller-binary option during research/planning.
- ALWAYS-coverage test targets (unit + property + fuzz where applicable) — the
  working browser example IS the EXAMPLE deliverable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Foundation to build on (reuse, do not rewrite)
- `examples/wasm-client/src/lib.rs` — 538-line dual-transport `WasmClient`; HTTP path
  uses raw JSON-RPC via `WasmHttpClient` + `to_jsonrpc()`; WS path uses high-level
  `Client<WasmWebSocketTransport>`. The new example builds on this style/structure.
- `examples/wasm-client/build.sh`, `examples/wasm-client/index.html`,
  `examples/wasm-client/main.js`, `examples/wasm-client/style.css` — build + demo
  harness pattern to mirror.
- `src/shared/wasm_http.rs` — Fetch transport (`WasmHttpClient`, `WasmHttpConfig`
  with `extra_headers` for the bearer; `session_id` handling). Basis for the new
  `WasmHttpTransport: Transport` (D-08).
- `src/shared/wasm_websocket.rs` — `WasmWebSocketTransport` (the existing `Transport`
  impl to be symmetric with).

### OAuth / PKCE
- `src/client/auth.rs` — PKCE token-exchange + OIDC discovery; `reqwest`/`tokio`
  bound, `#[cfg(not(target_arch = "wasm32"))]`. Source of the pure PKCE primitives
  to extract (D-02).
- `src/client/oauth.rs` — Authorization Code + PKCE flow; `tokio::net::TcpListener`
  loopback redirect (the part that does NOT translate to browser); contains
  verifier/challenge generation (lines ~592–629).
- `src/client/mod.rs:35-40` — where `auth`/`oauth` are wasm-gated off today.
- `src/server/auth/mock.rs`, `src/server/auth/oauth2.rs` — server-side IdP building
  blocks for the bundled self-contained demo provider (D-04).

### MCP Tasks over HTTP (wire contract — frozen, do not change)
- `examples/s46_http_tool_as_task.rs` — RECOMMENDED HTTP tool-as-task pattern:
  high-level `Server` + `StreamableHttpServer` + `with_task_support` tool +
  `TaskStore`; the demo server (D-04/D-05) follows this shape.
- `tests/tool_as_task_lifecycle_http.rs` — live HTTP `tasks/*` lifecycle test; the
  browser wire shapes must mirror it.
- `src/client/mod.rs:508-618` — high-level `Client` task helpers
  (`call_tool_with_task`, `tasks_get`, `tasks_result`, `tasks_list`, `tasks_cancel`)
  that become browser-usable via `WasmHttpTransport` (D-08).
- `src/server/mod.rs:1165-1177` — shared `task_dispatch` unit that serves all four
  `tasks/*` variants over HTTP (delivered by Phase 102).

### Phase / project context
- `.planning/ROADMAP.md` — Phase 103 entry: LOCKED shape + scope fences + success criteria.
- `.planning/phases/102-http-task-dispatch/102-VERIFICATION.md` — confirms the
  server-side HTTP `tasks/*` prerequisite is delivered (7/7), no blocker.
- `CLAUDE.md` §"ALWAYS Requirements" — fuzz/property/unit/example coverage;
  §"Release & Publish Workflow" — the new `pmcp` version that ships D-02 + D-08.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `WasmHttpClient` / `WasmHttpConfig` (`src/shared/wasm_http.rs`): already does
  Fetch request/response, manages `mcp-session-id`, and supports `extra_headers`
  (the natural place to inject `Authorization: Bearer <token>`). Wrap into a
  `Transport` impl rather than rewriting.
- `WasmWebSocketTransport` (`src/shared/wasm_websocket.rs`): the existing
  `Transport` impl to model `WasmHttpTransport` after.
- High-level `Client` task helpers (`src/client/mod.rs`): work over any `Transport`,
  so they become browser-callable for free once `WasmHttpTransport` exists.
- Server-side `mock.rs` + `oauth2` (`src/server/auth/`): self-contained IdP for the
  bundled demo server.
- `s46_http_tool_as_task.rs` + `InMemoryTaskStore`: copyable server scaffold for the
  task tool (adapt the synchronous task to a simulated multi-second one per D-05).

### Established Patterns
- WASM boundary discipline: SDK auth/oauth are `#[cfg(not(target_arch = "wasm32"))]`;
  new wasm code must be `#[cfg(target_arch = "wasm32")]`-gated and must NOT regress
  non-wasm builds (criterion 4).
- HTTP-over-Fetch uses request/response only (no SSE) — consistent with the locked
  out-of-scope list (streaming/SSE, elicitation, sampling, progress).
- Example build harness: `build.sh` → `wasm-pack`/`wasm-bindgen` output consumed by
  `index.html` + `main.js` (mirror `examples/wasm-client`).

### Integration Points
- New `WasmHttpTransport` plugs into `Client::new(transport)` in the example.
- Bearer token flows: PKCE token exchange (example) → sessionStorage → transport
  `extra_headers` → server `oauth2`/`mock` auth validation.
- New `pmcp` release packages the PKCE helper (D-02) + `WasmHttpTransport` (D-08).

</code_context>

<specifics>
## Specific Ideas

- User's strategic framing (verbatim intent): MCP is gaining popularity; enabling
  "calling MCP from a web application" — what the pmcp.run dev team is trying to do —
  is a unique capability and a good win for the SDK and its developer experience.
  This is WHY the otherwise-example-only phase deliberately adds two small reusable
  pieces to `pmcp` (PKCE helper + `WasmHttpTransport`) and cuts a new release.
- Demo must be runnable with no external accounts/secrets (offline, CI-friendly).
- The poll loop and Cancel button should be explicit/visible — the demo is meant to
  teach the Tasks lifecycle, not hide it behind an auto-poll helper.

</specifics>

<deferred>
## Deferred Ideas

- **IndexedDB token persistence** across tabs/restarts — documented as the production
  durability upgrade; not implemented this phase (sessionStorage chosen for the demo).
- **Full `#[cfg(wasm32)]` browser-PKCE orchestrator in `pmcp`** (auth-URL builder +
  callback handler as a turnkey SDK API) — explicitly out of SDK scope this phase
  (D-03); a candidate for a future phase if the example proves the pattern.
- **Real external OAuth provider integration** (Google/GitHub/Auth0) as a first-class
  runnable mode — out of scope; the bundled self-contained provider is the demo.
- **Popup/postMessage redirect flow** — not chosen; full-page redirect only.
- **Promoting the web-channel client to a published `crates/` library** — LOCKED out
  of scope by ROADMAP for this phase (example only).
- **Exponential backoff polling** — not chosen for the demo (fixed interval).

</deferred>

---

*Phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas*
*Context gathered: 2026-06-30*
