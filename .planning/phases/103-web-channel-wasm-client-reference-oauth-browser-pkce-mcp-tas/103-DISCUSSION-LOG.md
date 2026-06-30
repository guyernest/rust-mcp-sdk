# Phase 103: Web-channel WASM client reference (OAuth browser-PKCE + MCP Tasks) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-30
**Phase:** 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
**Areas discussed:** OAuth port strategy, Demo provider & server packaging, Token storage & redirect, Tasks transport & polling

---

## OAuth port strategy — auth code home

| Option | Description | Selected |
|--------|-------------|----------|
| New module in example | Browser-PKCE flow entirely inside the example (web-sys redirect + Fetch token exchange); SDK untouched | ✓ |
| New wasm module in SDK | `#[cfg(wasm32)]` browser-PKCE module under `src/client/` | |
| cfg-port existing modules | Make `client::auth`/`oauth` compile on wasm32 (abstract reqwest→Fetch, drop loopback) | |

**User's choice:** New module in example
**Notes:** Keeps SDK untouched, matches the "example only" scope fence, fastest path; pmcp.run lifts the browser orchestration directly.

## OAuth port strategy — PKCE crypto primitives

| Option | Description | Selected |
|--------|-------------|----------|
| Implement in example | Verifier/challenge via wasm crates (getrandom js + sha2 + base64url), self-contained | |
| Web Crypto via web-sys | Use browser SubtleCrypto through web-sys | |
| Reuse SDK PKCE helper | Extract pure verifier/challenge logic into a wasm-safe SDK helper shared native+browser | (basis of free-text) |

**User's choice:** Other (free text) — "reuse the SDK PKCE helper and release a new version of the PMCP SDK, if it is extending its functionality, without adding too much complexity. The MCP protocol is gaining popularity and if we can add a unique capability of calling MCP from web application, as the pmcp.run dev team is trying to do, it will be a good win for the SDK, and better DX."
**Notes:** Strategic steer — promote the reusable bits into `pmcp` and cut a new release, provided complexity stays low. Reconciled with the LOCKED fence: extending the existing `pmcp` crate (not a new `crates/` library) is permitted.

## OAuth port strategy — SDK scope (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Crypto helper only | SDK gains only the wasm-safe PKCE primitives; browser orchestration stays in example | ✓ |
| Helper + browser auth client | SDK also gains a thin `#[cfg(wasm32)]` browser-PKCE orchestrator | |

**User's choice:** Crypto helper only
**Notes:** Lowest complexity/risk to the native flow; honors the complexity guardrail.

---

## Demo provider & server packaging — backing server

| Option | Description | Selected |
|--------|-------------|----------|
| Bundled self-contained server | Demo `StreamableHttpServer` using SDK `server/auth/mock.rs` + `oauth2`; runs offline/CI | ✓ |
| Bundled server + real-provider docs | Bundled default + docs to point at Google/GitHub/Auth0 | |
| External real provider only | Document a real provider, no bundled IdP | |

**User's choice:** Bundled self-contained server
**Notes:** Strongest fit for ALWAYS coverage + reproducibility; no external accounts/secrets/network.

## Demo provider & server packaging — task tool shape

| Option | Description | Selected |
|--------|-------------|----------|
| Simulated long task | One `TaskSupport::Required` tool transitioning Working→Completed over seconds, so browser polls several times | ✓ |
| Instant task (mirror s46) | Reuse s46's synchronous completed-task shape | |
| Both tools | Instant (contract parity) + simulated long (UX demo) | |

**User's choice:** Simulated long task
**Notes:** Makes polling + cancel visible; wire shapes must still mirror s46 / the HTTP lifecycle test (frozen contract).

---

## Token storage & redirect — browser storage

| Option | Description | Selected |
|--------|-------------|----------|
| sessionStorage | verifier/state/token in sessionStorage via web-sys; origin-scoped, cleared on close | ✓ |
| IndexedDB | Durable cross-tab persistence; async, more boilerplate | |
| sessionStorage + IndexedDB note | sessionStorage for demo + documented IndexedDB upgrade path | |

**User's choice:** sessionStorage
**Notes:** Simplest synchronous API, good demo security default. IndexedDB recorded as deferred production upgrade.

## Token storage & redirect — redirect mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Full-page redirect | `window.location` → return to redirect URI → JS reads `?code=&state=` | ✓ |
| Popup window | IdP in popup, postMessage code back to opener | |

**User's choice:** Full-page redirect
**Notes:** Most compatible, matches criterion 1's described flow.

---

## Tasks transport & polling — transport approach

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch Transport adapter | New wasm `Transport` impl wrapping Fetch so high-level `Client` task helpers work as-is | ✓ |
| Raw JSON-RPC | Hand-build `tasks/*` JSON-RPC through `WasmHttpClient` (existing wasm-client style) | |

**User's choice:** Fetch Transport adapter

## Tasks transport & polling — transport home (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| In pmcp SDK | `WasmHttpTransport: Transport` under `src/shared/`, symmetric with `WasmWebSocketTransport`; ships in the new release | ✓ |
| In the example | Transport adapter inside the example only | |

**User's choice:** In pmcp SDK
**Notes:** The biggest "call MCP from a web app" win — makes the whole high-level `Client` usable over Fetch. Bundled into the same new `pmcp` release as the PKCE helper.

## Tasks transport & polling — poll UX

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed interval + live status | ~500ms poll until terminal; visible status line + Cancel button | ✓ |
| Exponential backoff | 500ms→1s→2s capped | |
| Reuse Client poll helper | Auto-poll helper, render start/finish only | |

**User's choice:** Fixed interval + live status
**Notes:** Keeps the poll loop explicit (the demo teaches it) and leaves room for a Cancel button.

---

## Claude's Discretion

- Example crate layout + `build.sh`/`index.html`/`main.js`/`style.css` structure (mirror `examples/wasm-client`).
- Error / expired-token / state-mismatch UX in the demo UI.
- Simulated long-task tool name + argument shape.
- Crypto crate choice for the PKCE helper internals (getrandom-js+sha2+base64url vs web-sys SubtleCrypto) — pick lower-complexity/smaller-binary.
- ALWAYS-coverage test targets (unit/property/fuzz).

## Deferred Ideas

- IndexedDB token persistence (production durability upgrade).
- Full `#[cfg(wasm32)]` browser-PKCE orchestrator in `pmcp` (turnkey auth API) — future phase.
- Real external OAuth provider (Google/GitHub/Auth0) as a first-class runnable mode.
- Popup/postMessage redirect flow.
- Promoting the web-channel client to a published `crates/` library (LOCKED out by ROADMAP).
- Exponential backoff polling.
