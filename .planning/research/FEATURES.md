# Feature Research: MCP Apps Developer Experience (v1.3)

**Domain:** Developer tooling for interactive UI widgets embedded in MCP servers (ChatGPT Apps, MCP Apps SEP-1865, MCP-UI)
**Researched:** 2026-02-24
**Confidence:** HIGH for preview completion and Playwright E2E (existing infrastructure is ~80% there); MEDIUM for ChatGPT manifest publishing (spec is stable but OpenAI review process is opaque); MEDIUM for WASM bridge injection (novel pattern, limited prior art)

## Context

This research targets v1.3 only. The following already exist and are NOT in scope:

- Core `mcp_apps.rs` types (`WidgetCSP`, `ChatGptToolMeta`, `ExtendedUIMimeType`, etc.)
- `ChatGptAdapter` and `UIAdapter` in `server::mcp_apps`
- Chess and Map example apps with `widget/` HTML files and `preview.html` mock bridges
- `mcp-preview` crate: Axum server, proxy, asset handler, full `index.html` with bridge injection, tool list/call API, WebSocket handler skeleton
- WASM client with dual transport (WebSocket + HTTP) in `examples/wasm-client/`
- `cargo pmcp preview` command wiring
- Playwright scaffolding: `chess-widget.spec.ts`, `mock-mcp-bridge.ts` fixture, `playwright.config.ts`
- `cargo pmcp` deploy targets (Lambda, Cloud Run, Workers) and `landing` command skeleton

The mcp-preview widget iframe rendering is **NOT working** yet: the bridge JavaScript in `wrapWidgetHtml()` uses `window.parent.previewRuntime` cross-origin access which fails when the widget HTML is loaded as `srcdoc` with different origin assumptions, and the `handleToolResponse` handler looks for `content_type === 'resource'` but MCP widgets are typically served as MCP resources that must be fetched separately by URI, not returned inline in tool responses.

The Playwright tests reference `page.goto('/chess/board.html')` but the `playwright.config.ts` `baseURL` and the file server (`serve.js`) are not yet wired to the chess widget directory.

---

## Feature Area 1: mcp-preview Widget Iframe Rendering

### What Expected Behavior Looks Like

A developer runs `cargo pmcp preview --url http://localhost:3000 --open`. A browser opens at `http://localhost:8765`. The left panel lists the server's tools (fetched via `/api/tools`). The developer selects a tool that has a widget resource (e.g., `chess_new_game`) and clicks Execute. The center panel renders the widget in an iframe. The widget works: clicking pieces calls `chess_move` and the board updates. The right DevTools panel shows State, Console, Network, and Events tabs updating in real time.

The bridge is the critical link. The widget's `window.mcpBridge.callTool(name, args)` must reach the PMCP server through the preview server proxy. The current `index.html` already injects the bridge via `srcdoc` — but the injected bridge uses `window.parent.previewRuntime` which breaks in sandboxed iframes. The correct pattern uses `postMessage` for cross-origin iframe communication.

### Table Stakes

