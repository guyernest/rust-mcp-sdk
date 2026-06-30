# Phase 103: Web-channel WASM client reference (OAuth browser-PKCE + MCP Tasks) - Pattern Map

**Mapped:** 2026-06-30
**Files analyzed:** 11 new/modified files
**Analogs found:** 10 / 11 (one file — the browser PKCE orchestration JS+wasm glue — is a partial-analog composite)

> Source of truth for this map: `103-CONTEXT.md` (D-01..D-09) + `103-RESEARCH.md`
> (exact file/line findings). RESEARCH already verified every analog with line
> numbers; this map condenses them per-file for the planner/executors.
>
> **Two files are FIXES/ADAPTS, not greenfield:**
> - `src/shared/wasm_http.rs` already contains `WasmHttpTransport` with a BROKEN
>   `Transport` impl (send discards response, receive errors) — D-08 = repair it.
> - The demo long-task tool is a NEW pattern (no prior analog completes after a
>   delay over the HTTP create-path); adapt `s46` + the TaskStore API.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/shared/wasm_http.rs` (MODIFY — fix `WasmHttpTransport::{send,receive}`) | transport | request-response | `src/shared/wasm_websocket.rs` `WasmWebSocketTransport` Transport impl | role-match (same trait, different I/O model — one-shot buffer vs channel) |
| `src/shared/pkce.rs` (NEW — pure PKCE crypto helper, D-02) | utility | transform | `src/client/oauth.rs` `generate_code_verifier`/`generate_code_challenge` (592-604) | exact (same logic, swap `rand`→`getrandom`) |
| `src/lib.rs` (MODIFY — re-export `pkce`) | config | n/a | existing `pub use shared::{WasmHttp...}` (lib.rs:139) + `pub use shared::StdioTransport` (100) | exact |
| `src/shared/mod.rs` (MODIFY — `pub mod pkce;`) | config | n/a | existing `pub mod` block (shared/mod.rs:3-29) | exact |
| `examples/web-channel-client/src/lib.rs` (NEW — `#[wasm_bindgen] WasmClient`) | component (wasm) | request-response | `examples/wasm-client/src/lib.rs` (538-line `WasmClient`) | role-match (mirror structure; SWAP raw `WasmHttpClient` for high-level `Client<WasmHttpTransport>`) |
| `examples/web-channel-client/src/bin/demo_server.rs` (NEW — bundled offline server + IdP, D-04/D-05) | server | request-response + event-driven (bg task) | `examples/s46_http_tool_as_task.rs` (server stand-up) + `src/server/auth/oauth2.rs` `InMemoryOAuthProvider` + `src/server/auth/mock.rs` `MockValidator` | role-match (s46 is synchronous; this adds delayed task + hand-wired OAuth routes) |
| `examples/web-channel-client/Cargo.toml` (NEW) | config | n/a | `examples/wasm-client/Cargo.toml` | exact (+ `web-sys` Storage/Location/UrlSearchParams; + `[[bin]]` for demo server) |
| `examples/web-channel-client/build.sh` (NEW) | config | n/a | `examples/wasm-client/build.sh` | exact |
| `examples/web-channel-client/index.html` (NEW — UI + status line + Cancel button, D-09) | component | n/a | `examples/wasm-client/index.html` | role-match |
| `examples/web-channel-client/main.js` (NEW — `?code=&state=` detect + 500ms poll loop, D-07/D-09) | component | event-driven | `examples/wasm-client/main.js` | partial (mirror init glue; the redirect-detect + poll loop are NEW, no direct analog) |
| `examples/web-channel-client/style.css` (NEW) | config | n/a | `examples/wasm-client/style.css` | exact |

## Pattern Assignments

### `src/shared/wasm_http.rs` — FIX `WasmHttpTransport::{send,receive}` (D-08, WEBCH-02)

