---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
plan: 05
subsystem: example
tags: [wasm, oauth, pkce, mcp-tasks, browser, fetch, web-channel, example]

# Dependency graph
requires:
  - phase: 103-01
    provides: "wasm-safe PKCE helper (pmcp::shared::pkce::{generate_code_verifier, code_challenge_s256, generate_state}) used for the browser authorize/exchange"
  - phase: 103-02
    provides: "Fixed WasmHttpTransport (send buffers the Fetch response into PendingSlot, receive pops it) so the high-level Client + typed task helpers work over Fetch"
  - phase: 103-04
    provides: "Bundled offline demo server (own native crate) — /oauth2/authorize (S256), /oauth2/token (Form<TokenRequest>), the slow_summarize Working->Completed task, the registered web-channel-client / http://127.0.0.1:8080/callback demo client, and the MEDIUM-6 single-user limitation to document"
provides:
  - "Browser WASM MCP client as its OWN cdylib crate (examples/web-channel-client/client/) — full-page-redirect PKCE + high-level Client<WasmHttpTransport> task lifecycle over Fetch, with NO native HTTP deps (SC-4)"
  - "Demo harness (build.sh, index.html, main.js, style.css) with an explicit visible 500ms poll loop + Cancel button (D-09) and ?code=&state= redirect detection (D-07)"
  - "Shared examples/web-channel-client/README.md documenting pmcp.run web-app channel adaptation + the single-user/no-concurrency limitation (MEDIUM-6 / T-103-RACE)"
affects: [pmcp.run web-app channel, web-channel-client]

# Tech tracking
tech-stack:
  added: [web-sys (Storage/Location/Url/UrlSearchParams/Headers/Request/RequestInit/Response), wasm-pack harness]
  patterns:
    - "HIGH-2 package split: the wasm cdylib client takes pmcp default-features=false features=[wasm] only — NO hyper/tokio/axum leak into the .wasm (verified via cargo tree)"
    - "Browser full-page-redirect PKCE: begin_login returns the authorize URL (built HERE, not cfg-porting oauth.rs); complete_login validates CSRF state and form-POSTs the token exchange"
    - "Bearer-in-Fetch: Client::new(WasmHttpTransport::new(config-with-Authorization-Bearer)) so the high-level typed task helpers work unchanged in the browser"
    - "Explicit/visible setTimeout poll loop in main.js (not a hidden auto-poll helper) — teaches the Tasks lifecycle"

key-files:
  created:
    - examples/web-channel-client/client/src/lib.rs
    - examples/web-channel-client/client/src/utils.js
    - examples/web-channel-client/client/Cargo.toml
    - examples/web-channel-client/client/build.sh
    - examples/web-channel-client/client/index.html
    - examples/web-channel-client/client/main.js
    - examples/web-channel-client/client/style.css
    - examples/web-channel-client/client/.gitignore
    - examples/web-channel-client/README.md
  modified:
    - Cargo.toml

key-decisions:
  - "Exposed task helpers as small #[wasm_bindgen] methods (invoke_task/poll_task/task_result/cancel_task) returning status STRINGS so the JS poll loop can branch on terminal status without re-deserializing the Task; the explicit 500ms setTimeout loop lives in main.js per D-09"
  - "exchange_code is a free function (not a WasmClient method) keeping each #[wasm_bindgen] method small (cognitive complexity <= 25) and isolating the Fetch token-exchange detail"
  - "Token exchange is application/x-www-form-urlencoded (grant_type/code/code_verifier/redirect_uri/client_id) to match the demo server's Form<TokenRequest> (oauth2.rs:256); access_token parsed from the JSON AccessToken response (oauth2.rs:108)"
  - ".cargo/config.toml was NOT created — 103-SPIKE Open Question 4 verdict: getrandom 0.4 uses the wasm_js feature, no rustflag needed; the wasm build confirmed this (exits 0)"

patterns-established:
  - "Calling MCP from a web application as a turnkey pattern: pmcp pkce helper + WasmHttpTransport + high-level Client typed task helpers, with the browser-only OAuth orchestration kept in the example (D-01/D-03)"

requirements-completed: [WEBCH-03, WEBCH-06, WEBCH-07]

# Metrics
duration: ~25min
completed: 2026-06-30
---

# Phase 103 Plan 05: Browser WASM client crate + demo harness Summary