| Feature | Why Expected | Complexity | Dependencies on Existing Code |
|---------|--------------|------------|-------------------------------|
| **Widget iframe renders from resource URI** | The whole purpose of mcp-preview. Without rendering, the tool is useless. | MEDIUM | `mcp-preview` proxy already calls `tools/call`; must add `resources/read` proxy call to fetch widget HTML by URI |
| **`window.mcpBridge.callTool()` routes to real MCP server** | Chess and Map widgets call `callTool()` on every user interaction. Bridge must reach the live server. | MEDIUM | Fix postMessage architecture in `wrapWidgetHtml()`. Bridge in iframe sends postMessage to parent; parent calls `/api/tools/call`; result posted back. CSP headers must allow parent-child messaging. |
| **MCP session initialization (handshake)** | MCP requires `initialize` before any method. Current `McpProxy::list_tools()` calls initialize each time (inefficient). Must call once and reuse session. | LOW | `McpProxy` already has `initialize()`. Add session state: track whether initialized. `initialize` once on preview server start. |
| **Resource fetching for widget HTML** | Widgets are served as MCP resources (`ui://chess/board.html`). The preview must call `resources/read` to get the HTML. Current code looks for inline HTML in tool response content, which is not how MCP Apps works. | MEDIUM | Add `/api/resources/read?uri=...` endpoint to preview. `McpProxy::read_resource(uri)` method. In `handleToolResponse`, scan tool result for `_meta.ui.widget` URI, then fetch that resource. |
| **`window.mcpBridge.getState()` and `setState()` work** | Widget state persistence across tool calls (chess board position, map center). DevTools State tab shows current state. | LOW | Already sketched in `wrapWidgetHtml()`. Wire postMessage state sync. |
| **DevTools panels update in real time** | Developers need to see what the widget is doing. Console, Network, Events should update on bridge calls. | LOW | Already in `index.html`. Must ensure postMessage from iframe triggers devtools log updates in parent. |
| **Connection status shows connected/disconnected** | Immediate feedback if the MCP server is not running. | LOW | Already in UI. `status-dot` element and `setStatus()` function exist. Ensure proxy errors surface here. |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Auto-reload on server restart** | When the MCP server restarts (hot-reload during development), the preview reconnects and refreshes the tool list. | MEDIUM | WebSocket handler skeleton exists (`/ws` route). Use WebSocket to signal reconnect. Poll `/api/tools` on disconnect. |
| **Environment variable simulation** | Theme (light/dark), locale, display mode, maxHeight, safeArea. Widget receives `openai/setGlobals` postMessage when env changes. | LOW | Already in `index.html` env-controls. `emitGlobalsUpdate()` already implemented. Just needs the postMessage fix to actually reach the iframe. |
| **Multiple widget resources panel** | Server may expose multiple UI resources. Show a resource picker in addition to tool list. | MEDIUM | Add `/api/resources` endpoint. Add resource panel to UI. |
| **Fullscreen mode** | Widget expands to fill viewport, simulating the Picture-in-Picture or expanded view mode in ChatGPT. | LOW | `.widget-frame-container.fullscreen` class already defined. Toggle logic in `updateDisplayMode()` already exists. |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Re-implement MCP session management in preview** | The preview is a dev tool, not a production client. Session complexity is a maintenance burden. | Use stateless HTTP POST to `/mcp` for each tool/resource call. MCP server handles its own statefulness. |
| **Hot module replacement (HMR) for widget HTML** | Watching widget file changes and pushing updates requires a file watcher, which adds OS-specific complexity. | Developers refresh the browser. Good enough for a dev tool. Add auto-reload later if demand exists. |
| **Preview server authentication** | The preview runs locally on 127.0.0.1. Adding auth adds friction with zero security benefit. | Document that preview is local-only. CORS is already set to `Any`. |

### Feature Dependencies

```
McpProxy::read_resource(uri) [NEW]
    required by: widget iframe rendering from resource URI

postMessage bridge architecture [REPLACE wrapWidgetHtml cross-origin access]
    required by: callTool(), getState(), setState(), DevTools updates
    required by: environment variable simulation (openai/setGlobals)

/api/resources/read endpoint [NEW]
    required by: widget iframe rendering
    requires: McpProxy::read_resource

/api/resources endpoint [NEW, optional]
    required by: multiple widget resources panel

MCP session initialization once [IMPROVE McpProxy]
    required by: all proxy calls (performance, correctness)
```

---

## Feature Area 2: WASM Widget Test Client (In-Browser MCP)

### What Expected Behavior Looks Like

A developer opens `http://localhost:8765` (preview server). In the widget iframe, the bridge connects not via server-side HTTP proxy but via the WASM MCP client running in the browser. The WASM client connects directly to the MCP server using HTTP transport. Tool calls from the widget go Browser → WASM client → MCP server (no server-side proxy hop). This enables testing widgets in conditions closer to how ChatGPT runs them: the client is in the browser, not on a server.

Alternatively: the WASM client is injected by the preview server into the widget iframe as the `window.mcpBridge` implementation, allowing the widget to be loaded from any origin.

The WASM client already exists (`examples/wasm-client/`). It has dual transport (WebSocket + HTTP) and exposes `connect()`, `list_tools()`, `call_tool()` via `wasm-bindgen`. The gap is:
1. The WASM client is not wired into the preview server's bridge injection.
2. The WASM client's `call_tool()` returns a `CallToolResult` structure, not the flat JSON that `window.mcpBridge.callTool()` widgets expect (they expect just the tool output value, not the `{content: [{type, text}]}` wrapper).

### Table Stakes

