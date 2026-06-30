# Phase 103: Web-channel WASM client reference (OAuth browser-PKCE + MCP Tasks) - Research

**Researched:** 2026-06-30
**Domain:** Browser WASM MCP client (wasm-bindgen/web-sys), OAuth 2.0 PKCE, MCP Tasks lifecycle over HTTP Fetch, pmcp SDK transport layer
**Confidence:** HIGH (all findings verified against in-repo source with exact line numbers; external crate facts verified against official docs)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Browser-specific OAuth orchestration (full-page redirect, reading `?code=&state=` on return, Fetch-based token exchange, threading bearer into transport headers) lives **in the example** (`examples/web-channel-client/`). Do NOT cfg-port `src/client/auth.rs` / `oauth.rs` wholesale (they are `reqwest` + `tokio::net::TcpListener` bound).
- **D-02:** Extract the **pure PKCE crypto primitives** (code_verifier gen, S256 code_challenge, state/nonce) into a **wasm-safe helper in the `pmcp` crate** shared by native + browser. Ship in a new `pmcp` release. Guardrail: keep it low-complexity; if making `oauth.rs` cfg-clean drags in `reqwest`/`tokio`, ship a minimal standalone helper instead.
- **D-03:** SDK scope is **crypto helper only** — NO full `#[cfg(wasm32)]` browser-PKCE orchestrator (auth-URL builder / callback handler) in `pmcp` this phase. Orchestration stays in the example.
- **D-04:** Bundle a **self-contained demo `StreamableHttpServer`** in the example, using `src/server/auth/mock.rs` + `oauth2` as the IdP. Runs fully offline (no external accounts/network/secrets), CI-testable.
- **D-05:** The bundled server exposes **one simulated long task** (a `TaskSupport::Required` tool that transitions `Working → Completed` over a few seconds via the `TaskStore`), so the browser polls `tasks/get` several times before `tasks/result`, and `tasks/cancel` is demonstrable. Wire shapes still mirror `s46` / `tests/tool_as_task_lifecycle_http.rs` (Phase 101 froze the contract — do NOT change it).
- **D-06:** Use **sessionStorage** (via `web-sys`) for `code_verifier` + OAuth `state` (must survive redirect) and the bearer token. (IndexedDB noted as production upgrade — NOT this phase.)
- **D-07:** Drive auth via **full-page redirect** (`window.location = authorize_url`); on load, JS detects `?code=&state=` and resumes. (NOT popup/postMessage.)
- **D-08:** Add a new **`WasmHttpTransport` implementing the `Transport` trait** to `pmcp` under `src/shared/` (symmetric with `WasmWebSocketTransport`, built atop the existing `WasmHttpClient` Fetch code). Lets the **high-level `Client`** + its typed task helpers run over browser Fetch. Ships in the same new `pmcp` release as the PKCE helper.
- **D-09:** The browser polls `tasks/get` on a **fixed ~500ms interval** (`setTimeout` / `gloo-timers`) until terminal, updating a visible status line, with a **Cancel button** (drives `tasks/cancel`) in `index.html`. (NOT exponential backoff; NOT a hidden auto-poll helper — the explicit poll loop is what the demo teaches.)

### Claude's Discretion
- Example crate layout, `build.sh`, `index.html`/`main.js`/`style.css` structure: mirror `examples/wasm-client` unless a better pattern emerges.
- Error / expired-token / state-mismatch UX surfacing in the demo UI.
- Exact name of the simulated long-task tool and its argument shape.
- Specific wasm crypto crates (`getrandom` js backend + `sha2` + base64url **vs** `web-sys` SubtleCrypto) — pick the lower-complexity, smaller-binary option. **(Research recommends `getrandom` + `sha2` + `base64` — see Standard Stack + Pitfall 2.)**
- ALWAYS-coverage test targets (unit + property + fuzz where applicable) — the working browser example IS the EXAMPLE deliverable.

### Deferred Ideas (OUT OF SCOPE)
- IndexedDB token persistence across tabs/restarts.
- Full `#[cfg(wasm32)]` browser-PKCE orchestrator in `pmcp` (auth-URL builder + callback handler as a turnkey SDK API).
- Real external OAuth provider integration (Google/GitHub/Auth0) as a first-class runnable mode.
- Popup/postMessage redirect flow.
- Promoting the web-channel client to a published `crates/` library (LOCKED out by ROADMAP — example only).
- Exponential backoff polling.
- **Out of scope per ROADMAP success criteria:** streaming HTTP / SSE, elicitation, sampling, progress. Tasks works via request/response polling, so SSE is NOT required.
</user_constraints>

<phase_requirements>
## Phase Requirements

No formal REQ-IDs are assigned yet (ROADMAP says "TBD (assign during plan-phase)"). Coverage is derived from the 4 ROADMAP success criteria (SC) and decisions D-01..D-09. Suggested REQ-ID scheme for the planner to adopt:

| Suggested ID | Description (from SC + decisions) | Research Support |
|--------------|-----------------------------------|------------------|
| WEBCH-01 | PKCE crypto helper compiles for wasm32 + native; produces RFC 7636-conformant verifier/challenge/state (D-02, D-03, SC-1) | "PKCE Helper Extraction" + Code Examples + Validation Architecture |
| WEBCH-02 | `WasmHttpTransport: Transport` carries `Client` + task helpers over Fetch; bearer injected via `extra_headers` (D-08, SC-1, SC-2) | "Transport Trait Surface" + Pitfall 1 (the existing-impl bug) |
| WEBCH-03 | Example performs full-page redirect PKCE flow; tokens in sessionStorage; bearer threaded to transport (D-01, D-06, D-07, SC-1) | "Browser PKCE Orchestration" + Architecture Patterns |
| WEBCH-04 | Bundled self-contained `StreamableHttpServer` + OAuth2 IdP runs offline, validates bearer (D-04, SC-3) | "Bundled Demo Server" + Pitfall 4 (IdP HTTP routes) |
| WEBCH-05 | Bundled server exposes ONE multi-second `Working→Completed` task; browser polls `tasks/get` ≥2× then `tasks/result`; `tasks/cancel` works (D-05, SC-2) | "Multi-Second Task Design" (the central novel-pattern finding) + Pitfall 3 |
| WEBCH-06 | Browser drives `call(task) → poll tasks/get @500ms → tasks/result` + Cancel button; wire shapes mirror frozen contract (D-09, SC-2) | "Tasks Wire Shapes (Frozen)" |
| WEBCH-07 | Runs end-to-end in a browser (build.sh + index.html), documented for pmcp.run adaptation (SC-3) | "WASM Build & Boundary" |
| WEBCH-08 | ALWAYS coverage (unit+property+fuzz where applicable); `make quality-gate` green; non-wasm builds NOT regressed (SC-4) | "Validation Architecture" + Pitfall 5 (wasm not in quality-gate) |
| WEBCH-09 | New `pmcp` release packages D-02 + D-08 public API (CLAUDE.md Release Workflow) | "Release Impact" |
</phase_requirements>

## Summary

This phase is **~80% wiring of existing, verified building blocks** plus **two genuinely-novel design problems** the planner must solve carefully. The good news, discovered in source: the SDK already has every transport and crypto primitive needed — `WasmHttpClient` (Fetch request/response with `extra_headers` for the bearer), `WasmWebSocketTransport` (the `Transport` impl to mirror), `InMemoryTaskStore` + the Phase-102 HTTP `tasks/*` dispatch, `InMemoryOAuthProvider` (full OAuth2 IdP), `MockValidator` (bearer `TokenValidator`), and `sha2`/`base64`/`getrandom` already in the dependency tree. The frozen `tasks/*` wire contract is exercised live in `tests/tool_as_task_lifecycle_http.rs` and `examples/s46_http_tool_as_task.rs`.