**A browser MCP client in its OWN wasm cdylib crate that runs a full-page-redirect OAuth PKCE flow via the `pmcp` PKCE helper, threads the bearer into the fixed `WasmHttpTransport`, and drives the complete MCP Tasks lifecycle (call → explicit 500ms poll → result + Cancel) through the high-level `pmcp::Client` over Fetch — building to wasm with ZERO native HTTP deps and documented for pmcp.run web-app-channel adaptation.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-06-30
- **Completed:** 2026-06-30
- **Tasks:** 2
- **Files modified:** 10 (9 created, 1 modified)

## Accomplishments
- Stood up the browser client as its OWN cdylib crate (`examples/web-channel-client/client/`), the HIGH-2 sibling of the native server crate. `cargo tree` confirms ZERO `hyper`/`tokio`/`axum` in the wasm dependency graph (SC-4).
- `WasmClient` performs the browser PKCE flow with the `pmcp` PKCE helper: `begin_login` builds verifier/S256-challenge/state, stores verifier+state in `sessionStorage`, and assembles the authorize URL HERE (D-01); `complete_login` validates the CSRF `state` (T-103-CSRF) then form-POSTs the token exchange matching the demo server's `Form<TokenRequest>` and stores the bearer.
- `connect` builds `Client::new(WasmHttpTransport::new(config))` with the bearer in `extra_headers`; `invoke_task`/`poll_task`/`task_result`/`cancel_task` expose the four typed task helpers to JS.
- `main.js` detects `?code=&state=` on redirect return (D-07) and runs an EXPLICIT, visible `setTimeout` 500ms poll loop on `tasks/get` until terminal then fetches `tasks/result`, with a Cancel button driving `tasks/cancel` (D-09).
- Shared root README documents the package split, the PKCE + bearer-in-Fetch + task-poll structure, run instructions, the real-provider/IndexedDB upgrade paths, and the MEDIUM-6 single-user/no-concurrency limitation.

## Task Commits

Each task was committed atomically:

1. **Task 1: Browser WASM client crate (PKCE + high-level task helpers)** - `273ab327` (feat)
2. **Task 2: Build harness + shared README** - `7a662f53` (feat)

_Task 1's TDD verify is the wasm `cargo build --lib` (exits 0); Task 2's verify is the full `build.sh` wasm-pack build (exits 0). Manual E2E (Login → run task → watch 500ms polls → Cancel) is the 103-VALIDATION Manual-Only criterion._

## Files Created/Modified
- `examples/web-channel-client/client/src/lib.rs` - `#[wasm_bindgen] WasmClient`: PKCE begin/complete login, sessionStorage helpers, `connect` over the high-level `Client<WasmHttpTransport>`, and the four task-lifecycle methods; free `exchange_code` for the form token POST.
- `examples/web-channel-client/client/src/utils.js` - structured-error glue (`newError`) for `to_js_error`.
- `examples/web-channel-client/client/Cargo.toml` - wasm cdylib manifest: `pmcp` `default-features=false features=["wasm"]`, web-sys Storage/Location/Url/UrlSearchParams/Headers/Request/RequestInit/Response; no native HTTP deps.
- `examples/web-channel-client/client/build.sh` - `wasm-pack build --target web --out-name web_channel_client`.
- `examples/web-channel-client/client/index.html` - Login UI, task status line, Cancel button.
- `examples/web-channel-client/client/main.js` - redirect detection + explicit 500ms poll loop + Cancel + logout/reconnect.
- `examples/web-channel-client/client/style.css` - mirrored from wasm-client + result/status styling.
- `examples/web-channel-client/client/.gitignore` - ignores `pkg/`, `target/`, `Cargo.lock` (wasm-pack/cargo artifacts).
- `examples/web-channel-client/README.md` - shared adaptation doc + single-user limitation.
- `Cargo.toml` - added `examples/web-channel-client/client` to the workspace `exclude` (standalone wasm example).

