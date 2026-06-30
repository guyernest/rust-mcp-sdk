# Web-Channel MCP Client (OAuth browser-PKCE + MCP Tasks)

A **strong reference** for calling MCP from a web application — the example the
[pmcp.run](https://pmcp.run) team lifts to build a web-app **channel**. It shows
the two advanced MCP capabilities a browser can drive today over plain
HTTP/Fetch (no SSE):

1. **OAuth via browser PKCE** — a real full-page-redirect Authorization Code +
   PKCE flow; the bearer token lives in the browser and is threaded into Fetch
   request headers.
2. **MCP Tasks lifecycle over Fetch** — `call(task) → poll tasks/get →
   tasks/result` (plus `tasks/cancel`) against a real `StreamableHttpServer`,
   driven through the **high-level** `pmcp::Client`.

Everything runs fully **offline** — the example bundles its own OAuth IdP and MCP
server, so there are no external accounts, secrets, or network dependencies.

## Layout — two crates, one example (the package split)

```
examples/web-channel-client/
├── client/      # WASM cdylib — the browser client (this is what runs in-page)
│   ├── Cargo.toml   #   pmcp { default-features = false, features = ["wasm"] }
│   ├── src/lib.rs   #   #[wasm_bindgen] WasmClient: PKCE + high-level task helpers
│   ├── src/utils.js #   structured-error glue
│   ├── build.sh     #   wasm-pack build --target web
│   ├── index.html   #   Login + task status + Cancel UI
│   ├── main.js      #   redirect-detect + explicit 500ms poll loop
│   └── style.css
└── server/      # native binary — the bundled OAuth IdP + MCP demo server
    └── src/main.rs
```

**Why two crates?** Cargo unifies dependency features *per package build*. A
single package cannot host both a wasm cdylib (`pmcp` with only the `wasm`
feature) and a native server (`pmcp` with `full`/`streamable-http`) without the
wasm build pulling `hyper`/`tokio`/`axum` into the `.wasm` artifact. Splitting
into `client/` (wasm) and `server/` (native) keeps the browser build free of any
native HTTP stack. Both crates are excluded from the workspace.

## Running the demo

You need three things: the wasm toolchain, the bundled server, and a static file
server for the client page.

```bash
# 0. One-time: install the wasm target + wasm-pack.
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# 1. Build the browser client to wasm.
cd examples/web-channel-client/client
./build.sh                      # -> pkg/web_channel_client{.js,_bg.wasm}

# 2. In another terminal, start the bundled IdP + MCP demo server (port 8787).
cargo run --manifest-path examples/web-channel-client/server/Cargo.toml

# 3. Serve the client page on http://127.0.0.1:8080 (the registered redirect
#    origin) and open it.
cd examples/web-channel-client/client
python3 -m http.server 8080
# open http://127.0.0.1:8080/index.html
```

Then click **Login (PKCE redirect)**. The page redirects to the bundled IdP,
which redirects back with `?code=&state=`; the client validates `state`,
exchanges the code for a bearer, connects, and enables **Run slow_summarize
task**. Click it and watch the task status flip `working → completed` across the
500 ms polls, then the result appears. Click **Cancel task** mid-run to drive
`tasks/cancel`.

### The redirect URI

The bundled IdP pre-registers the client `redirect_uri` as
`http://127.0.0.1:8080/callback`. PKCE requires the `redirect_uri` sent at
authorize-time to match a registered URI exactly. For the simplest run, serve
the client so that path resolves to `index.html` — e.g. copy `index.html` to a
file named `callback`, or use any static server that serves `index.html` for
`/callback`. The page's JS handles `?code=&state=` regardless of the path it is
served from. Alternatively, change the registered `redirect_uri` in
`server/src/main.rs` and the `REDIRECT_URI` constant in `main.js` to match your
serving setup.

## How the pieces fit (adapting this as a web-app channel)

- **PKCE (browser-side):** `WasmClient::begin_login` calls the `pmcp` PKCE helper
  (`generate_code_verifier` / `code_challenge_s256` / `generate_state`), stashes
  the verifier + CSRF `state` in `sessionStorage`, and returns the authorize URL
  for `window.location = ...`. On return, `complete_login` verifies `state`
  (CSRF defense), then `POST`s `grant_type=authorization_code` + `code` +
  `code_verifier` (+ `redirect_uri`, `client_id`) as
  `application/x-www-form-urlencoded` to `/oauth2/token` and stores the bearer.
- **Transport (bearer-in-Fetch):** `connect` builds
  `Client::new(WasmHttpTransport::new(config))` with the bearer in
  `extra_headers` as `Authorization: Bearer <token>`. The transport injects that
  header on every Fetch — so the high-level client and all four typed task
  helpers work in the browser unchanged.
- **Tasks lifecycle:** `main.js` runs an explicit, visible `setTimeout` poll loop
  on `tasks/get` at a fixed 500 ms interval until the status is terminal, then
  fetches `tasks/result`. The Cancel button calls `tasks/cancel`.

To adapt for a **real provider** (Google / GitHub / Auth0 / your own IdP): point
`server-origin` (or the `authorize`/`token` URLs in `main.js`) at the real
provider's endpoints, register this client and its `redirect_uri`, and keep the
same browser PKCE + bearer-in-Fetch + task-poll structure. The MCP half is
unchanged — only the IdP endpoints and client registration differ.

## Token storage & the durability upgrade path

This demo stores the `code_verifier`, CSRF `state`, and bearer in
**`sessionStorage`** (origin-scoped, synchronous, cleared on tab close) — a good
default for a demo. For production durability across tabs/restarts, **IndexedDB**
is the upgrade path; tokens that must never be reachable from JS belong in an
`httpOnly` cookie set by a backend-for-frontend. Neither is implemented here.

## Known limitation — single-user / no concurrency for the delayed task

The bundled server's `slow_summarize` is a deliberately delayed task: the tool
returns `status: "working"` and a background updater completes it a few seconds
later, so the browser can poll several times and demonstrate Cancel. To complete
the *correct* store-minted task, the updater would ideally correlate it via a
unique marker the tool sets — but the SDK create-path cannot carry such a marker
this phase (the tool never sees the store-minted id, and the `Task` type has no
tool-writable variable/metadata field). The updater therefore diffs a pre-create
snapshot of the owner's `Working` task ids and completes the single *new* id; if
two of the **same owner's** tasks are created concurrently it declines to guess
(completes nothing) rather than complete the wrong one.

**Consequence:** the delayed-task demo is **single-user / no-concurrency** for a
given authenticated owner. This is a property of the *demo updater*, not of the
MCP Tasks protocol or the SDK's task store, and is the documented `T-103-RACE`
acceptance (carried over from the server plan's MEDIUM-6). A production server
would attach a real correlation id to the task it creates.