The **two non-trivial findings** that dominate planning risk:

1. **`WasmHttpTransport` already exists in `src/shared/wasm_http.rs` (lines 44-223) and its `Transport` impl is BROKEN by design** — `send()` performs the HTTP POST and **discards the response**, and `receive()` returns a hard error (`"HTTP transport requires send() before receive()"`). The high-level `Client::send_request` (src/client/mod.rs:1981-1985) calls `send(msg)` THEN loops on `receive()` to correlate the response. So D-08 is not "add a new transport" — it is **fix the existing one** to buffer the POST response in `send()` and pop it in `receive()` (an internal one-slot response queue). This is the linchpin that makes `Client` + all four task helpers work over Fetch.

2. **A genuinely time-delayed `Working → Completed` task over the HTTP create-path has no existing example or test** — every current task tool (`s46`, both lifecycle tests) either completes synchronously (returns a nested `result` so `build_task_created_response` immediately calls `set_result` + `update_status(Completed)`, src/server/task_dispatch.rs:260-275) or stays pending forever (`stay_pending` returns `working` with no result and nothing ever updates it). D-05 requires a NEW pattern: the tool returns `status: "working"` (so the store mints a Working task), and a **background `tokio::spawn` task that closes over `Arc<dyn TaskStore>` + the resolved owner** sleeps a few seconds then calls `set_result` + `update_status(Completed)`. The subtlety: the tool handler runs BEFORE the store mints the id (the id is minted in `build_task_created_response` AFTER the handler returns), so the background updater must discover the task via `store.list(owner, None)` (most-recent Working task) rather than receive the id directly.

**Primary recommendation:** Treat D-08 as a **fix-and-correlate** of the existing `WasmHttpTransport` (internal `Option<TransportMessage>` response slot), implement the D-02 PKCE helper with `getrandom::fill` + `sha2::Sha256` + `base64::URL_SAFE_NO_PAD` (all sync, zero new wasm bloat — reject SubtleCrypto which is async and adds web-sys features), and design the D-05 multi-second task as a tool that returns `working` + spawns a store-updating background task keyed by `store.list(owner)`. The PKCE helper is the strongest ALWAYS-coverage target (pure crypto → property + fuzz).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| PKCE verifier/challenge/state generation | pmcp SDK (wasm-safe helper, D-02) | Example (calls helper) | Pure crypto must be reusable by native loopback flow AND browser; lives in crate per D-02 |
| Authorization redirect + `?code=&state=` handling | Browser/Client (example JS + wasm) | — | Browser-only orchestration, structurally different from native loopback (D-01/D-03) |
| Token exchange (code → bearer) over Fetch | Browser/Client (example wasm) | — | Browser fetch to IdP `/oauth2/token`; example-level per D-01 |
| Token + verifier + state storage | Browser/Client (sessionStorage via web-sys, D-06) | — | Must survive full-page redirect round-trip |
| MCP JSON-RPC transport over Fetch | pmcp SDK (`WasmHttpTransport`, D-08) | — | The reusable "call MCP from a web app" win; carries high-level `Client` |
| `tasks/*` lifecycle dispatch (get/result/list/cancel) | API/Backend (Phase-102 `task_dispatch`, src/server/mod.rs:1165-1177) | — | Server-side, already shipped; browser just drives it |
| Task state persistence + Working→Completed transition | API/Backend (`TaskStore` + background updater, D-05) | — | Store owns task state; background task drives the time-delayed transition |
| Bearer validation on MCP requests | API/Backend (`StreamableHttpServer` auth_provider, src/server/streamable_http_server.rs:758-770) | — | Server validates `Authorization` header against `TokenValidator` |
| OAuth2 IdP endpoints (`/oauth2/authorize`, `/oauth2/token`) | API/Backend (example-wired axum routes over `InMemoryOAuthProvider`, D-04) | — | **NOT auto-served** by `StreamableHttpServer` — the example must hand-wire these (see Pitfall 4) |
| Poll loop @500ms + Cancel button | Browser/Client (example JS, D-09) | — | Explicit, visible — teaches the lifecycle |

## Standard Stack

### Core (all ALREADY in the pmcp dependency tree — no new crate adds required for the SDK side)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sha2` | 0.11 (Cargo.toml:81, **non-optional**) | S256 code_challenge `SHA-256(verifier)` | `[VERIFIED: in-tree, src/client/oauth.rs:22,600-603]` Pure Rust, sync, works on wasm32, already used by native PKCE |
| `base64` | 0.22 (Cargo.toml:82, **non-optional**) | `URL_SAFE_NO_PAD` encoding of verifier + challenge | `[VERIFIED: in-tree, src/client/oauth.rs:19,595,603]` RFC 7636 base64url; already used by native PKCE |
| `getrandom` | 0.4 + `features=["wasm_js"]` (Cargo.toml:126, wasm target) | Cryptographic random bytes for verifier + state — **sync**, wasm-safe | `[VERIFIED: in-tree wasm dep]` Backs `uuid` js feature already; `getrandom::fill(&mut [u8])` is sync `[CITED: docs.rs/getrandom/0.4.1]` |
| `wasm-bindgen` / `wasm-bindgen-futures` | 0.2 / 0.4 (Cargo.toml:120-121) | wasm boundary, `JsFuture` for Fetch promises | `[VERIFIED: in-tree]` already used by both wasm transports |
| `web-sys` | 0.3 (Cargo.toml:122) | Fetch (`Request`/`Response`/`Headers`), `Window`, `console` | `[VERIFIED: in-tree]` — but **needs added features** for sessionStorage (see below) |
| `js-sys` | 0.3 (Cargo.toml:123) | JS interop | `[VERIFIED: in-tree]` |