**Analog:** `src/shared/wasm_websocket.rs` `WasmWebSocketTransport` (the only working
wasm `Transport` impl). The HTTP transport cannot reuse the websocket channel model
(no server-push stream); instead it buffers the one-shot Fetch response in `send()`
and pops it in `receive()`. The `do_request` helper already exists and returns a
parsed `TransportMessage`; only the trait impl is broken.

**Trait surface to mirror** — `wasm_websocket.rs:126-162`:
```rust
#[async_trait(?Send)]
impl Transport for WasmWebSocketTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> { /* ws.send */ Ok(()) }
    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx.next().await.ok_or_else(|| Error::Transport(TransportError::ConnectionClosed))
    }
    async fn close(&mut self) -> Result<()> { /* ws.close */ Ok(()) }
}
```

**Existing BROKEN impl to replace** — `wasm_http.rs:190-217` (send discards
`_response`; receive returns hard error `"HTTP transport requires send() before receive()"`).

**Existing helper to reuse (do NOT rewrite)** — `wasm_http.rs:72-82`:
```rust
async fn do_request(&mut self, message: &TransportMessage) -> Result<TransportMessage> {
    let body = serde_json::to_string(message)...?;
    let response_text = self.do_http_request(&body).await?;
    serde_json::from_str(&response_text)...        // already returns a parsed TransportMessage
}
```
`do_http_request` (wasm_http.rs:85-187) already injects `extra_headers`
(117-121 — bearer goes here), `mcp-session-id` (103-107), and updates `session_id`
from the response (167-169). **Bearer injection works for free** once correlation is fixed.

**Fix shape** (add a one-slot buffer field to the struct at `wasm_http.rs:44-49`):
```rust
pub struct WasmHttpTransport {
    config: WasmHttpConfig,
    session_id: Option<String>,
    protocol_version: Option<String>,
    pending_response: Option<TransportMessage>, // NEW: one-slot response buffer
}
#[async_trait(?Send)]
impl Transport for WasmHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.pending_response = Some(self.do_request(&message).await?); // POST + BUFFER
        Ok(())
    }
    async fn receive(&mut self) -> Result<TransportMessage> {
        self.pending_response.take()
            .ok_or_else(|| Error::internal("receive() called before send() on HTTP transport"))
    }
    async fn close(&mut self) -> Result<()> { Ok(()) }
}
```

**Why it must work this way:** `Client::send_request` (client/mod.rs:1981-1998) calls
`transport.send(msg)` ONCE then loops on `transport.receive()` until it gets a
`TransportMessage::Response`. The current impl makes `Client.initialize()` fail
immediately. The `WasmHttpClient` wrapper (wasm_http.rs:226-265, raw path used by the
old example) stays untouched for backward compat — only the `Transport` impl changes.