## Decisions Made
- **Task helpers return status strings.** `poll_task`/`cancel_task` return the snake_case `TaskStatus` string so the JS loop branches on `working`/`completed`/etc. without re-deserializing a `Task`. The explicit poll loop (the teachable part) stays in `main.js` per D-09 rather than hidden in a wasm helper.
- **`exchange_code` is a free function.** Isolating the Fetch token-exchange detail (headers, form body, JSON parse) out of the `#[wasm_bindgen]` methods keeps each method small (cognitive complexity ≤ 25, CLAUDE.md).
- **No `.cargo/config.toml`.** The 103-SPIKE Open Question 4 verdict (getrandom 0.4 selects the wasm backend from the `wasm_js` *feature*, not a rustflag) was confirmed empirically: the wasm `cargo build` and `wasm-pack build` both exit 0 with no `.cargo` config.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Excluded the client crate from the workspace**
- **Found during:** Task 1 (client crate build)
- **Issue:** A new crate under `examples/` is otherwise treated as a workspace member, which would force the wasm cdylib (`pmcp` `wasm`-only) into the host workspace build/lint and could leak feature unification — exactly the HIGH-2 split this plan must avoid (the sibling server crate was excluded the same way in plan 04).
- **Fix:** Added `examples/web-channel-client/client` to the root `Cargo.toml` workspace `exclude` list (alongside the already-excluded `.../server`).
- **Files modified:** Cargo.toml
- **Verification:** `cargo build --manifest-path examples/web-channel-client/client/Cargo.toml --target wasm32-unknown-unknown --lib` exits 0; `make quality-gate` exits 0; `cargo tree` shows no hyper/tokio/axum.
- **Committed in:** 273ab327 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking). The plan's `files_modified` listed a `client/.cargo/config.toml`; per the 103-SPIKE getrandom verdict it was correctly NOT created (no rustflag needed) — a planned-conditional file, not a deviation. The root `Cargo.toml` exclude edit is the one structural add required to honor the HIGH-2 split.
**Impact on plan:** No scope creep. `examples/wasm-client/*` is unchanged (LOCKED fence verified via `git diff --quiet`).

## Issues Encountered
- `ToolCallResponse` re-exports from `pmcp` (and `pmcp::client`), not `pmcp::types` — the import was corrected to `pmcp::ToolCallResponse` during Task 1.
- The bundled IdP registers the demo `redirect_uri` as `http://127.0.0.1:8080/callback`; since PKCE requires an exact `redirect_uri` match, the README documents serving the page so `/callback` resolves to `index.html` (the JS handles `?code=&state=` regardless of path), or editing the registration to match the serving setup. The server crate is committed (plan 04) and was NOT modified.

## Threat Flags
None — the example introduces no security surface beyond the plan's `<threat_model>`: CSRF state is verified before exchange (T-103-CSRF), the bearer lives in origin-scoped sessionStorage with the IndexedDB/httpOnly upgrade documented (T-103-XSS-TOKEN, accept), the `redirect_uri` is the registered loopback (T-103-OPENREDIR), randomness is the getrandom-backed pmcp helper (T-103-RNG), the high-level Client mints unique ids over the one-shot transport (T-103-REPLAY), and the single-user task limitation is documented (T-103-RACE, accept). No new external packages (T-103-SC) — deps mirror examples/wasm-client + the web-sys features the redirect/storage need.

## Known Stubs
None. The client is a fully-wired browser demo: real PKCE, real token exchange against the bundled IdP, real high-level Client task lifecycle. The `slow_summarize` payload and the single fixed `demo-user` identity are intentional offline-demo behaviors (documented), not unwired data sources. The single-user/no-concurrency limitation of the delayed task is documented (README + threat model), not a silent gap.

## User Setup Required
- Wasm toolchain for building the example: `rustup target add wasm32-unknown-unknown` + `cargo install wasm-pack` (both already present in this environment). Running the demo additionally needs a static file server for the client page and the bundled server process — all offline, no accounts/secrets (see README).

## Next Phase Readiness
- The browser EXAMPLE deliverable (SC-3) is complete: it builds to wasm with no native HTTP deps and is documented for pmcp.run adaptation. Combined with the 103-01 PKCE helper, the 103-02 transport fix, and the 103-04 demo server, the phase's "call MCP from a web application" story is end-to-end runnable (Manual E2E is the documented validation step).

## Self-Check: PASSED

- Created files verified on disk: `examples/web-channel-client/client/{src/lib.rs,src/utils.js,Cargo.toml,build.sh,index.html,main.js,style.css,.gitignore}`, `examples/web-channel-client/README.md`, and this SUMMARY.
- Task commits verified in git log: `273ab327` (feat), `7a662f53` (feat).
- `cargo build --target wasm32-unknown-unknown --lib` and `bash build.sh` (wasm-pack) both exit 0; `cargo tree` shows 0 hyper/axum/tokio; `examples/wasm-client/` is unchanged; `make quality-gate` exits 0 on the workspace.

---
*Phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas*
*Completed: 2026-06-30*
