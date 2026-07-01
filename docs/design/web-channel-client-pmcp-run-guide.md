# Web-Channel Client — integration guide for the pmcp.run team

**Audience:** the pmcp.run team building a **web-application channel** — a browser
front-end that lets end users talk to pmcp.run agents (MCP servers) directly from
a web page, authenticated with OAuth and driving long-running work as MCP Tasks.

**TL;DR:** `examples/web-channel-client` is a working, "strong reference" browser
MCP client compiled to WebAssembly. It demonstrates the two hard parts of a web
channel — **browser OAuth (Authorization Code + PKCE)** and the **MCP Tasks
lifecycle over plain Fetch** — using the high-level `pmcp::Client` with **zero
native HTTP dependencies** in the `.wasm` artifact. Lift the `client/` crate,
point it at your IdP and your agent's MCP endpoint, and you have a web channel.

> **Link note:** paths below are repo-relative and clickable in a local clone.
> The GitHub links target `paiml/rust-mcp-sdk@main`; this example was added in
> Phase 103 and lands on `main` when that branch merges. Until then the code is
> on branch `gsd/phase-102-http-task-dispatch` — ask us to push it if you want
> live links before the merge.

---

## 1. What the example gives you

| Capability | How | Where |
|---|---|---|
| Browser OAuth **Authorization Code + PKCE** (full-page redirect) | `begin_login` builds the authorize URL + stashes verifier/state in `sessionStorage`; `complete_login` validates `state` (CSRF) and exchanges the code for a bearer | `client/src/lib.rs` |
| **MCP over browser Fetch** via the high-level `pmcp::Client` | `Client<WasmHttpTransport>` — the bearer is threaded on every request via `extra_headers` | SDK `src/shared/wasm_http.rs` |
| **MCP Tasks lifecycle** (call → poll → result, + cancel) | `call_tool_with_task`, `tasks_get`, `tasks_result`, `tasks_cancel` exposed as `WasmClient` methods; `main.js` runs an explicit 500 ms poll loop | `client/src/lib.rs` + `client/main.js` |
| **No native HTTP stack in the wasm bundle** | `pmcp` is taken with `default-features = false, features = ["wasm"]` — no `hyper`/`tokio`/`axum` in `.wasm` | `client/Cargo.toml` |

Deliberately **out of scope** for this reference (deferred in Phase 103): SSE
server-push, elicitation, sampling, and progress notifications. The transport is
one-shot request/response Fetch; server-initiated streams need an SSE transport.

---

## 2. Architecture — two crates, and why

```
examples/web-channel-client/
├── client/      # WASM cdylib — THIS is what you lift into your web channel
│   ├── src/lib.rs   #   #[wasm_bindgen] WasmClient: PKCE + high-level task helpers
│   ├── main.js      #   redirect handling + explicit 500 ms poll loop + Cancel
│   ├── index.html   #   minimal Login / task-status / Cancel UI
│   ├── serve.py     #   callback-aware static server (maps /callback -> index.html)
│   ├── build.sh     #   wasm-pack build --target web
│   └── src/utils.js #   structured-error glue (JS Error from pmcp::Error)
└── server/      # native binary — a DEMO IdP + MCP server (reference only)
    └── src/main.rs  #   InMemoryOAuthProvider merged with pmcp::axum::router()
```

The split exists because Cargo unifies features **per package build**: a single
crate can't be both a wasm cdylib (`pmcp` `wasm` feature) and a native server
(`pmcp` `full`) without dragging the native HTTP stack into the `.wasm`. For your
web channel, **you only need `client/`** — the `server/` crate is a
self-contained demo so the example runs end-to-end offline. In production your
IdP and your agent's MCP endpoint replace it.

**Runtime flow:**

```
 Browser page (client/, wasm)                     Your infra
 ┌──────────────────────────┐   full-page redirect  ┌──────────────┐
 │ begin_login() ───────────┼──────────────────────▶│  OAuth IdP    │
 │                          │◀── ?code=&state= ──────│ (authorize)   │
 │ complete_login() ────────┼── code+verifier ──────▶│ (token)       │
 │   → bearer in session    │◀── access_token ───────└──────────────┘
 │                          │
 │ connect() → initialize   │   Fetch + Bearer      ┌──────────────┐
 │ invoke_task()/poll/…     ┼──────────────────────▶│ MCP endpoint  │
 │  (WasmHttpTransport)     │◀── JSON / SSE frame ───│ (your agent)  │
 └──────────────────────────┘                       └──────────────┘
```