### Supporting (example-crate dependencies — mirror `examples/wasm-client/Cargo.toml`)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde-wasm-bindgen` | 0.6 | JS ↔ Rust value conversion in `#[wasm_bindgen]` methods | `[VERIFIED: examples/wasm-client/Cargo.toml]` mirror existing |
| `console_error_panic_hook` | 0.1 | Readable panics in browser console | `[VERIFIED: examples/wasm-client/Cargo.toml]` |
| `tracing-wasm` | 0.2 | wasm tracing → browser console | `[VERIFIED: examples/wasm-client/Cargo.toml]` |
| `gloo-timers` | 0.4 (verified on crates.io) | Optional: Rust-side 500ms timer for poll loop (D-09) | `[ASSUMED]` Only if poll loop lives in Rust. **Recommended alternative: drive the 500ms `setTimeout` from `main.js`** (zero new Rust dep, and D-09 explicitly wants the poll loop visible in the demo's JS) |

### web-sys feature additions required (NOT currently enabled)

The example crate's `web-sys` dependency must enable, beyond what `examples/wasm-client` uses:
- `Storage`, `Window` (`window.sessionStorage()`) — for D-06 token/verifier/state storage `[CITED: docs.rs/web-sys Storage]`
- `Location`, `UrlSearchParams` — for reading `?code=&state=` on redirect return (D-07)

These go in the **example's** `Cargo.toml` (and, if the storage helper is in the example, nowhere in `pmcp`). The PKCE helper in `pmcp` (D-02) needs NO web-sys features — it is pure crypto.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `getrandom` + `sha2` + `base64` (sync) | `web-sys` `Crypto`/`SubtleCrypto` (`window.crypto().subtle().digest()`) | **REJECTED.** SubtleCrypto `digest()` returns a `Promise` → needs `JsFuture` + `async` everywhere `[CITED: docs.rs/web-sys SubtleCrypto; huijzer.xyz/posts/wasm-crypto]`. Adds `Crypto`+`SubtleCrypto` web-sys features, makes the helper async (so it can't be shared with the sync native flow), and is higher-complexity. `sha2` is sync, pure, already in-tree, and works on wasm32. **This resolves the D-02 discretion point: choose `getrandom`+`sha2`+`base64`.** |
| `rand = "0.10"` (native PKCE uses `rand::rng().random()`, oauth.rs:594) | `getrandom::fill` directly | The wasm-safe helper should call `getrandom::fill` directly, NOT `rand`. `rand` is an **optional** dep behind the `oauth` feature (Cargo.toml:88,164) and pulls extra machinery; `getrandom` is the lower layer `rand` itself uses and is already a wasm dep. Native flow can keep using `rand` OR switch to the shared helper. |
| `gloo-timers` for poll loop | `setTimeout` in `main.js` | Prefer JS `setTimeout` — D-09 wants the poll loop visible/explicit in the demo, and it avoids a new Rust dep. |
| New `WasmHttpTransport` | Fix the EXISTING one | The struct + `Transport` impl already exist (wasm_http.rs:44-223) and are already exported (lib.rs:139). D-08 = repair `send`/`receive` correlation, not greenfield. |

**Installation (SDK side):** No new `pmcp` dependencies required — `sha2`, `base64`, `getrandom` are present. The PKCE helper is a new pure-Rust module (`src/shared/pkce.rs` or `src/client/pkce.rs`, gated for both targets) reusing existing deps.

**Example side:** mirror `examples/wasm-client/Cargo.toml`, add `web-sys` features (`Storage`, `Location`, `UrlSearchParams`), and depend on `pmcp` with the wasm feature set (see Pitfall 6 about the feature wiring).

**Version verification (performed this session):**
- `pmcp` published latest = **2.10.0** `[VERIFIED: cargo search pmcp]` — local tree is also 2.10.0 (Cargo.toml:3).
- `getrandom` latest = 0.4.3, `sha2` = 0.11.0, `base64` = 0.22.1, `gloo-timers` = 0.4.0 `[VERIFIED: cargo search]`.

## Package Legitimacy Audit

> The SDK side adds NO new external packages (all crypto deps already in-tree). The example side reuses crates already vetted in `examples/wasm-client`. `slopcheck` was unavailable in this session — packages are tagged accordingly.

| Package | Registry | Age | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-------------|-----------|-------------|
| `sha2` | crates.io | mature (RustCrypto) | github.com/RustCrypto/hashes | unavailable | Approved — already in-tree (Cargo.toml:81) |
| `base64` | crates.io | mature | github.com/marshallpierce/rust-base64 | unavailable | Approved — already in-tree (Cargo.toml:82) |
| `getrandom` | crates.io | mature (rust-random) | github.com/rust-random/getrandom | unavailable | Approved — already in-tree (Cargo.toml:126) |
| `gloo-timers` | crates.io | mature (rustwasm) | github.com/rustwasm/gloo | unavailable | `[ASSUMED]` — OPTIONAL; prefer JS `setTimeout` (no install) |
| `serde-wasm-bindgen`, `console_error_panic_hook`, `tracing-wasm` | crates.io | mature | (rustwasm ecosystem) | unavailable | Approved — already used by `examples/wasm-client` |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

*slopcheck was unavailable; however, every package is already present in the repo's existing, building dependency tree (verified in Cargo.toml / examples/wasm-client/Cargo.toml), which is stronger evidence than a registry probe. The only genuinely-new candidate (`gloo-timers`) is optional and recommended against in favor of JS `setTimeout`. No `checkpoint:human-verify` gate is necessary, but the planner SHOULD still run `make build` + `make wasm-build` as the install-verification step.*

## Architecture Patterns

### System Architecture Diagram

```text
                          BROWSER (examples/web-channel-client, WASM)
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  index.html  ──(load)──► main.js ──► WasmClient (wasm-bindgen)             │
  │     │                        │                                             │
  │     │  detect ?code=&state=  │  pkce helper (pmcp::...::pkce)              │
  │     │  on redirect return    │   verifier/challenge/state (sync, getrandom)│
  │     ▼                        ▼                                             │
  │  [1] full-page redirect   sessionStorage (web-sys): verifier, state, token │
  │   window.location =          │                                             │
  │   authorize_url(challenge)   │ [3] token exchange (Fetch POST /oauth2/token)│
  │       │                      │      code + verifier ──► bearer token       │
  │       │                      ▼                                             │
  │       │              [4] Client<WasmHttpTransport>  (high-level pmcp)      │
  │       │                   extra_headers: Authorization: Bearer <token>     │
  │       │                   call_tool_with_task / tasks_get / tasks_result / │
  │       │                   tasks_cancel  (each = one Fetch request/response)│
  └───────┼──────────────────────────┼─────────────────────────────────────────┘
          │ [2] GET /oauth2/authorize │ [5..N] POST / (JSON-RPC tasks/*)
          ▼                           ▼          @500ms poll loop (D-09)
  ┌──────────────────────────────────────────────────────────────────────────┐
  │            BUNDLED DEMO SERVER (binary in the example, native, offline)    │
  │                                                                            │
  │  axum Router (hand-wired by the example — see Pitfall 4):                  │
  │    /oauth2/authorize ─► InMemoryOAuthProvider.authorize() ─► redirect+code │
  │    /oauth2/token     ─► InMemoryOAuthProvider.exchange_code() ─► bearer    │
  │                                                                            │
  │  StreamableHttpServer (POST /) ──► validates Authorization: Bearer        │
  │     auth_provider (TokenValidator: MockValidator/oauth2) [streamable:758]  │
  │       │                                                                    │
  │       ▼ (post-auth)                                                        │
  │  Server::handle_request ──► task_dispatch (mod.rs:1165-1177)              │
  │       tasks/get|result|list|cancel  ──► InMemoryTaskStore                  │
  │       call(task) ──► long-task tool returns "working" ──► store mints      │
  │              Working task                                                  │
  │                  └─► tokio::spawn background updater (D-05):               │
  │                        sleep(N s) ► store.set_result ► update_status(Completed)│
  └──────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```text
examples/web-channel-client/
├── Cargo.toml          # crate-type=["cdylib"] + bin for demo server; pmcp wasm features + web-sys Storage/Location/UrlSearchParams
├── build.sh            # wasm-pack build --target web (mirror examples/wasm-client/build.sh)
├── index.html          # connect/login UI, task status line, Cancel button (D-09)
├── main.js             # init wasm; detect ?code=&state=; drive 500ms setTimeout poll loop
├── style.css           # mirror examples/wasm-client/style.css
├── src/
│   ├── lib.rs          # #[wasm_bindgen] WasmClient: PKCE flow + Client<WasmHttpTransport> task helpers
│   └── bin/
│       └── demo_server.rs   # self-contained StreamableHttpServer + axum OAuth2 routes + long-task tool (D-04/D-05)
└── README.md           # how pmcp.run adapts this as a web-app channel (SC-3)