| Feature | Why Expected | Complexity | Dependencies on Existing Code |
|---------|--------------|------------|-------------------------------|
| **WASM client loads in preview iframe context** | If the WASM client is the bridge implementation, it must load inside the iframe. The iframe is served via `srcdoc` so WASM loading needs special handling (blob URL or `importmap`). | HIGH | `examples/wasm-client/pkg/` has compiled WASM + JS. Preview server must serve WASM as a static asset, then inject `<script type="module">` loading it. |
| **Bridge adapter: WASM client → `window.mcpBridge` shape** | The WASM `call_tool()` returns `{content: [{type: "text", text: "..."}]}`. Widget code calls `callTool()` and expects the unwrapped tool output (e.g., a `GameState` object). The adapter must unwrap MCP content format into the tool's return value. | MEDIUM | New JavaScript shim layer. Parse `CallToolResult.content[0].text` as JSON if content type is `text`. If content type is `resource` with widget MIME type, load the widget. |
| **WASM client handles CORS (HTTP transport to local server)** | The widget iframe is at `http://localhost:8765` and the MCP server is at `http://localhost:3000`. Cross-origin fetch is blocked without CORS headers. | LOW | PMCP's `StreamableHttpServer` already emits CORS headers. Confirm `Access-Control-Allow-Origin: *` is present. The preview server already sets `CorsLayer::new().allow_origin(Any)`. |
| **Connection URL configuration passed to WASM client** | The WASM client must know the MCP server URL. The preview server knows this (`config.mcp_url`). Must inject the URL into the iframe context. | LOW | Pass `mcp_url` from `/api/config` response. Inject into bridge setup script. |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Side-by-side comparison: proxy mode vs WASM mode** | Preview server has two bridge modes: (a) server-side proxy (simple), (b) WASM client in browser (closer to production ChatGPT). Toggle in UI. | HIGH | Mode A (proxy) uses `/api/tools/call`. Mode B (WASM) loads WASM client. Same widget, two modes, one preview UI. |
| **WASM client as standalone `mcpBridge` polyfill** | Developers can include the WASM client in their widget's `preview.html` and get a real MCP bridge without running the preview server. | MEDIUM | Package the WASM + bridge adapter as `widget-runtime.js`. Widgets `<script src="widget-runtime.js">` and it provides `window.mcpBridge`. This is the `packages/widget-runtime/` referenced in chess README but not yet built. |
| **Request ID tracking fix** | Current WASM HTTP client uses hardcoded IDs (`id: 3i64.into()`). In concurrent widget scenarios, ID collisions produce incorrect responses. | LOW | Add atomic ID counter. Already noted as `// TODO` in source. |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **WASM client in production widget bundle** | Embedding the full WASM MCP client in every widget HTML adds ~200KB+ per widget. ChatGPT injects its own bridge — widgets should use `window.mcpBridge`, not bundle their own client. | WASM client is for the preview server environment only. Production widgets assume `window.mcpBridge` exists (injected by host). |
| **WebSocket transport for WASM-to-server connection** | WebSocket requires a persistent connection. Lambda/serverless deployments (main PMCP target) do not support persistent WebSocket connections. HTTP transport is the only viable option. | Use WASM HTTP transport exclusively for MCP Apps widgets. WebSocket WASM transport is for other use cases. |

### Feature Dependencies

```
Preview server serves WASM assets at /wasm/* [NEW]
    requires: wasm-client/pkg/ copied to mcp-preview assets

Bridge adapter JavaScript (wasm-client → mcpBridge shape) [NEW]
    requires: WASM client loaded in iframe

MCP server URL injected into iframe [NEW]
    requires: /api/config endpoint (already exists)

Request ID atomic counter in WASM HTTP client [FIX]
    required by: concurrent tool calls in WASM mode
```

---

## Feature Area 3: Publishing — ChatGPT Manifest + Deploy Targets

### What Expected Behavior Looks Like

**ChatGPT manifest generation:** A developer runs `cargo pmcp deploy --target chatgpt-apps` (or a subcommand like `cargo pmcp manifest --chatgpt`). The CLI reads the server's tool definitions (via `cargo pmcp schema export` or by connecting to the running server) and generates a `chatgpt-manifest.json` describing the app for submission to the ChatGPT App Directory. The file includes server URL, app name, tool definitions, and HTTPS verification metadata.