---

## 3. The WASM client API (what your web channel calls)

All methods are exported to JS via `wasm-bindgen` and take `&self` (see §5 on
re-entrancy). Import with `import init, { WasmClient } from './pkg/…'`.

| JS method | Rust | Purpose |
|---|---|---|
| `new WasmClient()` | `lib.rs:175` | construct; installs panic hook + tracing |
| `begin_login(authorizeUrl, clientId, redirectUri) → url` | `lib.rs:196` | mint PKCE verifier/challenge/state, return the authorize URL to navigate to |
| `await complete_login(tokenUrl, code, state, clientId, redirectUri)` | `lib.rs:239` | validate `state`, exchange `code`+verifier for a bearer, store it |
| `is_logged_in() → bool` | `lib.rs:275` | is a bearer present in this tab's `sessionStorage` |
| `await connect(mcpUrl)` | `lib.rs:285` | build `Client<WasmHttpTransport>` with the bearer, run `initialize` |
| `await invoke_task(name, argsObj) → taskId` | `lib.rs:316` | `tools/call` as a Task; returns the store-minted task id |
| `await poll_task(taskId) → statusString` | `lib.rs:343` | one `tasks/get`; returns `working`/`completed`/`failed`/`cancelled`/`input_required` |
| `await task_result(taskId) → resultJson` | `lib.rs:351` | `tasks/result` once terminal |
| `await cancel_task(taskId) → statusString` | `lib.rs:360` | `tasks/cancel` |
| `logout()` | `lib.rs:368` | clear the bearer + PKCE secrets, drop the client |

The browser orchestration that ties these together — assembling the redirect,
detecting `?code=&state=` on return, and the poll loop — lives in
[`client/main.js`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/web-channel-client/client/main.js),
intentionally explicit and framework-free so you can port it to React/Svelte/etc.

**Primary file to read/lift:**
[`client/src/lib.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/web-channel-client/client/src/lib.rs)
(`examples/web-channel-client/client/src/lib.rs`).

---

## 4. SDK building blocks the client relies on

These live in the `pmcp` crate (feature `wasm`), so you get them by depending on
`pmcp` — you don't copy them:

- **`WasmHttpTransport`** — Fetch-based `Transport`; injects `extra_headers`
  (your `Authorization: Bearer …`) on every request, tracks `mcp-session-id`,
  and accepts **both** JSON and single-frame SSE responses.
  [`src/shared/wasm_http.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/src/shared/wasm_http.rs)
  (`WasmHttpConfig` at `:18`, transport at `:46`).
- **PKCE helper** — target-agnostic RFC 7636 primitives:
  `generate_code_verifier` (`:94`), `code_challenge_s256` (`:115`),
  `generate_state` (`:143`).
  [`src/shared/pkce.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/src/shared/pkce.rs).
- **High-level `pmcp::Client`** — `initialize`, `call_tool_with_task`,
  `tasks_get`/`tasks_result`/`tasks_cancel`.
  [`src/client/mod.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/src/client/mod.rs).