# pmcp crate additions (the two reusable pieces — ship in new release):
src/shared/pkce.rs      # D-02: pure PKCE helper (gated for BOTH targets, no reqwest/tokio)
src/shared/wasm_http.rs # D-08: FIX send()/receive() correlation in the EXISTING WasmHttpTransport
```

### Pattern 1: Transport send/receive correlation for one-shot Fetch (D-08 — THE central SDK fix)
**What:** The high-level `Client` correlates request↔response by calling `transport.send(msg)` then looping on `transport.receive()` until it sees a `TransportMessage::Response` (src/client/mod.rs:1981-1998). A one-shot Fetch transport must adapt this bidirectional-stream model: do the POST in `send()` and **buffer the response**, then return it from `receive()`.
**When to use:** Always, for `WasmHttpTransport`. The current impl (wasm_http.rs:190-217) does the POST in `send()` and **throws the response away**, then `receive()` errors — so `Client` over Fetch cannot work today.
**Example:**
```rust
// Source: derived from src/client/mod.rs:1981-1998 (correlation loop) +
//         src/shared/wasm_http.rs:72-82 (do_request returns the parsed response)
// FIX shape for the existing WasmHttpTransport:
pub struct WasmHttpTransport {
    config: WasmHttpConfig,
    session_id: Option<String>,
    protocol_version: Option<String>,
    pending_response: Option<TransportMessage>, // NEW: one-slot response buffer
}

#[async_trait(?Send)]
impl Transport for WasmHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // POST and BUFFER the response (do_request already parses TransportMessage)
        let response = self.do_request(&message).await?;
        self.pending_response = Some(response);
        Ok(())
    }
    async fn receive(&mut self) -> Result<TransportMessage> {
        self.pending_response
            .take()
            .ok_or_else(|| Error::internal("receive() called before send() on HTTP transport"))
    }
    async fn close(&mut self) -> Result<()> { Ok(()) }
}
```
**Note:** `do_request` (wasm_http.rs:72-82) already returns a parsed `TransportMessage`, and `do_http_request` already injects `extra_headers` (wasm_http.rs:117-121) — so bearer injection via `WasmHttpConfig.extra_headers` works for free once correlation is fixed. The `WasmHttpClient` wrapper (wasm_http.rs:226-265, used by today's raw example) can stay for backward compat.

### Pattern 2: Pure wasm-safe PKCE helper (D-02)
**What:** Lift the three primitives from `oauth.rs` (generate_code_verifier:593-596, generate_code_challenge:599-604, plus state via the same verifier gen) into a target-agnostic module using `getrandom::fill` instead of `rand`.
**When to use:** The helper backs both the native loopback flow and the browser example.
**Example:**
```rust
// Source: src/client/oauth.rs:592-604 (logic) + docs.rs/getrandom/0.4 (fill)
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

/// RFC 7636 §4.1 code verifier: 43-char base64url of 32 random bytes.
pub fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("system RNG (getrandom) must be available");
    URL_SAFE_NO_PAD.encode(bytes)
}

/// RFC 7636 §4.2 S256 code challenge.
pub fn code_challenge_s256(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// CSRF state token (same entropy source as the verifier).
pub fn generate_state() -> String { generate_code_verifier() }
```
(Prefer returning `Result` over `.expect()` if the planner wants the no-panic CLAUDE.md `check-unwraps` gate to stay clean — `getrandom::fill` returns `Result<(), Error>`. See Pitfall 7.)

### Pattern 3: Time-delayed Working→Completed task over the HTTP create-path (D-05 — novel, no prior example)
**What:** Make a `TaskSupport::Required` tool that does NOT complete synchronously, then transition it after a delay. The create-path completes synchronously ONLY if the tool returns a nested `result` (task_dispatch.rs:260-275). To stay Working then complete later, the tool returns `status: "working"` (no `result`), and a background task updates the store.
**When to use:** The single demo long-task tool.
**Constraint discovered:** The tool handler runs BEFORE the store mints the canonical id (id is minted in `build_task_created_response`, task_dispatch.rs:254-258, AFTER the handler returns). So the background updater cannot be handed the id; it must discover it via `store.list(owner, None)` and pick the most-recent `Working` task for that owner.
**Example:**
```rust
// Source: tools modeled on tests/tool_as_task_lifecycle_http.rs:82-100 (pending shape),
//   store API src/server/task_store.rs:242-330 (create/update_status/set_result/list),
//   owner resolution src/server/task_dispatch.rs:168-186 (subject-first, "local" fallback)
let store_for_bg = Arc::clone(&store); // store: Arc<dyn TaskStore>
let long_task = TypedTool::new_with_schema(
    "slow_summarize",
    serde_json::json!({ "type": "object" }),
    move |_args, extra| {
        let store = Arc::clone(&store_for_bg);
        // owner the create-path will scope to (subject, or "local")
        let owner = extra.auth_context.as_ref()
            .map(|c| c.subject.clone()).unwrap_or_else(|| "local".into());
        Box::pin(async move {
            // Spawn a background updater. It waits for the store to mint the task
            // (create happens just AFTER this handler returns), then transitions it.
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await; // let create land
                // find the freshly-minted Working task for this owner
                if let Ok((tasks, _)) = store.list(&owner, None).await {
                    if let Some(t) = tasks.into_iter()
                        .find(|t| matches!(t.status, pmcp::types::tasks::TaskStatus::Working)) {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await; // simulate work
                        let result = pmcp::types::CallToolResult::new(
                            vec![pmcp::types::Content::text("done after 3s")]);
                        let _ = store.set_result(&t.task_id, &owner, result).await;
                        let _ = store.update_status(&t.task_id, &owner,
                            pmcp::types::tasks::TaskStatus::Completed, None).await;
                    }
                }
            });
            // Return WORKING (no nested result) so the create-path mints a Working task.
            Ok(serde_json::json!({
                "taskId": "tool-fabricated", "status": "working", "ttl": 60000,
                "createdAt": "2026-06-30T00:00:00Z", "lastUpdatedAt": "2026-06-30T00:00:00Z"
            }))
        })
    },
)
.with_description("Summarize as a multi-second MCP Task (Working→Completed)")
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));
```
**Planner caveats:** (a) The `list`-and-pick-most-recent-Working heuristic is fine for a single-user demo but races under concurrency — acceptable for a reference example, must be noted. (b) Confirm during planning whether `extra.auth_context` is populated on the HTTP path (the create-path scopes to the SAME owner via `create_path_auth`, mod.rs:1361-1365). (c) An alternative the planner MAY prefer: a custom `TaskStore` wrapper whose `create()` schedules the background completion — more deterministic (knows the id at mint time) but higher complexity. Evaluate both; the closure+`list` approach keeps the demo readable.

### Anti-Patterns to Avoid
- **Rewriting the transport layer.** D-08 + ROADMAP say reuse — fix `send`/`receive` in the existing `WasmHttpTransport`; do not author a parallel transport.
- **Modifying `examples/wasm-client`.** LOCKED scope fence — it stays the minimal demo.
- **Porting `oauth.rs` / `auth.rs` to wasm wholesale.** They are `reqwest`+`tokio::net::TcpListener` bound (oauth.rs:640) and `#[cfg(not(target_arch = "wasm32"))]` (client/mod.rs:35-40). Extract ONLY the pure crypto (D-02/D-03).
- **Using SubtleCrypto for hashing.** Async, web-sys-feature-heavy, can't share with native. Use `sha2`.
- **Hiding the poll loop in a helper.** D-09 wants the explicit 500ms `setTimeout` loop visible in the demo.
- **Trusting the tool-fabricated `taskId`.** The store mints the canonical id; the wire id must be the store id (proven by `assert_ne!(task_id, "tool-fabricated")` in the live test, line 208).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SHA-256 for S256 challenge | Custom hashing / SubtleCrypto plumbing | `sha2::Sha256` (in-tree) | Sync, pure, audited, already used by native PKCE |
| base64url encoding | Manual base64 + URL-safe substitution | `base64::URL_SAFE_NO_PAD` (in-tree) | RFC 7636-correct, already used by native PKCE |
| Random bytes on wasm | `Math.random` / custom JS RNG | `getrandom::fill` (`wasm_js`) | CSPRNG via Web Crypto; already backs `uuid` js feature |
| JSON-RPC request↔response correlation | Manual id-matching in the example | High-level `Client` + fixed `WasmHttpTransport` | `Client::send_request` already does correlation, middleware, task helpers |
| `tasks/*` wire serialization | Hand-written task JSON in the browser | `Client::call_tool_with_task`/`tasks_get`/`tasks_result`/`tasks_cancel` (client/mod.rs:508-620) | Typed, mirrors the frozen contract, no duck-typing |
| OAuth2 IdP (authorize/token/codes/tokens) | Custom token store | `InMemoryOAuthProvider` (server/auth/oauth2.rs:377) | Full self-contained IdP; authorize + exchange_code + validate_token |
| Bearer validation on the server | Custom header parsing | `StreamableHttpServer` auth_provider + `MockValidator`/oauth2 `TokenValidator` | Already validates `Authorization` (streamable:758-770) |
| Task state machine + TTL | Custom status transitions | `InMemoryTaskStore` (task_store.rs:483+) | Validates transitions, persists results, owner-scoped |