**Demo landing page:** `cargo pmcp preview --demo` generates a standalone `demo/index.html` with a mock bridge. The developer opens it in a browser, clicks through the widget, takes screenshots for the App Directory submission. No server needed.

**Deploy extension for MCP Apps:** `cargo pmcp deploy --target aws-lambda` already works for basic MCP servers. The extension adds: (a) widget asset serving — the lambda must serve widget HTML at the `ui://` resource URIs; (b) HTTPS URL in generated outputs so it can be submitted to ChatGPT.

### Table Stakes

| Feature | Why Expected | Complexity | Dependencies on Existing Code |
|---------|--------------|------------|-------------------------------|
| **ChatGPT-compatible manifest generation** | ChatGPT App Directory requires a specific manifest format for submission. Without this, the app cannot be submitted. | MEDIUM | Read tool definitions from server (`McpProxy::list_tools()`). Generate JSON matching ChatGPT Apps schema (name, description, server URL, HTTPS endpoint, auth type). `cargo-pmcp` already has `schema` command for exporting MCP schema. |
| **`text/html+skybridge` MIME type verified on deploy** | ChatGPT requires resources to use `text/html+skybridge` MIME type. If the MIME type is wrong, the widget won't render. | LOW | `ExtendedUIMimeType::HtmlSkybridge` already defined in `mcp_apps.rs`. Verification is a lint/check step in deploy. |
| **HTTPS URL in deployment outputs** | ChatGPT will not register an HTTP server. The deployment output must include the HTTPS endpoint. | LOW | `cargo-pmcp` outputs already track deployed URL. Add HTTPS validation: warn if URL is not `https://`. Lambda + API Gateway always produces HTTPS. Cloud Run always HTTPS. |
| **Widget HTML served from MCP resources endpoint** | The deployed server must serve widget HTML at the `ui://` resource URIs when `resources/read` is called. | LOW | Chess example already does this via `ChessResources` handler. The deploy check must verify at least one resource with `text/html+skybridge` MIME exists. |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Auto-generated demo landing page with mock bridge** | Stakeholders and reviewers can interact with the widget before the server is deployed. The landing page embeds the chess/map widget HTML with a fully functioning mock bridge and tool call log. | MEDIUM | Generate a single-file HTML combining: widget HTML, mock bridge script (like `preview.html` today), tool responses hardcoded or editable in the page. `cargo pmcp preview --demo --output demo.html`. |
| **`cargo pmcp deploy --mcp-apps` flag** | Extends the existing deploy targets to verify MCP Apps requirements and output the manifest. | LOW | Reuse existing deploy targets. Add a `--mcp-apps` flag that triggers manifest generation and HTTPS verification after deploy. |
| **ChatGPT App Directory submission checklist** | After generating the manifest, print a checklist: HTTPS endpoint present, MIME types correct, CSP configured, widget renders in preview. Reduces submission rejections. | LOW | CLI output only. No new API calls. Gather from existing deploy outputs. |
| **Manifest validation against ChatGPT schema** | Validate the generated manifest against the ChatGPT Apps schema before submission. Catches missing fields, invalid URLs, wrong MIME types. | MEDIUM | Embed the ChatGPT manifest JSON schema in `cargo-pmcp`. Validate at manifest generation time. Fail fast with actionable errors. |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Automated ChatGPT App Directory submission** | The submission process requires OAuth, human review, and manual approval. Automating submission is not possible and would break with ChatGPT API changes. | Generate the manifest and print instructions for manual submission. |
| **Publishing to non-ChatGPT stores (Claude App Directory, etc.)** | These stores do not exist yet (as of 2026-02). Building for hypothetical stores wastes effort. | Build the ChatGPT manifest. Make the manifest structure extensible. When other stores emerge, add adapters. |
| **Complex deploy orchestration for widget CDN** | Hosting widget assets on a CDN (CloudFront, etc.) separate from the MCP server adds infrastructure complexity. | Widgets are served by the MCP server itself via `resources/read`. The server IS the CDN. No separate asset hosting needed. |

### Feature Dependencies