**cfg discipline:** entire file is `#![cfg(target_arch = "wasm32")]` (wasm_http.rs:6).
The fix must NOT regress the non-wasm host build (it can't — file is wasm-only) but
MUST compile under `make wasm-build` (Pitfall 5).

---

### `src/shared/pkce.rs` — NEW pure PKCE crypto helper (D-02/D-03, WEBCH-01)

**Analog:** `src/client/oauth.rs:592-604` (the `OAuthClient` private methods).
Same RFC 7636 logic; the ONLY change is the RNG source (native uses `rand`, which is
an optional `oauth`-feature dep that won't build on the wasm target — swap to
`getrandom::fill`, already a wasm dep, Cargo.toml:126).

**Analog logic (lift verbatim, change RNG)** — `oauth.rs:592-604`:
```rust
fn generate_code_verifier() -> String {
    let random_bytes: [u8; 32] = rand::rng().random();   // <-- replace with getrandom::fill
    URL_SAFE_NO_PAD.encode(random_bytes)
}
fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}
```

**Imports to copy** — `oauth.rs:19-22`:
```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
// NEW for wasm-safe RNG (do NOT use `rand` here):
// getrandom::fill(&mut bytes) -> Result<(), getrandom::Error>
```

**State reuse note:** native code reuses the verifier generator for the CSRF `state`
(`oauth.rs:672`: `.append_pair("state", &Self::generate_code_verifier())`) — mirror
that (`generate_state()` = same entropy source).

**Module gating:** target-agnostic (compiles on BOTH host + wasm). Add `pub mod pkce;`
to `src/shared/mod.rs` WITHOUT a cfg gate (contrast `peer`/`stdio` which are
`#[cfg(not(target_arch = "wasm32"))]` at shared/mod.rs:11-27). It needs only
`sha2`/`base64` (non-optional, Cargo.toml:81-82) + `getrandom` (wasm dep) — NO
feature gate (Pitfall 6).

**Pitfall 7 (check-unwraps gate):** `getrandom::fill` returns `Result`. Return
`Result<String>` from helpers (map to `pmcp::Error`) rather than `.expect()`/`.unwrap()`
in `src/` — `make quality-gate` runs `check-unwraps` (Makefile:673).

**ALWAYS coverage (this is THE fuzz/property target):** RFC 7636 §appendix-B vector
unit test; verifier 43-char base64url charset property test; S256 determinism;
base64url roundtrip proptest. (RESEARCH Validation Architecture WEBCH-01.)

---

### `src/lib.rs` + `src/shared/mod.rs` — re-export the new helper

**Analog (mod.rs):** the `pub mod` list at `shared/mod.rs:3-29`. Add ungated
`pub mod pkce;`.

**Analog (lib.rs):** existing re-export lines. The wasm transports re-export at
`lib.rs:139` (`pub use shared::{WasmHttpClient, WasmHttpConfig, WasmHttpTransport};`,
inside a `#[cfg(target_arch = "wasm32")]` block); the always-available
`pub use shared::StdioTransport;` is at lib.rs:100. PKCE is target-agnostic, so
re-export it in an UNGATED block (model on lib.rs:100, NOT lib.rs:139). RESEARCH
"Release Impact" suggests `pmcp::...::pkce::{generate_code_verifier, code_challenge_s256, generate_state}`.

---

### `examples/web-channel-client/src/lib.rs` — NEW `#[wasm_bindgen] WasmClient` (D-01/D-06/D-08, WEBCH-03/06)

**Analog:** `examples/wasm-client/src/lib.rs` (mirror structure). **Key divergence:**
the existing example's HTTP path uses the RAW `WasmHttpClient` + hand-built
`to_jsonrpc()` (wasm-client/lib.rs:18-23, 137-154); the NEW example uses the
HIGH-LEVEL `Client<WasmHttpTransport>` so all four typed task helpers work over Fetch
(the D-08 win).

**Wasm boilerplate to copy verbatim** — `wasm-client/lib.rs:25-105`:
```rust
#[wasm_bindgen]
extern "C" { #[wasm_bindgen(typescript_type = "Error")] pub type JsError; }
// StructuredError + From<pmcp::Error> + to_js_error(err) -> JsValue   (lib.rs:37-69)
#[wasm_bindgen]
pub struct WasmClient { /* ... */ }
#[wasm_bindgen]
impl WasmClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        static INIT_TRACING: std::sync::Once = std::sync::Once::new();
        INIT_TRACING.call_once(|| tracing_wasm::set_as_global_default());
        Self { /* ... */ }
    }
}
```

**Connect pattern — REPLACE the raw HTTP branch (wasm-client/lib.rs:134-164) with
high-level Client** (model on the WebSocket branch at wasm-client/lib.rs:123-133):
```rust
let config = WasmHttpConfig {
    url,
    extra_headers: vec![("Authorization".into(), format!("Bearer {token}"))], // wasm_http.rs:117-121 injects
};
let mut client = Client::new(WasmHttpTransport::new(config)); // <-- the fixed transport
client.initialize(ClientCapabilities::default()).await.map_err(to_js_error)?;
self.http_client = Some(client);
```

**Task helpers to expose to JS** (each = one Fetch round-trip; signatures from
client/mod.rs:508-620):
```rust
client.call_tool_with_task("slow_summarize".into(), json!({})).await? // -> ToolCallResponse::Task(t).task_id
client.tasks_get(&task_id).await?      // poll; Task.status.is_terminal()
client.tasks_result(&task_id).await?   // CallToolResult after terminal
client.tasks_cancel(&task_id).await?   // Cancel button
```

**PKCE + storage (example-level, D-01):** call the new `pmcp` pkce helper for
verifier/challenge/state; store via `web-sys` `Storage` (sessionStorage). The native
auth-URL assembly at `oauth.rs:660-672` (append `code_challenge`,
`code_challenge_method=S256`, `state`) is the reference for the query params the JS/wasm
must build — do NOT cfg-port that module (it's `reqwest`+`TcpListener` bound, D-01).

---

### `examples/web-channel-client/src/bin/demo_server.rs` — NEW bundled offline server (D-04/D-05, WEBCH-04/05)

**Primary analog:** `examples/s46_http_tool_as_task.rs` (server stand-up + lifecycle).
**Crucial difference (D-05):** s46's tool completes SYNCHRONOUSLY (returns a nested
`result`, s46:75-87, so the create-path immediately sets Completed). The demo needs a
tool that returns `status:"working"` (NO nested result) so the store mints a *Working*
task, plus a background updater that completes it after a delay — NO prior analog.

**Server stand-up to copy** — `s46:93-119`:
```rust
let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
let server = Server::builder()
    .name("web-channel-demo").version("1.0.0")
    .tool("slow_summarize", long_task)
    .task_store(Arc::clone(&store))   // presence of store auto-advertises `tasks`
    // .auth_provider(...)            // bearer validation (see below)
    .build()?;
let server = Arc::new(Mutex::new(server));
let bind: SocketAddr = "127.0.0.1:0".parse()?;          // EPHEMERAL PORT (s46:106)
let (bound, server_handle) = StreamableHttpServer::new(bind, server).start().await?;
// ... server_handle.abort() on shutdown (s46:114-116)
```

**Tool tool-shape analog (sync, for the wire fields)** — `s46:70-91`
(`TypedTool::new_with_schema` + `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))`).
The demo tool returns `"status":"working"` instead of `"completed"+result`.

**Background updater (NEW pattern, D-05)** — uses the `TaskStore` API directly. Trait
methods (task_store.rs):
```rust
store.list(owner, None).await        // (266-270) -> (Vec<Task>, Option<cursor>) — find most-recent Working
store.set_result(&task_id, owner, CallToolResult::new(...)).await  // (320-330)
store.update_status(&task_id, owner, TaskStatus::Completed, None).await  // (254-260)
store.cancel(&task_id, owner).await  // (273) — used by tasks/cancel path
```
The updater must DISCOVER the store-minted id via `list` (the id is minted in
`build_task_created_response` AFTER the tool handler returns — see RESEARCH Pattern 3
& Pitfall 3; the `list`-most-recent-Working heuristic is acceptable for a single-user
demo, A5).

**OAuth IdP (D-04) — `StreamableHttpServer` does NOT serve IdP routes (Pitfall 4).**
Use `InMemoryOAuthProvider` (oauth2.rs:377, `::new(base_url)` at 403-416) as both the
IdP and the bearer `TokenValidator`. Trait methods to drive from hand-wired axum
routes (oauth2.rs:318-360):
```rust
provider.validate_authorization(&req).await?;                 // 326
provider.create_authorization_code(client_id, user_id, redirect_uri, scopes, Some(challenge), Some("S256")).await? // 329-337
provider.exchange_code(&token_req).await?    // 340 -> AccessToken (code -> bearer)
provider.validate_token(&token).await?       // 357 -> TokenInfo (server-side bearer check)
provider.metadata().await?                   // 360 -> OAuthMetadata (discovery)
```
PKCE verification is built in (`verify_pkce`, oauth2.rs:431-447, S256 via sha2/base64).
**Open Question 1 (RESEARCH):** merge `/oauth2/authorize` + `/oauth2/token` axum routes
with the MCP router onto ONE origin (prefer the lower-level `pmcp::axum` router builder,
streamable_http_server.rs:286) to avoid CORS — spike in Wave 0.

**Alternative bearer validator (simpler than oauth2):** `MockValidator`
(mock.rs:57-82, `::new(user_id)` + `impl TokenValidator` at mock.rs:258-259
`async fn validate(&self, token) -> Result<AuthContext>`). Use if the planner decouples
"IdP that issues tokens" from "server-side bearer check."

**cfg discipline:** `#![cfg(not(target_arch = "wasm32"))]` (mirror s46:49) — the demo
server is a NATIVE binary built with `streamable-http`/`full`, NOT wasm (Pitfall 6).

---

### Example build harness files (Cargo.toml, build.sh, index.html, main.js, style.css)

**Analogs (mirror `examples/wasm-client/`):**

**`Cargo.toml`** — base on `examples/wasm-client/Cargo.toml`:
```toml
[lib]
crate-type = ["cdylib"]
[package.metadata.wasm-pack]
wasm-opt = false
[dependencies]
pmcp = { path = "../..", default-features = false, features = ["wasm"] }  # NOT http/streamable-http (Pitfall 6)
wasm-bindgen = "0.2"; wasm-bindgen-futures = "0.4"; serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"; tracing-wasm = "0.2"
web-sys = { version = "0.3", features = ["console", "WebSocket",
    "Storage", "Window", "Location", "UrlSearchParams"] }  # NEW vs wasm-client (D-06/D-07)
[dev-dependencies]
wasm-bindgen-test = "0.3"
```
**Additions vs analog:** the extra `web-sys` features above, AND a `[[bin]]` (or a
separate native dep set) for `demo_server.rs` which needs `pmcp` with
`streamable-http`/`full` — the wasm `cdylib` and the native `bin` have DIFFERENT
feature requirements; the planner must structure this so the wasm lib build does not
pull hyper/tokio (Pitfall 6). May warrant the demo server as a sibling crate or a
target-gated `[[bin]]`.

**`build.sh`** — copy verbatim from `wasm-client/build.sh` (changes only the
`--out-name`):
```bash
export CARGO_PROFILE_RELEASE_LTO=false
wasm-pack build --target web --out-name <name> --no-opt
cp pkg/<name>_bg.wasm . && cp pkg/<name>.js .
```
**Pitfall 2 / A2:** if `make wasm-build` fails on `getrandom`, add
`examples/web-channel-client/.cargo/config.toml` with
`[target.wasm32-unknown-unknown] rustflags = ['--cfg','getrandom_backend="wasm_js"']`
(verify exact getrandom version with `cargo tree -p getrandom`).

**`index.html`** — mirror `wasm-client/index.html`; ADD a task status line
(`Working → Completed`) and a **Cancel button** (D-09).

**`main.js`** — mirror `wasm-client/main.js` for the wasm-init glue; ADD (NO direct
analog): (1) on load, read `window.location.search` for `?code=&state=` and resume the
flow (D-07); (2) a fixed ~500ms `setTimeout` poll loop calling the wasm `tasks_get`
method until terminal, then `tasks_result` (D-09 — keep it explicit/visible, do NOT
hide in a helper).

**`style.css`** — copy from `wasm-client/style.css`.

## Shared Patterns

### WASM cfg-gating discipline
**Source:** `src/shared/wasm_http.rs:6` + `src/shared/mod.rs:11-27` + `examples/s46:49`
**Apply to:** ALL new files.
- wasm-only code: `#![cfg(target_arch = "wasm32")]` (the transport fix stays here).
- native-only code: `#![cfg(not(target_arch = "wasm32"))]` (the demo server).
- target-agnostic code: NO cfg gate (the PKCE helper — it compiles on both).
- Re-exports follow the same gate as the item (lib.rs:139 wasm-gated transports vs
  lib.rs:100 ungated StdioTransport).
**Verification (Pitfall 5):** BOTH `make quality-gate` (host) AND `make wasm-build`
must pass — wasm is NOT in the default gate (Makefile:58-61 vs 660-679).

### Transport trait shape (`async_trait(?Send)`)
**Source:** `src/shared/wasm_websocket.rs:126-162`
**Apply to:** the `WasmHttpTransport` fix.
```rust
#[async_trait(?Send)]
impl Transport for T {
    async fn send(&mut self, message: TransportMessage) -> Result<()>;
    async fn receive(&mut self) -> Result<TransportMessage>;
    async fn close(&mut self) -> Result<()>;
}
```
`TransportMessage` enum + `Transport` trait defined in `src/shared/transport.rs`.

### PKCE crypto primitives (base64url + S256)
**Source:** `src/client/oauth.rs:19-22, 592-604` (logic) and `src/server/auth/oauth2.rs:431-447` (the verify side, proves the same `sha2`+`base64::URL_SAFE_NO_PAD` convention server-side)
**Apply to:** `src/shared/pkce.rs` (gen side) — the bundled IdP's `verify_pkce`
already speaks the same encoding, so a verifier/challenge from the helper validates
against the demo server out of the box.

### High-level `Client` builder + typed task helpers over any Transport
**Source:** `examples/s46_http_tool_as_task.rs:122-199` (native) + `src/client/mod.rs:508-620` (helpers)
**Apply to:** the wasm example's `WasmClient` — once `WasmHttpTransport` is fixed, the
SAME typed helper calls used by the native s46 example run in the browser.

### Server builder + auto-advertised tasks
**Source:** `examples/s46_http_tool_as_task.rs:93-100`
**Apply to:** `demo_server.rs`. `.task_store(store)` presence auto-advertises the
`tasks` capability (no manual capability wiring).

### Frozen `tasks/*` wire contract
**Source:** `tests/tool_as_task_lifecycle_http.rs` (live HTTP shapes) — Phase 101 froze
it, Phase 102 delivered HTTP dispatch (server/mod.rs:1165-1177, verified 7/7).
**Apply to:** the demo server tool + the browser flow. Do NOT change the wire shapes;
the store mints the canonical id (assert `task_id != "tool-fabricated"`, s46:161-165).

## No Analog Found

| File | Role | Data Flow | Reason / Mitigation |
|------|------|-----------|---------------------|
| `examples/.../main.js` (redirect-detect + 500ms poll loop) | component | event-driven | No browser PKCE-redirect or task-poll-loop JS exists in the repo. Build from RESEARCH "Browser PKCE flow shape" + "Driving the task lifecycle over Fetch" code sketches (RESEARCH §Code Examples). The wasm-init half mirrors `wasm-client/main.js`. |
| Time-delayed `Working→Completed` task tool (inside `demo_server.rs`) | service | event-driven | NO existing tool completes after a delay over the HTTP create-path (s46 is synchronous; `stay_pending` never completes — Pitfall 3). Build from RESEARCH Pattern 3 (tool returns `working` + `tokio::spawn` updater that discovers the id via `store.list(owner)`). Add a server-side integration test modeled on `tests/tool_as_task_lifecycle_http.rs`. |

## Metadata

**Analog search scope:** `src/shared/` (wasm_http, wasm_websocket, transport, mod),
`src/client/` (mod task helpers, oauth, auth gating), `src/server/` (task_store,
task_dispatch, mod, auth/oauth2, auth/mock, streamable_http_server), `examples/`
(s46, wasm-client lib/build/Cargo/index/main/css, s29 oauth), `src/lib.rs`.
**Files scanned:** ~16 source files + 6 example-harness files (verified line numbers).
**Pattern extraction date:** 2026-06-30