- **JSON-RPC wire codec** (single source of truth for framing) —
  [`src/shared/transport.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/src/shared/transport.rs).

---

## 5. Adapting it into your web channel

The `client/` crate is written to be re-pointed with minimal edits:

1. **Point at your IdP.** The client needs only four values, all passed in at
   call time (nothing IdP-specific is compiled in): the **authorize URL**, the
   **token URL**, a **`client_id`**, and a **`redirect_uri`**. In the demo these
   are constants in `client/main.js` and the `#server-origin` input in
   `index.html`; replace them with your IdP's endpoints. The bundled `server/`
   IdP is only a stand-in — delete it and register the client with your real
   provider. PKCE requires the `redirect_uri` at authorize-time to match a
   registered URI exactly.

2. **Point at your agent's MCP endpoint.** `connect(mcpUrl)` accepts any
   MCP-over-HTTP (streamable-HTTP) endpoint. In pmcp.run that's the agent's
   served endpoint. Multiple agents → one `WasmClient` per endpoint (each holds
   its own bearer + session), or extend the wrapper to hold several.

3. **Thread the bearer.** Handled for you: `connect` puts
   `Authorization: Bearer <token>` into `WasmHttpConfig.extra_headers`, and the
   transport sends it on every Fetch. If your gateway wants a different header,
   change the one line in `connect` (`lib.rs:285`).

4. **Keep or replace the poll loop.** `main.js` polls `tasks/get` every 500 ms
   and renders status transitions. Reuse that pattern, or wire `poll_task` into
   your framework's state/store; the wrapper is UI-agnostic.

5. **Serve the callback.** OAuth returns to `redirect_uri` (`/callback` in the
   demo). Your static host must serve the app HTML for that path — the bundled
   `serve.py` shows the pattern (map `/callback` → `index.html`, `text/html`).

The **server-side** merge pattern is also worth copying if you host the IdP and
MCP on one origin: the demo merges an OAuth IdP with `pmcp::axum::router()` via
the public `axum::Router::merge` seam and maps the IdP-minted user id onto the
MCP `AuthContext` / task owner — see
[`server/src/main.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/web-channel-client/server/src/main.rs)
(`build_merged_router` at `:402`, `BearerAuthAdapter` at `:107`).

---

## 6. Wasm gotchas already solved (so you don't rediscover them)

Browser end-to-end UAT of this example surfaced four issues that are invisible to
`wasm-pack build` and to native HTTP tests; all are fixed in the SDK/example
(see the `[2.11.0]` CHANGELOG entry):

1. **`std::time::Instant::now()` panics on wasm32** ("time not implemented on
   this platform"). `MiddlewareContext` now uses `web_time::Instant`, so every
   `Client::send_request` works in-browser.
2. **Wire framing** — the transport now emits a proper JSON-RPC frame (it used to
   serialize the internal enum, which servers rejected with `-32700`).
3. **SSE tool responses** — the streamable-HTTP server answers `initialize` as
   JSON but streams `tools/call`/`tasks/*` as `text/event-stream`; the transport
   now parses both shapes.
4. **wasm-bindgen re-entrancy** — `WasmClient` methods take `&self` (client behind
   `RefCell<Option<Rc<Client>>>`) so overlapping calls can't trigger the
   "recursive use of an object … unsafe aliasing" abort; contention degrades to a
   graceful "client busy" error.

The upshot: **the transport is browser-functional as of pmcp 2.11.0** — build on
that version or later.

---

## 7. Run it locally (5 steps)

```bash
# 0. one-time
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# 1. build the wasm client
cd examples/web-channel-client/client
./build.sh

# 2. start the bundled demo IdP + MCP server (separate terminal)
cargo run --manifest-path examples/web-channel-client/server/Cargo.toml   # :8787

# 3. serve the client page (callback-aware; NOT plain http.server)
cd examples/web-channel-client/client && python3 serve.py                 # :8080

# 4. open and drive it
open http://127.0.0.1:8080/index.html
# Login (PKCE) → Run slow_summarize task → watch working→completed → Cancel
```

Full walk-through and the redirect-URI rationale:
[`examples/web-channel-client/README.md`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/web-channel-client/README.md).

---

## 8. Known limitations (demo server, not the client)

- The bundled `server/` is a **single-user demo** IdP + MCP server; its
  background task updater assumes one owner (see the MEDIUM-6 note in
  `server/src/main.rs`). Your production MCP servers replace it entirely.
- One-shot Fetch transport → **no server-push** (SSE/notifications). If your
  channel needs streaming progress or elicitation, that's a follow-up transport.

---

## 9. File index

| File | Link |
|---|---|
| Example README | `examples/web-channel-client/README.md` |
| **WASM client (lift this)** | `examples/web-channel-client/client/src/lib.rs` |
| Browser orchestration (redirect + poll loop) | `examples/web-channel-client/client/main.js` |
| UI | `examples/web-channel-client/client/index.html` |
| Callback-aware static server | `examples/web-channel-client/client/serve.py` |
| wasm build script | `examples/web-channel-client/client/build.sh` |
| client crate manifest (`wasm`-only pmcp) | `examples/web-channel-client/client/Cargo.toml` |
| Demo IdP + MCP server (reference) | `examples/web-channel-client/server/src/main.rs` |
| SDK: Fetch transport | `src/shared/wasm_http.rs` |
| SDK: PKCE helper | `src/shared/pkce.rs` |
| SDK: high-level Client | `src/client/mod.rs` |
| SDK: JSON-RPC wire codec | `src/shared/transport.rs` |

GitHub base (resolves once Phase 103 is on `main`):
`https://github.com/paiml/rust-mcp-sdk/tree/main/examples/web-channel-client`.