```
ChatGPT manifest JSON generation [NEW]
    requires: McpProxy::list_tools() (already exists in mcp-preview)
    requires: deployment output URL (already in cargo-pmcp deploy outputs)
    requires: mcp_apps::WidgetCSP serialization to ChatGPT format

Demo landing page generation [NEW]
    requires: widget HTML from server (resources/read)
    requires: mock bridge JavaScript (pattern exists in preview.html)
    requires: cargo-pmcp CLI subcommand

Deploy --mcp-apps flag [NEW]
    requires: existing deploy targets (all three already work)
    requires: ChatGPT manifest generation
```

---

## Feature Area 4: Widget Authoring DX

### What Expected Behavior Looks Like

**File-based widgets:** Currently the chess server does `include_str!("../widget/board.html")` — the widget HTML is already a separate file. The pattern works. The issue is that the `cargo pmcp new` scaffolding generates no widget file: it only creates a calculator server with no widget. The `--mcp-apps` template creates a server with a placeholder widget file, a `preview.html` mock bridge, and a `Cargo.toml` with `mcp-apps` feature enabled.

**Shared bridge library:** Both chess and map copy the same mock bridge pattern into their `preview.html`. This is duplication. A shared JavaScript file (`widget-runtime.js`) provides `window.mcpBridge` with the mock implementation, usable in any `preview.html` by including one `<script>` tag. This file ships as a PMCP static asset, served by `cargo pmcp preview` at `/widget-runtime.js`.

**Scaffolding:** `cargo pmcp new --mcp-apps my-widget` creates a complete MCP Apps workspace: Rust server with 1-2 example tools, `widget/index.html` with the bridge integration pattern, `preview.html` using the shared bridge library, and Playwright test skeleton.

### Table Stakes

| Feature | Why Expected | Complexity | Dependencies on Existing Code |
|---------|--------------|------------|-------------------------------|
| **File-based widget authoring (separate HTML file, not Rust inline strings)** | Already implemented in chess and map examples via `include_str!`. The pattern works. The scaffolding template must generate a widget file, not inline HTML. | LOW | Template generation in `cargo-pmcp/src/templates/`. Add `mcp_apps` template variant. |
| **`cargo pmcp new --mcp-apps` scaffolding template** | Developers starting a new MCP App need a working starting point. Without scaffolding, they manually copy the chess example. | MEDIUM | Add new template to `cargo-pmcp/src/templates/mod.rs`. Generate: `src/main.rs` with `ChatGptAdapter`, `widget/index.html` with bridge boilerplate, `preview.html` with shared bridge, `Cargo.toml` with `mcp-apps` feature. Wire into `cargo pmcp new` and `cargo pmcp add server` commands. |
| **Shared bridge library (`widget-runtime.js`) served by preview server** | Chess and map both copy the same 200-line mock bridge script. Single source of truth eliminates drift when the bridge API changes. | LOW | Extract `wrapWidgetHtml()` bridge JavaScript from `index.html` into `assets/widget-runtime.js`. Serve at `/widget-runtime.js`. Update `preview.html` templates to `<script src="http://localhost:8765/widget-runtime.js">`. |
| **Widget HTML hot-reload on save (development mode)** | MCP App developer edits `widget/index.html`, refreshes the preview browser, sees changes. | LOW | No server change needed. The widget HTML is re-fetched on each tool execute. `srcdoc` is reset. Developer just clicks Execute again. Or: add a Refresh button in the preview UI that re-runs the last tool call. |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Widget authoring guide in scaffolded README** | Generated `README.md` explains the bridge API (`callTool`, `getState`, `setState`), the stateless widget pattern, CSP configuration, and how to run the preview. Removes the need to read the full MCP Apps spec. | LOW | Template content. No code changes. |
| **Bridge API type definitions (`widget-runtime.d.ts`)** | TypeScript type definitions for `window.mcpBridge` and `window.openai`. Widget HTML with `<script type="module">` can import types for IDE autocompletion. | LOW | Hand-write `.d.ts` file. Ship alongside `widget-runtime.js`. |
| **Stateless widget pattern enforcement in template** | The scaffolded widget uses the stateless pattern from the chess example (full state sent with each tool call). Comment in the generated code explains why: no server-side sessions, works across MCP hosts, easy horizontal scaling. | LOW | Template content. One comment block. |
| **Widget CSP configuration helper** | The scaffolded `main.rs` includes `WidgetCSP::new().connect("https://...")` with commented examples for common cases (external API, CDN, tile server). | LOW | Template content. |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **React/Vue/Svelte widget template** | Frontend framework templates balloon the scaffolding surface: different build steps, different CSP requirements, different bundle configurations. The chess and map examples prove vanilla HTML/JS is sufficient for rich widgets. | Generate vanilla HTML/JS templates. Document that any framework works if you produce a single HTML file (per the MCP Apps spec). |
| **Widget build pipeline integration (Vite, webpack, etc.)** | A build step for widgets adds Node.js as a requirement, which is jarring in a Rust-first ecosystem. The MCP Apps spec supports raw HTML. | Serve raw HTML. If developers want a build step, they call `vite build` themselves and point the server at the output. |
| **Widget versioning and CDN hosting** | Versioning widget HTML files on S3/CloudFront adds infrastructure complexity. Widgets change with the server code — they should be versioned together. | Widgets are embedded in the Rust binary via `include_str!`. They version with the server. No separate CDN needed. |