**Key insight:** This phase has near-zero "build new infrastructure." The risk is in **wiring two existing-but-incomplete seams** (the broken Fetch transport correlation, and the missing time-delayed task pattern) and the **example-level OAuth orchestration** that is deliberately NOT in the SDK.

## Runtime State Inventory

Not applicable — this is a greenfield example + additive SDK code phase. No rename/refactor/migration. (No stored data, live-service config, OS-registered state, secrets, or build artifacts are being renamed or migrated.)

## Common Pitfalls

### Pitfall 1: The existing `WasmHttpTransport::Transport` impl silently discards responses
**What goes wrong:** A planner who treats D-08 as "add a transport" may not notice `WasmHttpTransport` already exists and already (badly) implements `Transport`. Its `send()` POSTs and drops the response; `receive()` errors (wasm_http.rs:190-217). Using it with the high-level `Client` fails immediately on the first `initialize`.
**Why it happens:** The struct was written as a Fetch helper, and the `Transport` impl was a non-functional stub (the working raw path is `WasmHttpClient::request`, wasm_http.rs:240-254).
**How to avoid:** Implement the one-slot `pending_response` buffer (Pattern 1). Add a unit test asserting `send` then `receive` returns the POSTed response.
**Warning signs:** `Client.initialize()` returns `"HTTP transport requires send() before receive()"`.

### Pitfall 2: getrandom 0.4 on `wasm32-unknown-unknown` needs the backend cfg, not just the feature
**What goes wrong:** Even with `getrandom = { features = ["wasm_js"] }`, on `wasm32-unknown-unknown` getrandom 0.4 historically requires `--cfg getrandom_backend="wasm_js"` via `.cargo/config.toml` `[target.wasm32-unknown-unknown] rustflags = ['--cfg','getrandom_backend="wasm_js"']`. There is currently **NO `.cargo/config.toml`** in the repo setting this (verified: only the root `.cargo/config.toml` exists with unrelated content). `[CITED: docs.rs/getrandom; users.rust-lang.org getrandom-0-3-2-wasm-js-feature-issue]`
**Why it happens:** getrandom moved RNG backend selection from a Cargo feature to a cfg flag across the 0.2→0.3→0.4 line; the rule shifted and the message "no longer required and does nothing" applies only to certain versions.
**How to avoid:** During Wave 0, `make wasm-build` the example. If it fails with a getrandom "unsupported target" error, add `examples/web-channel-client/.cargo/config.toml` with the backend cfg. Verify against the EXACT getrandom version `cargo tree -p getrandom` resolves.
**Warning signs:** Link/compile error mentioning `getrandom` "no backend" / "unsupported target" on the wasm build.

### Pitfall 3: A "working" task with no updater stays pending forever
**What goes wrong:** Copying the `stay_pending` tool (lifecycle_http test:82-100) gives a task that NEVER completes — the browser polls `tasks/get` indefinitely and `tasks/result` always returns `-32002`.
**Why it happens:** The create-path only auto-completes when the tool returns a nested `result` (task_dispatch.rs:260-275); a bare `working` status creates a Working task and nothing transitions it.
**How to avoid:** Implement the background updater (Pattern 3) that calls `set_result` + `update_status(Completed)` after a delay. Add a server-side integration test (mirroring lifecycle_http.rs) asserting the task reaches `Completed` after the delay.
**Warning signs:** Poll loop never terminates; `tasks/result` perpetually `-32002`.

### Pitfall 4: `StreamableHttpServer` does NOT serve the OAuth2 IdP HTTP endpoints
**What goes wrong:** The planner assumes registering `InMemoryOAuthProvider` makes `/oauth2/authorize` and `/oauth2/token` reachable over HTTP. It does NOT — `StreamableHttpServer` only validates the `Authorization` header on MCP requests (streamable:758-770). `s29_oauth_server.rs` runs over **stdio** and merely *logs* the OAuth URLs (s29:182-183, 217-225); it never serves them.
**Why it happens:** The IdP is a provider object (authorize/exchange_code/validate_token methods), not an HTTP server.
**How to avoid:** In `demo_server.rs`, hand-wire an axum `Router` with `GET /oauth2/authorize` → `provider.authorize(...)` (redirect with `?code=&state=`) and `POST /oauth2/token` → `provider.exchange_code(...)`, and run it (either as additional routes merged with the MCP router, or a second listener on the same origin). The MCP `StreamableHttpServer`'s `auth_provider` must validate the bearer the IdP issued (share the provider as both IdP and `TokenValidator`, or pair `InMemoryOAuthProvider.validate_token` with the server's auth path). Confirm exact route-merge mechanics during planning (see Open Question 1).
**Warning signs:** Browser redirect to `/oauth2/authorize` 404s; token exchange POST 404s.

### Pitfall 5: `make quality-gate` does NOT build wasm — non-wasm regression can pass while wasm breaks (or vice versa)
**What goes wrong:** `quality-gate` runs fmt/lint/build/test/audit (Makefile:660-679) but the default `build` targets the host. WASM is a SEPARATE target (`make wasm-build`, Makefile:58-61: `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm`). SC-4 requires BOTH: wasm builds AND non-wasm not regressed.
**Why it happens:** wasm is not in the default gate.
**How to avoid:** The phase's verification must run BOTH `make quality-gate` AND `make wasm-build` (and `make wasm-release`). Add cfg-gating discipline so the new `pmcp` PKCE module compiles on both targets and `WasmHttpTransport` changes don't break the host build. Sample BOTH per Validation Architecture.
**Warning signs:** Green `make quality-gate` but red `make wasm-build`, or vice versa.