### Feature Dependencies

```
Shared bridge library (widget-runtime.js) [NEW]
    required by: cargo pmcp new --mcp-apps template (preview.html uses it)
    required by: WASM widget test client (WASM mode uses it)
    served by: mcp-preview static assets handler (already exists)

cargo pmcp new --mcp-apps template [NEW]
    requires: shared bridge library
    requires: templates/mod.rs update
    requires: ChatGptAdapter usage pattern (already in chess example)

cargo pmcp add server --mcp-apps flag [NEW, additive]
    requires: mcp_apps template variant
    same as: cargo pmcp new but adds server to existing workspace
```

---

## Feature Area 5: Ship Examples + Playwright E2E

### What Expected Behavior Looks Like

**Chess and Map examples ship:** `cargo run --example mcp-apps-chess` builds and starts the server. `cargo run --example mcp-apps-map` builds and starts the map server. Both are in the workspace examples. Both have complete READMEs. Both compile with `--features mcp-apps`. Both have `preview.html` files that work in the browser.

**Playwright E2E tests pass:** `cd tests/playwright && npm install && npx playwright test` passes all tests in `chess-widget.spec.ts`. The test server (`serve.js`) serves the widget HTML at the paths the tests expect (`/chess/board.html`, `/map/map.html`). The mock bridge fixture works.

### Table Stakes

| Feature | Why Expected | Complexity | Dependencies on Existing Code |
|---------|--------------|------------|-------------------------------|
| **Chess example compiles and runs** | `cargo build --example mcp-apps-chess --features mcp-apps` produces a working binary. Currently the example depends on `pmcp::server::mcp_apps` types that may not be wired correctly. | LOW | Chess example code already complete. Verify feature flag wiring. Add to workspace Cargo.toml examples section if not present. |
| **Map example compiles and runs** | Same as chess. The map example uses Leaflet.js (loaded from CDN) — must verify CSP allows `cdn.leafletjs.com` in the widget HTML. | LOW | Map example code exists. Same as chess: feature flag verification. |
| **Playwright test server serves widget files** | The `serve.js` in `tests/playwright/` must serve `examples/mcp-apps-chess/widget/board.html` at `/chess/board.html`. | LOW | `serve.js` exists but may not be configured to serve from example directories. Update `playwright.config.ts` `webServer` command. |
| **All chess widget Playwright tests pass** | 10 tests in `chess-widget.spec.ts`. Mock bridge handles `chess_new_game`, `chess_move`, `chess_valid_moves`. Widget must: show 64 squares, select pieces, highlight valid moves, make moves, update status. | MEDIUM | Tests already written. Widget may need `data-file` and `data-rank` attributes on squares, `#board`, `#status`, `#newGameBtn` IDs. May require updates to `widget/board.html`. |
| **Map widget Playwright tests** | Equivalent to chess tests but for the map. Search, category filter, marker click, city list, detail panel. | MEDIUM | Tests not yet written. Add `tests/playwright/tests/map-widget.spec.ts`. |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Playwright test report with widget screenshots** | `npx playwright test --reporter=html` produces an HTML report with screenshots of the widget at each test step. Useful for sharing with stakeholders. | LOW | Playwright built-in. Add `screenshot: 'on'` to `playwright.config.ts`. |
| **Integration test: preview server + chess server** | A Playwright test that: starts chess MCP server, starts preview server pointing at chess, opens preview, selects `chess_new_game` tool, clicks Execute, verifies widget renders. End-to-end with real servers. | HIGH | Requires starting two servers from within Playwright setup. `playwright.config.ts` `webServer` supports multiple servers. Complex but high value. |
| **Widget accessibility tests** | Playwright axe-core integration verifies the chess board has proper ARIA labels. Screen reader accessibility. | MEDIUM | `@axe-core/playwright` npm package. Not in scope for v1.3 MVP. |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Cross-browser testing matrix (Chrome, Firefox, Safari, WebKit)** | MCP Apps widgets are rendered by the host (ChatGPT, Claude) in their own WebView. The developer's preview is Chromium. Testing other browsers for the preview tool is low value. | Default Playwright to Chromium only. Document that production hosts use their own rendering. |
| **Visual regression testing (screenshot diffing)** | Widget UIs change frequently during development. Snapshot drift causes false failures. | Save screenshots for manual review, not as test assertions. |
| **Testing with real ChatGPT** | ChatGPT's review process involves human reviewers, not automated tests. | Test with the mock bridge. The bridge is a faithful simulation of the ChatGPT bridge API. |

### Feature Dependencies

```
Playwright test server wired to widget directories [FIX]
    required by: all Playwright tests

Widget HTML data attributes and IDs match Playwright selectors [FIX/VERIFY]
    required by: chess-widget.spec.ts assertions
    may require: updates to examples/mcp-apps-chess/widget/board.html

Map widget Playwright tests [NEW]
    requires: Playwright test server serves map widget
    requires: map widget has testable IDs

Integration test: preview + chess [NEW, optional]
    requires: both servers startable from playwright setup
    requires: preview server bridge working (Feature Area 1)
```

---

## Feature Dependencies (Cross-Area)

```
[Feature Area 1: mcp-preview Bridge Fix]
    |
    +--enables--> [Feature Area 2: WASM mode in preview]
    |               (WASM bridge replaces proxy bridge in preview)
    |
    +--enables--> [Feature Area 5: Integration Playwright test]
    |               (preview working = end-to-end test possible)
    |
    +--enables--> [Feature Area 3: Demo landing page]
                    (demo page uses same bridge injection pattern)

[Feature Area 4: Shared bridge library]
    |
    +--used-by--> [Feature Area 1: preview bridge]
    +--used-by--> [Feature Area 4: scaffolded preview.html]
    +--used-by--> [Feature Area 3: demo landing page]

[Feature Area 5: Chess + Map examples ship]
    |
    +--validates--> [Feature Area 4: widget authoring DX]
                      (examples ARE the reference implementation)
```

---

## MVP Recommendation

### Launch With (v1.3.0)

Priority order based on dependencies and user value:

**P1 — Blocking (must work or the milestone fails):**
1. **mcp-preview bridge fix** (Feature Area 1): Replace `window.parent.previewRuntime` with postMessage. Add `resources/read` proxy. Without this, the preview tool shows the UI but widgets don't work.
2. **Chess and map examples ship** (Feature Area 5): `cargo build` must succeed. `preview.html` must work. These are the only concrete deliverables users can try.
3. **Playwright tests pass** (Feature Area 5): Tests exist but may need widget HTML attribute fixes. Green CI is table stakes for "shipped."

**P2 — High value, not blocking:**
4. **`cargo pmcp new --mcp-apps` scaffolding** (Feature Area 4): Enables community adoption. Without this, users copy the chess example manually.
5. **Shared bridge library (`widget-runtime.js`)** (Feature Area 4): Reduces duplication. Trivial once preview bridge is fixed.
6. **ChatGPT manifest generation** (Feature Area 3): Required for App Directory submission. Medium complexity, high practical value.

**P3 — Ship if time allows:**
7. **WASM widget test client** (Feature Area 2): Differentiator, but proxy mode (P1) already works. WASM mode is a "closer to production" option.
8. **Demo landing page generation** (Feature Area 3): Nice for demos, not required for publishing.
9. **Map Playwright tests** (Feature Area 5): Chess tests validate the pattern. Map tests are duplicative work.

### Defer to v1.4+

- WASM bridge polyfill (`widget-runtime.js` with WASM client inside)
- Integration Playwright test (preview + real server)
- Manifest validation against ChatGPT schema
- Multiple widget resources panel
- Auto-reload on server restart via WebSocket