### Pitfall 6: The `wasm` feature does not pull `http`/`streamable-http` — but `WasmHttpTransport` is cfg-gated, not feature-gated
**What goes wrong:** Confusion about which feature exposes `WasmHttpTransport`. It is gated purely by `#![cfg(target_arch = "wasm32")]` (wasm_http.rs:6) and exported under the same cfg (lib.rs:138-139) — it does NOT require the `http` feature. The `wasm` feature (Cargo.toml:173) = `["websocket-wasm", "uuid/js", "futures-channel", "futures-locks"]`. The example builds `pmcp` with `default-features = false, features = ["wasm"]` (mirror examples/wasm-client/Cargo.toml).
**How to avoid:** Keep the PKCE helper compiling under the `wasm` feature set (it only needs `sha2`/`base64`/`getrandom`, all non-feature-gated or wasm-target deps). Do NOT add `http`/`streamable-http` to the example's wasm `pmcp` dep (those pull hyper/axum/tokio and won't build on wasm). The **demo server** binary is a SEPARATE native build that DOES use `streamable-http` + `full`.
**Warning signs:** wasm build pulls hyper/tokio and fails; or PKCE module disappears because it was wrongly gated behind a native-only feature.

### Pitfall 7: `getrandom::fill` returns `Result` — `.expect()`/`.unwrap()` trips the CLAUDE.md check-unwraps gate
**What goes wrong:** `make quality-gate` runs `check-unwraps` (Makefile:673). A `.unwrap()`/`.expect()` on `getrandom::fill` in `src/` may fail the gate.
**How to avoid:** Return `Result` from the helper (e.g. `generate_code_verifier() -> Result<String>`) or document an allow with a `// Why:` annotation. Confirm `check-unwraps` scope (it may only scan certain paths) during Wave 0.
**Warning signs:** `check-unwraps` failure on the new pkce module.

## Code Examples

### Bundled demo server skeleton (D-04/D-05) — adapt s46
```rust
// Source: examples/s46_http_tool_as_task.rs:63-119 (server stand-up) +
//   src/server/auth/oauth2.rs:377,403-451 (InMemoryOAuthProvider) +
//   src/server/streamable_http_server.rs:758-770 (bearer validation)
use std::sync::Arc;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::{Server};
use tokio::sync::Mutex;

let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
let server = Server::builder()
    .name("web-channel-demo")
    .version("1.0.0")
    .tool("slow_summarize", /* long_task from Pattern 3 */)
    .task_store(Arc::clone(&store)) // auto-advertises `tasks`
    // .auth_provider(/* MockValidator or oauth2-backed TokenValidator */)  // validates bearer
    .build()?;
let server = Arc::new(Mutex::new(server));
let bind: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
let (bound, handle) = StreamableHttpServer::new(bind, server).start().await?;
// SEPARATELY: axum Router for /oauth2/authorize + /oauth2/token over InMemoryOAuthProvider (Pitfall 4)
```

### Browser PKCE flow shape (D-01/D-06/D-07) — example-level
```rust
// Source: src/client/oauth.rs:660-672 (auth URL params, S256) + web-sys Storage/Location
// 1) On "Login": gen verifier+challenge+state, store verifier+state in sessionStorage,
//    build authorize_url with code_challenge + code_challenge_method=S256 + state, then:
//      window.location().set_href(&authorize_url)        // full-page redirect (D-07)
// 2) On page load: read window.location().search(); if ?code & ?state present:
//      - verify state == sessionStorage("state")          // CSRF (state-mismatch UX = discretion)
//      - Fetch POST /oauth2/token { code, code_verifier=sessionStorage("verifier"), ... }
//      - store access_token in sessionStorage
// 3) Build Client<WasmHttpTransport> with extra_headers
//      = vec![("Authorization".into(), format!("Bearer {token}"))]  // wasm_http.rs:117-121 injects it
//    then client.initialize(...).await; drive tasks (Code Examples below).
```