---

## Competitor / Ecosystem Analysis

| Capability | Official MCP Apps SDK (`@modelcontextprotocol/ext-apps`) | OpenAI Apps SDK | PMCP SDK v1.3 (target) |
|------------|----------------------------------------------------------|-----------------|------------------------|
| **Preview tool** | No built-in preview. Developers use the MCP Inspector (tool-call focus, no widget rendering). | No preview tool. Use developer mode in ChatGPT directly. | `cargo pmcp preview`: full widget rendering with DevTools. **Best-in-class for Rust.** |
| **Bridge simulation** | Manual: create `preview.html` with `window.mcpBridge`. | Manual: same pattern. | Shared `widget-runtime.js`. Mock bridge + proxy bridge. Lower friction. |
| **Scaffolding** | `npx @modelcontextprotocol/create-app` (TypeScript only). | OpenAI Apps Builder. No CLI for MCP. | `cargo pmcp new --mcp-apps` (Rust). **First Rust scaffolding for MCP Apps.** |
| **Publishing** | Manual manifest creation. | Apps SDK has manifest helpers (TypeScript). | `cargo pmcp manifest --chatgpt`. **First Rust manifest generator.** |
| **Widget testing** | No standardized test harness. Use Playwright manually. | No test harness. | Playwright fixture (`mock-mcp-bridge.ts`) ships with SDK. **Differentiator.** |
| **WASM widget client** | Not applicable (TypeScript SDK runs in Node.js). | Not applicable. | Unique to PMCP. Browser-native MCP client. **Only Rust SDK with this.** |

---

## Sources

### Authoritative (HIGH confidence)
- Codebase analysis: `crates/mcp-preview/assets/index.html` — bridge injection implementation, identified cross-origin bug
- Codebase analysis: `crates/mcp-preview/src/proxy.rs` — proxy architecture, missing `resources/read`
- Codebase analysis: `examples/wasm-client/src/lib.rs` — WASM client capabilities, hardcoded ID bug
- Codebase analysis: `cargo-pmcp/src/templates/server.rs` — no `--mcp-apps` template variant exists
- Codebase analysis: `tests/playwright/tests/chess-widget.spec.ts` — 10 tests targeting specific HTML selectors
- Codebase analysis: `.planning/PROJECT.md` — v1.3 milestone scope, existing infrastructure inventory

### Verified (MEDIUM confidence)
- [MCP Apps Official Docs](https://modelcontextprotocol.io/docs/extensions/apps) — `text/html+mcp` MIME type, resource registration pattern
- [ext-apps GitHub](https://github.com/modelcontextprotocol/ext-apps) — official SDK, `app-bridge` postMessage protocol
- [@modelcontextprotocol/ext-apps v1.1.0 API](https://modelcontextprotocol.github.io/ext-apps/api/) — `App.callServerTool()` API shape
- [MCP Apps Blog 2026-01-26](http://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/) — official launch, current host support (Claude, VS Code, Goose, Postman, MCPJam)
- [OpenAI Apps SDK build MCP server](https://developers.openai.com/apps-sdk/build/mcp-server/) — ChatGPT deployment requirements (HTTPS mandatory)
- [ChatGPT Apps MCP Developer Mode](https://help.openai.com/en/articles/12584461-developer-mode-apps-and-full-mcp-connectors-in-chatgpt-beta) — App Directory submission process, December 2025 launch
- [Connect from ChatGPT guide](https://developers.openai.com/apps-sdk/deploy/connect-chatgpt/) — HTTPS requirement, manifest fields

### Additional Context (LOW confidence)
- [MCP Apps playground](https://github.com/digitarald/mcp-apps-playground) — widget authoring patterns in the wild
- [SEP-1865 PR](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865) — original proposal, spec background

---

*Feature research for: MCP Apps Developer Experience (v1.3 milestone)*
*Researched: 2026-02-24*
*Key findings: (1) mcp-preview bridge is 80% done but has a critical cross-origin postMessage bug; (2) the widget must be fetched via `resources/read`, not returned inline in tool responses; (3) the WASM client is complete but not wired into the preview; (4) Playwright tests are written but not yet passing; (5) no `--mcp-apps` scaffolding template exists yet; (6) ChatGPT manifest format is stable and well-documented; (7) shared bridge library reduces the main DX friction point.*