### Driving the task lifecycle over Fetch (D-08/D-09) — high-level Client
```rust
// Source: src/client/mod.rs:508-620 (task helpers) — work over ANY Transport once Fetch is fixed
let resp = client.call_tool_with_task("slow_summarize".into(), serde_json::json!({})).await?;
let task_id = match resp { ToolCallResponse::Task(t) => t.task_id, _ => /* sync result */ };
// JS main.js: setTimeout 500ms loop calling a wasm method that does:
let task = client.tasks_get(&task_id).await?;     // poll
if task.status.is_terminal() { let r = client.tasks_result(&task_id).await?; /* show */ }
// Cancel button -> client.tasks_cancel(&task_id).await?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `examples/wasm-client` raw JSON-RPC via `WasmHttpClient::request` + manual `to_jsonrpc` (wasm-client/lib.rs:18-23,144-154) | High-level `Client<WasmHttpTransport>` with typed task helpers | This phase (D-08) | Browser gets the same typed API as native; no hand-built JSON-RPC |
| Tasks only reachable via in-process duplex shim (Phase 101) | `tasks/*` over real HTTP via `task_dispatch` (mod.rs:1165-1177) | Phase 102 (shipped, verified 7/7) | Browser can drive the full lifecycle over Fetch — no SSE |
| getrandom RNG backend selected by Cargo feature | RNG backend selected by `--cfg getrandom_backend` | getrandom 0.3→0.4 | wasm builds may need `.cargo/config.toml` cfg (Pitfall 2) |

**Deprecated/outdated:**
- The `examples/wasm-client/claude-instance/INVESTIGATION_REPORT.md` references **getrandom 0.2.16 with the "js" feature** — that is STALE; the repo is on getrandom 0.4 with `wasm_js` (Cargo.toml:126). Do not follow the 0.2 guidance.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `gloo-timers` 0.4 is legit/safe IF used | Standard Stack | Low — recommended AGAINST in favor of JS `setTimeout` (no install) |
| A2 | getrandom 0.4 on wasm32-unknown-unknown still needs `--cfg getrandom_backend="wasm_js"` in this repo's setup | Pitfall 2 | Medium — if unneeded, the `.cargo/config.toml` is harmless; if needed and omitted, wasm build fails. **Verify via `make wasm-build` in Wave 0.** |
| A3 | `extra.auth_context` is populated on the HTTP create-path so the background updater can resolve the owner | Pattern 3 | Medium — if empty, owner defaults to `"local"` and the demo still works single-user; cross-owner would need the header. Verify during planning. |
| A4 | Hand-wiring axum `/oauth2/authorize` + `/oauth2/token` over `InMemoryOAuthProvider` is the intended D-04 mechanism (no turnkey HTTP IdP exists) | Pitfall 4 | Medium — confirmed no existing HTTP-served IdP example; exact route-merge with the MCP router is Open Question 1 |
| A5 | The `list`-and-pick-most-recent-Working heuristic for the background updater is acceptable for a single-user demo | Pattern 3 | Low — fine for a reference example; planner may choose the custom-`TaskStore`-wrapper alternative for determinism |

## Open Questions

1. **How to merge the OAuth2 IdP routes with the MCP server over one origin?**
   - What we know: `StreamableHttpServer::start()` builds its own axum router (streamable:286) and binds a listener; `InMemoryOAuthProvider` exposes `authorize`/`exchange_code`/`metadata` but no HTTP routes.
   - What's unclear: whether to (a) run a SECOND axum listener for `/oauth2/*` on the same host:port via a merged `Router`, (b) use the lower-level `pmcp::axum::router()` / `build_mcp_router` (streamable:286) to compose MCP + OAuth routes into one `Router` the example serves itself, or (c) run two ports (CORS implications for the browser).
   - Recommendation: Prefer composing one axum `Router` via the lower-level builder so the browser hits a single origin (avoids CORS). Spike this in Wave 0; fall back to two listeners if composition is awkward.

2. **Does the create-path's owner (`create_path_auth`, mod.rs:1361-1365) match what `store.list(owner)` will see for the background updater?**
   - What we know: create-path scopes the minted task to the same owner the tool ran as; `resolve_owner` is subject-first with `"local"` fallback (task_dispatch.rs:168-186).
   - What's unclear: exact `subject` value when authenticated via the bundled IdP's bearer (the `MockValidator`/oauth2 `AuthContext.subject`).
   - Recommendation: log the resolved owner in the demo server during Wave 0; assert the updater's `list` finds the task.

3. **Should the D-02 helper return `Result` (no-panic) or panic on RNG failure?**
   - What we know: `make quality-gate` runs `check-unwraps` (Makefile:673); `getrandom::fill` returns `Result`.
   - Recommendation: return `Result` from helper fns; map to `pmcp::Error`. Confirm `check-unwraps` scope.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `wasm-pack` | build.sh (example) | ✓ | 0.13.1 | — |
| `wasm32-unknown-unknown` target | wasm build | ✓ | installed | — |
| `cargo` / Rust stable | all | ✓ | (host) | — |
| `make wasm-build` target | SC-4 wasm verification | ✓ | Makefile:58-61 | — |
| A static file server | serving index.html locally | ✓ (any) | — | `python3 -m http.server` |
| `slopcheck` | package legitimacy audit | ✗ | — | All packages already in-tree (stronger evidence) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `slopcheck` (mitigated — all deps already vetted in-tree).

## Validation Architecture

> `workflow.nyquist_validation: true` in `.planning/config.json` → this section applies.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` 1.7 + `quickcheck` 1.0 (Cargo.toml:131-133); `wasm-bindgen-test` 0.3 for wasm unit tests (examples/wasm-client/Cargo.toml) |
| Config file | none — cargo test; `Makefile` `test-all`, `validate-always` targets |
| Quick run command | `cargo test --features full <module/test>` (PKCE helper + server integration) |
| Full suite command | `make quality-gate` AND `make wasm-build` (both required for SC-4) |

### Phase Requirements → Test Map
| Req | Behavior | Test Type | Automated Command | File Exists? |
|-----|----------|-----------|-------------------|-------------|
| WEBCH-01 | verifier is 43-char base64url; charset valid (RFC 7636) | property | `cargo test pkce_verifier_charset_len` | ❌ Wave 0 |
| WEBCH-01 | S256 challenge deterministic for same verifier | property/unit | `cargo test pkce_challenge_deterministic` | ❌ Wave 0 |
| WEBCH-01 | base64url roundtrip (encode→decode) of random bytes | fuzz/property | `cargo test pkce_base64url_roundtrip` (proptest) | ❌ Wave 0 |
| WEBCH-01 | challenge == known RFC 7636 §appendix-B test vector | unit | `cargo test pkce_rfc7636_vector` | ❌ Wave 0 |
| WEBCH-02 | `WasmHttpTransport` send→receive returns POSTed response | unit (wasm-bindgen-test) | `wasm-pack test --headless` | ❌ Wave 0 |
| WEBCH-05 | long-task reaches `Completed` after delay over HTTP | integration | `cargo test --features full long_task_completes_over_http` | ❌ Wave 0 (model on tests/tool_as_task_lifecycle_http.rs) |
| WEBCH-05 | `tasks/result` returns `-32002` before completion, content after | integration | (same test) | ❌ Wave 0 |
| WEBCH-06 | `tasks/cancel` transitions task to `Cancelled` | integration | `cargo test --features full task_cancel_over_http` | ❌ Wave 0 |
| WEBCH-04 | bearer-less MCP request is rejected (401/auth error) | integration | `cargo test --features full demo_server_requires_bearer` | ❌ Wave 0 |
| WEBCH-07 | example wasm crate compiles | build | `make wasm-build` (+ example) | ✓ target exists |
| WEBCH-08 | non-wasm not regressed | build/test | `make quality-gate` | ✓ |

### Sampling Rate
- **Per task commit:** `cargo test --features full <touched module>` + `cargo build` (host).
- **Per wave merge:** `make quality-gate` AND `make wasm-build`.
- **Phase gate:** `make quality-gate` green + `make wasm-build`/`make wasm-release` green + the browser example runs end-to-end (manual SC-3 check) before `/gsd:verify-work`.

### Wave 0 Gaps
- [ ] `tests/pkce_helper.rs` (or in-module `#[cfg(test)]`) — verifier/challenge/state property + RFC vector tests (WEBCH-01)
- [ ] `fuzz/` target OR proptest for base64url roundtrip + verifier invariants (WEBCH-01; pure crypto = ideal fuzz target per ALWAYS)
- [ ] wasm unit test for `WasmHttpTransport` send/receive correlation (WEBCH-02) — `wasm-bindgen-test`
- [ ] `tests/web_channel_long_task_http.rs` — multi-second Working→Completed + cancel + bearer-required (WEBCH-04/05/06), modeled on `tests/tool_as_task_lifecycle_http.rs`
- [ ] Verify `make wasm-build` succeeds for the example (resolves Pitfall 2 / A2)
- [ ] The working browser example IS the EXAMPLE deliverable (ALWAYS `cargo run --example` equivalent = build.sh + serve + manual run)

## Security Domain

> `security_enforcement` not explicitly `false` in config → included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | OAuth 2.0 Authorization Code + PKCE (S256) — `InMemoryOAuthProvider` IdP; bearer validated server-side (streamable:758-770) |
| V3 Session Management | yes | sessionStorage for token (origin-scoped, cleared on tab close, D-06); `mcp-session-id` handled by transport (wasm_http.rs:103-107) |
| V4 Access Control | yes | Task owner-scoping / IDOR protection — owner derived from `AuthContext.subject`, NOT client params (task_dispatch.rs:168-186); cross-owner isolation proven in lifecycle_http test:273-325 |
| V5 Input Validation | yes (light) | Typed deserialization of `tasks/*` results via `Client` helpers; OAuth `state` CSRF check in the example |
| V6 Cryptography | yes | S256 via `sha2` (never hand-roll); CSPRNG via `getrandom` (never `Math.random`) |

### Known Threat Patterns for browser OAuth + WASM MCP

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| CSRF on the authorization redirect | Tampering/Spoofing | `state` param generated + stored in sessionStorage, verified on return (D-07; oauth.rs:672 uses random state) |
| Authorization code interception | Information Disclosure | PKCE S256 binds the code to the verifier (RFC 7636) — the whole point of D-02 |
| Token theft via XSS | Information Disclosure | sessionStorage (origin-scoped, cleared on tab close) — demo default; IndexedDB/httpOnly noted as production upgrade (deferred) |
| IDOR on `tasks/*` (read/cancel another owner's task) | Elevation of Privilege | Owner-scoped store ops; isolation enforced server-side and proven over HTTP (lifecycle_http test:289-325) |
| Replayed JSON-RPC id (cross-cache, per MEMORY pmcp.run bug) | Spoofing | Each `Client` mints unique ids (`Uuid::new_v4`, client/mod.rs:522); one-shot transport doesn't share a cache |
| RNG weakness | Tampering | `getrandom` (Web Crypto) for verifier + state, not JS `Math.random` |

## Project Constraints (from CLAUDE.md)

- **ALWAYS Requirements (MANDATORY for new features):** fuzz + property + unit + working `cargo run --example`. The PKCE helper is the natural fuzz/property target (pure crypto); the browser example IS the example deliverable; the long-task server integration test is the unit/integration coverage. Plan MUST include all four.
- **Quality gate before any commit/PR:** `make quality-gate` (fmt-check, lint with pedantic+nursery, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always, purity-check). **Add `make wasm-build` to the phase gate** (not in quality-gate by default — Pitfall 5).
- **Cognitive complexity ≤25 per function** (PMAT CI gate, pinned 3.15.0). The transport fix + PKCE helper are small; keep the demo server's tool closure and background updater factored.
- **Zero SATD / no `unwrap`/`expect` in `src/`** (check-unwraps) — return `Result` from PKCE helper (Pitfall 7).
- **Release & Publish Workflow (D-02 + D-08 add public API):** new `pmcp` minor bump 2.10.0 → **2.11.0** (additive: PKCE helper + functional `WasmHttpTransport`). Downstream pins to update if bumped: `crates/mcp-tester/Cargo.toml` (currently pins `pmcp = "2.8.1"`, version 0.7.0) and `cargo-pmcp/Cargo.toml` — bump their `pmcp` dep + their own versions per the workflow. `rustup update stable` before the release gate.
- **Builder pattern / `async_trait` / `serde camelCase` / examples numbered** conventions apply. Example dir is named (`web-channel-client`), not numbered, consistent with `wasm-client`.
- **justfile preference (user global):** repo uses `make`; this phase follows the existing `make` targets (no justfile present).

## Release Impact

- **Version bump:** `pmcp` 2.10.0 → **2.11.0** (minor — additive public API: new PKCE helper module + repaired/functional `WasmHttpTransport: Transport`). No breaking changes intended.
- **Public API surface added:** `pmcp::...::pkce::{generate_code_verifier, code_challenge_s256, generate_state}` (exact path = planner's call; `src/shared/pkce.rs` re-exported, available on both targets) and the now-working `WasmHttpTransport` (already exported at lib.rs:139 — behavior change, not new symbol).
- **Downstream pins:** `mcp-tester` (pins `pmcp = "2.8.1"`) and `cargo-pmcp` only need bumping IF they must consume the new API; for a pure additive release they can stay, but per the workflow rule "downstream crates that pin a bumped dependency must also be bumped" applies only if their pin is updated. Decide at release time.
- **CI/publish:** tag `vX.Y.Z` triggers `.github/workflows/release.yml` (publishes in dep order). The example crate is NOT published (it's an example, per the LOCKED "no new crate" fence).

## Sources

### Primary (HIGH confidence — in-repo source, exact lines)
- `src/shared/transport.rs:64-78,195-250` — `TransportMessage` enum + `Transport` trait (wasm vs native cfg split)
- `src/shared/wasm_http.rs:6,44-265` — existing `WasmHttpTransport` (broken send/receive:190-217), `WasmHttpClient`, `extra_headers` injection:117-121
- `src/shared/wasm_websocket.rs:30-162` — `WasmWebSocketTransport` Transport impl (channel-based receive) — the model to mirror
- `src/client/mod.rs:35-40` (wasm-gating), `:508-620` (task helpers), `:1948-2048` (send_request correlation loop)
- `src/client/oauth.rs:19-22,592-604,660-672` — PKCE primitives (rand/sha2/base64), auth URL + S256 + state
- `src/server/task_store.rs:242-371,483+` — TaskStore trait (create/get/update_status/list/cancel/set_result/get_result), InMemoryTaskStore
- `src/server/task_dispatch.rs:168-186,225-288,311-336,346-396` — owner resolution, create-path sync-completion, tasks/* routing
- `src/server/mod.rs:1150-1182,1300-1500` — HTTP tasks/* interception, create-path owner scoping, handle_call_tool
- `src/server/auth/mock.rs:57-269` — MockValidator (TokenValidator)
- `src/server/auth/oauth2.rs:172-373,377-451,536,716-723` — InMemoryOAuthProvider, OidcDiscoveryMetadata, exchange_code, endpoints
- `src/server/streamable_http_server.rs:286,758-770,1044-1048` — router build, bearer validation (no IdP HTTP routes)
- `examples/s46_http_tool_as_task.rs:49-201` — server stand-up + lifecycle scaffold
- `tests/tool_as_task_lifecycle_http.rs:33-326` — frozen tasks/* wire shapes, cross-owner isolation, reliability practices
- `examples/s29_oauth_server.rs:1-90,125-225` — IdP over stdio (proves no HTTP IdP routes)
- `examples/wasm-client/{src/lib.rs,build.sh,index.html,main.js,Cargo.toml}` — build/demo harness to mirror
- `Cargo.toml:81-82,88,120-126,151-176` — deps (sha2/base64/getrandom/rand), features (wasm)
- `Makefile:57-67,660-679` — wasm-build target, quality-gate composition

### Secondary (MEDIUM-HIGH — official external docs)
- getrandom 0.4 `fill` signature + `wasm_js`/`getrandom_backend` cfg — [docs.rs/getrandom/0.4.1](https://docs.rs/getrandom/0.4.1/getrandom/), [GitHub rust-random/getrandom](https://github.com/rust-random/getrandom)
- getrandom wasm backend cfg gotcha — [users.rust-lang.org getrandom-0-3-2-wasm-js-feature-issue](https://users.rust-lang.org/t/getrandom-0-3-2-wasm-js-feature-issue/127584)
- web-sys SubtleCrypto is async (Promise/JsFuture) — [docs.rs/web-sys SubtleCrypto](https://docs.rs/web-sys/0.3.52/web_sys/struct.SubtleCrypto.html), [huijzer.xyz/posts/wasm-crypto](https://huijzer.xyz/posts/wasm-crypto/)
- Crate versions — `cargo search` (this session): pmcp 2.10.0, getrandom 0.4.3, sha2 0.11.0, base64 0.22.1, gloo-timers 0.4.0

### Tertiary (LOW — flagged for validation)
- getrandom `--cfg getrandom_backend` necessity for THIS repo's exact build (A2) — verify via `make wasm-build` in Wave 0

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crypto deps confirmed in-tree with the exact lines they're already used; external versions verified via cargo search.
- Architecture / transport fix: HIGH — the correlation model and the broken existing impl are read directly from source with line numbers.
- D-05 multi-second task pattern: MEDIUM-HIGH — the create-path constraint is verified from source; the specific updater heuristic is a derived design (no prior example exists), flagged in Assumptions/Open Questions.
- OAuth IdP HTTP wiring (D-04): MEDIUM — confirmed no turnkey HTTP-served IdP exists; exact route-merge mechanism is Open Question 1.
- Pitfalls: HIGH — each tied to a specific source line or official doc.

**Research date:** 2026-06-30
**Valid until:** 2026-07-30 (stable in-repo APIs; getrandom/wasm tooling can shift faster — re-verify the backend cfg if wasm build fails)
