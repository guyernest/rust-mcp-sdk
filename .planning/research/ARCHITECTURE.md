# Architecture Research: MCP Apps Developer Experience (v1.3)

**Domain:** MCP Apps developer tooling — preview rendering, WASM widget bridge, publishing, authoring DX
**Researched:** 2026-02-24
**Confidence:** HIGH (based on direct codebase analysis of all existing components)

## System Overview

### Current Architecture (v1.2 baseline)

```
+--- pmcp core crate --------------------------------------------------+
|                                                                        |
|  src/types/mcp_apps.rs (behind mcp-apps feature flag)                |
|    WidgetCSP, WidgetMeta, ChatGptToolMeta, WidgetResponseMeta        |
|    ExtendedUIMimeType, UIAction, UIContent, HostType                  |
|    UIMetadata, UIDimensions, ToolVisibility, RemoteDomFramework       |
|                                                                        |
|  src/server/mcp_apps.rs                                               |
|    ChatGptAdapter: transforms HTML, injects skybridge bridge          |
|    UIAdapter trait                                                     |
|                                                                        |
+--- crates/mcp-preview (Axum HTTP server, ~50% complete) ------------+
|                                                                        |
|  PreviewServer + PreviewConfig                                        |
|    port 8765, proxies to mcp_url                                      |
|                                                                        |
|  Handlers:                                                             |
|    GET  /            -> page::index (serves assets/index.html)        |
|    GET  /api/config  -> api::get_config                               |
|    GET  /api/tools   -> api::list_tools (via McpProxy)               |
|    POST /api/tools/call -> api::call_tool (via McpProxy)             |
|    GET  /assets/*    -> assets::serve (rust-embed)                    |
|    GET  /ws          -> websocket::handler (tool calls over WS)       |
|                                                                        |
|  McpProxy:                                                             |
|    HTTP -> MCP JSON-RPC 2.0 (POST /mcp)                              |
|    Methods: initialize, tools/list, tools/call                        |
|    No session persistence (re-initializes each request)               |
|                                                                        |
+--- examples/wasm-client (wasm-bindgen WASM module) -----------------+
|                                                                        |
|  WasmClient: connect(url) -> WebSocket or HTTP transport             |
|    list_tools() -> JsValue                                            |
|    call_tool(name, args) -> JsValue                                   |
|    Uses pmcp transports: WasmWebSocketTransport, WasmHttpClient       |
|                                                                        |
+--- cargo-pmcp (CLI binary) -----------------------------------------+
|                                                                        |
|  preview.rs: cargo pmcp preview --url <mcp-url> --port <port>        |
|    Delegates to mcp_preview::PreviewServer::start()                   |
|                                                                        |
|  new.rs: cargo pmcp new [--tier foundation|domain]                   |
|    No --mcp-apps flag yet                                             |
|                                                                        |
+--- examples/ (standalone MCP App examples) -------------------------+
|                                                                        |
|  mcp-apps-chess/                                                      |
|    src/main.rs:                                                        |
|      ServerBuilder + StreamableHttpServer                             |
|      ChessResources (ResourceHandler): serves board.html via         |
|        ChatGptAdapter.transform() -> text/html+skybridge              |
|      widget/board.html: HTML with window.mcpBridge.callTool()        |
|    preview.html: standalone mock bridge (no real MCP calls)           |
|                                                                        |
|  mcp-apps-map/ (similar structure to chess)                          |
|    preview.html: standalone mock bridge                               |
|                                                                        |
+--- tests/playwright/ (scaffolding placeholder, not yet wired) ------+
```

### Target Architecture (v1.3) — New + Modified Components

```
+--- pmcp core (mcp-apps feature) — MINIMAL CHANGES ------------------+
|                                                                        |
|  src/types/mcp_apps.rs — UNCHANGED                                   |
|  src/server/mcp_apps.rs — MODIFIED                                   |
|    Add: file-based widget loading (include_str! macro helper)         |
|    Add: ChatGPT manifest generation types                             |
|                                                                        |
+--- crates/mcp-preview — MAJOR UPGRADE ------------------------------+
|                                                                        |
|  server.rs — MODIFIED                                                 |
|    Add: widget iframe proxy route GET /widget-proxy?uri=<resource-uri>|
|    Add: resources listing route GET /api/resources                   |
|    Add: resource read route GET /api/resources/read?uri=<uri>        |
|    Add: demo mode (no live MCP, serves standalone preview.html)      |
|                                                                        |
|  proxy.rs — MODIFIED                                                  |
|    Add: resources/list MCP call                                       |
|    Add: resources/read MCP call (returns resource content)           |
|    Add: session persistence (don't re-initialize every request)       |
|    Fix: atomic request ID tracking                                    |
|                                                                        |
|  handlers/ — NEW FILES                                                |
|    widget.rs: GET /widget-proxy                                       |
|      Calls resources/read on MCP server, returns HTML content         |
|      Injects preview-mode mcpBridge JavaScript into HTML             |
|      Bridge routes callTool() through preview /api/tools/call         |
|                                                                        |
|  bridge/ — NEW MODULE                                                 |
|    mod.rs: BridgeInjector                                             |
|      inject_preview_bridge(html: &str, tool_call_url: &str) -> String |
|      Replaces/augments window.mcpBridge with real HTTP proxy bridge   |
|      Supports text/html+mcp and text/html+skybridge MIME types        |
|                                                                        |
|  demo.rs — NEW                                                        |
|    DemoServer: serves preview.html as landing page                    |
|    Reads static preview.html from example directory                   |
|    No live MCP connection required                                    |
|                                                                        |
+--- crates/mcp-bridge-js — NEW CRATE --------------------------------+
|                                                                        |
|  Shared JavaScript bridge library for widgets                         |
|  Built with wasm-pack or as a plain JS file                          |
|  Exposes: window.mcpBridge API                                        |
|    callTool(name, args): Promise<result>                              |
|    getState(): object                                                 |
|    setState(state): void                                              |
|    onBridgeReady(callback): void                                      |
|  Detects environment: ChatGPT (window.openai), preview               |
|  (window.mcpBridge injected), standalone (mock)                      |
|  Distributed as: CDN-hostable .js file embedded in mcp-preview       |
|                                                                        |
+--- examples/wasm-client — MODIFIED ---------------------------------+
|                                                                        |
|  lib.rs — MODIFIED                                                    |
|    Add: list_resources() -> JsValue                                   |
|    Add: read_resource(uri) -> JsValue (returns HTML content)         |
|    Add: render_widget(uri, container_id): integrates iframe +bridge   |
|    Fix: request ID counter (currently hardcoded 1/2/3)               |
|                                                                        |
|  widget-renderer/ — NEW module in wasm-client                        |
|    WidgetRenderer: creates iframe, injects mcpBridge                 |
|    Handles postMessage communication between iframe and bridge        |
|    Implements text/html+mcp protocol                                  |
|                                                                        |
+--- cargo-pmcp — MODIFIED -------------------------------------------+
|                                                                        |
|  commands/new.rs — MODIFIED                                           |
|    Add: --mcp-apps flag                                               |
|    Generates: widgets/ directory, widget HTML template, Rust          |
|      resource handler skeleton with ChatGptAdapter wiring             |
|                                                                        |
|  commands/deploy/ — MODIFIED                                          |
|    Add: widget serving configuration in deployment descriptors        |
|    Add: ChatGPT manifest upload step                                  |
|                                                                        |
|  commands/landing/ — NEW COMMAND                                      |
|    cargo pmcp landing --server <id>                                   |
|    Generates standalone demo HTML page from preview.html              |
|    Bundles mock bridge + widget HTML for stakeholder sharing          |
|                                                                        |
|  commands/manifest/ — NEW COMMAND                                     |
|    cargo pmcp manifest --server <id>                                  |
|    Generates ChatGPT-compatible manifest.json                         |
|    Reads pmcp.toml [mcp-apps] section for metadata                   |
|                                                                        |
+--- tests/playwright/ — NEW TEST INFRASTRUCTURE ---------------------+
|                                                                        |
|  playwright.config.ts: Configure base URL, browser targets            |
|  tests/widget.spec.ts: Widget E2E tests                               |
|  tests/bridge.spec.ts: Bridge injection tests                         |
|  fixtures/: Mock MCP server fixture for testing                       |
|                                                                        |
```

## Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|----------------|-------------------|
| `mcp-preview::McpProxy` | HTTP->MCP JSON-RPC translation | MCP server HTTP endpoint |
| `mcp-preview::BridgeInjector` | Injects JS bridge into widget HTML | None (pure transform) |
| `mcp-preview::handlers::widget` | Fetches resource HTML, injects bridge, serves | McpProxy, BridgeInjector |
| `mcp-bridge-js` | Shared JS bridge library for all widgets | Embedded in mcp-preview assets |
| `WasmClient` | In-browser MCP protocol client | MCP server WS/HTTP |
| `WasmClient::WidgetRenderer` | Creates iframe, manages bridge lifecycle | WasmClient, DOM |
| `cargo-pmcp::ManifestCommand` | Generates ChatGPT manifest.json | pmcp.toml config |
| `cargo-pmcp::new --mcp-apps` | Scaffolds widget project structure | Templates |
| `pmcp::server::mcp_apps` | HTML transform + ChatGPT metadata | Used by app servers |
| `ChessResources` / app resources | Serves widget HTML via MCP resources | pmcp ServerBuilder |

## Recommended Project Structure

```
crates/mcp-preview/
  src/
    server.rs           # PreviewServer, AppState, PreviewConfig (MODIFIED)
    proxy.rs            # McpProxy with session persistence (MODIFIED)
    bridge/             # NEW: Bridge injection logic
      mod.rs            #   BridgeInjector, inject_preview_bridge()
      js/               #   Bundled JS source for the preview bridge
        bridge.js       #   Preview-mode mcpBridge implementation
    handlers/
      page.rs           # Unchanged (serves index.html)
      api.rs            # Add resources endpoints (MODIFIED)
      assets.rs         # Unchanged
      websocket.rs      # Unchanged
      widget.rs         # NEW: /widget-proxy handler
      demo.rs           # NEW: demo landing page handler
    assets/
      index.html        # Preview UI (enhanced with iframe support)
      ...               # Other static assets

crates/mcp-bridge-js/   # NEW CRATE
  src/
    lib.rs              # Rust entry point (minimal, just serves the JS)
  js/
    bridge.ts           # TypeScript source for shared bridge library
    bridge.d.ts         # Type definitions
  dist/
    bridge.js           # Bundled output (committed, not gitignored)

examples/wasm-client/
  src/
    lib.rs              # WasmClient (MODIFIED: add resources, fix IDs)
    widget_renderer.rs  # NEW: WidgetRenderer for iframe lifecycle
    utils.js            # Unchanged

examples/mcp-apps-chess/
  src/
    main.rs             # Server (MODIFIED: use file-based widget)
  widgets/              # NEW: separate widget files
    board.html          # Extracted from inline strings (was widget/board.html)
  preview.html          # Enhanced with real MCP proxy option

examples/mcp-apps-map/
  (same structure as chess)

cargo-pmcp/src/commands/
  new.rs                # MODIFIED: add --mcp-apps flag
  preview.rs            # MODIFIED: add --demo flag
  landing/              # NEW command module
    mod.rs
  manifest/             # NEW command module
    mod.rs

tests/playwright/
  playwright.config.ts
  tests/
    widget.spec.ts
    bridge.spec.ts
  fixtures/
    mock-mcp-server.ts

src/types/mcp_apps.rs   # Minimal additions for manifest types
src/server/mcp_apps.rs  # Add file-based widget macro helpers
```

### Structure Rationale

- **`crates/mcp-bridge-js/` as a separate crate:** The bridge JS is consumed both by `mcp-preview` (embedded as static asset) and potentially by developers for their own widget CDN distribution. Separate crate means the JS can be versioned independently and published to npm or CDN.
- **`bridge/` module inside `mcp-preview`:** Bridge injection logic is preview-server-specific (it points callTool() at the preview server's HTTP endpoint). Not shared externally.
- **`widgets/` directory in examples:** Separating HTML files from Rust source is the key authoring DX improvement. Avoids `include_str!` deep in method bodies.
- **`commands/landing/` and `commands/manifest/` as separate modules:** Follows the existing `commands/secret/mod.rs` pattern — each command is its own module directory with its own `mod.rs`.

## Architectural Patterns

### Pattern 1: Preview Bridge Injection

The core integration challenge: how does a widget's `window.mcpBridge.callTool()` reach the real MCP server during development?

**What:** `BridgeInjector` transforms widget HTML before serving it from `mcp-preview`. It injects a JavaScript shim that replaces `window.mcpBridge` with an implementation that POSTs to `/api/tools/call` on the preview server. The preview server then proxies to the real MCP server.

**When to use:** Every time mcp-preview serves a widget resource via `/widget-proxy`.

**Trade-offs:**
- Pro: Widget HTML works unchanged in both preview and production
- Pro: No CORS issues (same-origin, preview server proxies)
- Pro: Tool call logs appear in the preview dev panel
- Con: HTML injection is fragile if widget has strict CSP; preview must set permissive CSP headers on `/widget-proxy` responses

**Data flow:**
```
Browser iframe (widget HTML)
    |
    | window.mcpBridge.callTool("chess_move", {...})
    |   [injected JS resolves this to HTTP]
    v
Preview Server POST /api/tools/call
    |
    | McpProxy.call_tool()
    v
MCP Server POST /mcp
    |
    | JSON-RPC tools/call response
    v
Preview Server -> iframe -> widget updates
```

**Implementation sketch:**
```rust
// crates/mcp-preview/src/bridge/mod.rs
pub struct BridgeInjector {
    tool_call_url: String, // e.g., "http://localhost:8765/api/tools/call"
}

impl BridgeInjector {
    pub fn inject(&self, html: &str, mime_type: &str) -> String {
        let bridge_script = format!(
            r#"<script>
            window.mcpBridge = {{
                callTool: async (name, args) => {{
                    const res = await fetch("{}", {{
                        method: "POST",
                        headers: {{"Content-Type": "application/json"}},
                        body: JSON.stringify({{name, arguments: args}})
                    }});
                    const data = await res.json();
                    if (!data.success) throw new Error(data.error);
                    return data.content?.[0]?.text
                        ? JSON.parse(data.content[0].text)
                        : data;
                }},
                getState: () => JSON.parse(localStorage.getItem("mcpState") || "{{}}"),
                setState: (s) => localStorage.setItem("mcpState", JSON.stringify(s))
            }};
            window.dispatchEvent(new Event("mcpBridgeReady"));
            </script>"#,
            self.tool_call_url
        );
        // Insert bridge_script before </head> or at start of <body>
        inject_before_body(html, &bridge_script)
    }
}
```

### Pattern 2: Widget Resource Proxy

**What:** A new `GET /widget-proxy?uri=<resource-uri>` endpoint in mcp-preview fetches resource content from the MCP server, injects the bridge, and serves it with appropriate headers for iframe embedding.

**When to use:** When the preview UI wants to render a widget in an iframe.

**Trade-offs:**
- Pro: The iframe src points to the preview server (same-origin context)
- Pro: Enables live resource fetching (not static file serving)
- Pro: Supports MIME type negotiation (returns text/html regardless of original MIME)
- Con: Resources must be text/html or text/html+* — binary resources are not renderable

**Integration with existing code:** The new handler calls `McpProxy::read_resource(uri)` (a new method to add), processes content through `BridgeInjector::inject()`, and returns `text/html` with `X-Frame-Options: SAMEORIGIN`.

### Pattern 3: File-Based Widget Authoring

**What:** Instead of `include_str!("../../widget/board.html")` scattered in handler methods, provide a convention and macro helper for loading widget files.

**When to use:** In every MCP Apps server that has HTML widgets.

**Trade-offs:**
- Pro: Widgets are editable without touching Rust code
- Pro: IDE support for HTML files
- Pro: `cargo pmcp new --mcp-apps` can scaffold the right directory structure
- Con: File path in `include_str!` must be relative to the Rust source file

**Pattern:**
```rust
// Widget file at: widgets/board.html (sibling to src/)
// In src/main.rs:
const BOARD_WIDGET: &str = include_str!("../widgets/board.html");
// OR with a helper macro (new addition to pmcp::server::mcp_apps):
pmcp::widget!("board.html")  // Expands to include_str!("../widgets/board.html")
```

The `pmcp::widget!()` macro is a thin wrapper that:
1. Finds `widgets/` relative to `CARGO_MANIFEST_DIR`
2. Expands to `include_str!` with the correct path
3. Errors at compile time if the file is missing

### Pattern 4: Shared Bridge Library for Widget Authoring

**What:** `mcp-bridge-js` provides a canonical JavaScript file that handles environment detection and bridge lifecycle. Widgets include it as a CDN script tag rather than implementing `window.mcpBridge` consumption themselves.

**When to use:** In all widget HTML files.

**Trade-offs:**
- Pro: Eliminates copy-pasted bridge initialization code from every widget
- Pro: Single place to fix bridge compatibility across ChatGPT/preview/standalone
- Pro: Version-pinned via CDN URL

**Widget usage:**
```html
<!-- Include once, then use window.mcpBridge anywhere -->
<script src="https://cdn.pmcp.run/bridge/0.1.0/bridge.min.js"></script>
<script>
  window.addEventListener('mcpBridgeReady', () => {
    // bridge is ready
    const state = await window.mcpBridge.callTool('chess_new_game', {});
  });
</script>
```

The library internally detects:
1. `window.openai` is present: use ChatGPT Apps API
2. `window.mcpBridge` is already injected by host: use it directly
3. Neither: fall back to postMessage protocol (standard MCP Apps)

### Pattern 5: ChatGPT Manifest Generation

**What:** `cargo pmcp manifest` reads `pmcp.toml [mcp-apps]` section and generates a `manifest.json` (or `.well-known/ai-plugin.json`) compatible with ChatGPT App registration.

**When to use:** During deployment, as part of `cargo pmcp deploy`.

**Trade-offs:**
- Pro: Single source of truth (pmcp.toml) drives both deployment and manifest
- Pro: Avoids hand-editing JSON with correct structure
- Con: Must stay current with OpenAI's manifest schema (which evolves)

**pmcp.toml additions:**
```toml
[mcp-apps]
name = "Chess Game"
description = "Play chess with an interactive board widget"
server_url = "https://chess.pmcp.run"
logo_url = "https://chess.pmcp.run/assets/logo.png"

[[mcp-apps.tools]]
name = "chess_new_game"
widget_uri = "ui://chess/board.html"
invoking = "Setting up the board..."
invoked = "Board ready!"
```

## Data Flows

### Flow 1: Widget Rendering in mcp-preview

```
Developer: opens http://localhost:8765
    |
    v
PreviewServer GET /
  serves index.html (React/vanilla JS preview UI)
    |
    | UI fetches tool list
    v
PreviewServer GET /api/tools -> McpProxy.list_tools() -> MCP Server
    |
    | Developer selects a tool with widget_uri in its metadata
    v
PreviewServer GET /api/resources/read?uri=ui://chess/board.html
    -> McpProxy.read_resource(uri) -> MCP Server resources/read
    -> BridgeInjector.inject(html, "text/html+skybridge")
    -> Returns HTML with injected mcpBridge shim
    |
    | Preview UI loads iframe with widget HTML
    v
iframe: widget/board.html renders
    |
    | User clicks "New Game"
    v
iframe: window.mcpBridge.callTool("chess_new_game", {})
    -> fetch POST http://localhost:8765/api/tools/call
    |
    v
PreviewServer POST /api/tools/call
    -> McpProxy.call_tool("chess_new_game", {})
    -> MCP Server tools/call
    -> Returns game state
    |
    v
iframe: widget updates board display
Preview dev panel: logs tool call + result
```

### Flow 2: WASM Test Client Widget Rendering

```
Browser page (wasm-client context, e.g. mcp-preview index.html)
    |
    | WasmClient.connect("http://localhost:3000")
    v
pmcp HTTP transport: initialize session
    |
    | WasmClient.list_resources()
    v
pmcp JSON-RPC: resources/list -> returns [{ uri: "ui://chess/board.html", ... }]
    |
    | WasmClient::WidgetRenderer.render(uri, container_element)
    v
WidgetRenderer:
  1. WasmClient.read_resource(uri) -> HTML content
  2. Creates <iframe> in container
  3. srcdoc = BridgeInjector.inject(html) [injects postMessage bridge]
  4. Listens for postMessage from iframe
    |
    | Widget iframe posts: { type: "callTool", name: "...", args: {...} }
    v
WidgetRenderer:
  5. WasmClient.call_tool(name, args) -> result
  6. Posts result back to iframe: { type: "toolResult", result: {...} }
    |
    v
Widget iframe: handles result, updates UI
```

### Flow 3: Cargo pmcp new --mcp-apps Scaffolding

```
cargo pmcp new my-widget-server --mcp-apps
    |
    v
new.rs::execute(name="my-widget-server", mcp_apps=true)
    |
    v
templates::mcp_apps::generate(workspace_dir, name)
    Creates:
      my-widget-server/
        Cargo.toml        # pmcp with mcp-apps feature
        pmcp.toml         # [mcp-apps] section stub
        src/
          main.rs         # ServerBuilder + StreamableHttpServer + ResourceHandler
        widgets/
          main.html       # Widget HTML template using mcp-bridge-js CDN
    |
    v
Prints next steps:
  1. cd my-widget-server
  2. Edit widgets/main.html with your UI
  3. cargo run  (starts MCP server on :3000)
  4. cargo pmcp preview  (opens widget preview on :8765)
```

### Flow 4: Demo Landing Page Generation

```
cargo pmcp landing --server chess --output ./dist
    |
    v
LandingCommand::execute()
    |
    | Reads: examples/mcp-apps-chess/preview.html (or equivalent)
    | Reads: pmcp.toml for title/description
    v
Inlines all assets:
  - Widget HTML (base64 or inline)
  - Mock bridge JavaScript
  - CSS
    |
    v
Writes: ./dist/index.html
  Self-contained, no server required
  Suitable for GitHub Pages / S3 static hosting
    |
    v
Prints: "Landing page generated at ./dist/index.html"
```

### Flow 5: ChatGPT Manifest Generation

```
cargo pmcp manifest --server chess --output ./dist
    |
    v
ManifestCommand::execute()
    |
    | Reads: pmcp.toml [mcp-apps] section
    | Reads: MCP server tools/list (optional, for tool metadata)
    v
Generates:
  ./dist/.well-known/ai-plugin.json:
    { name, description, api.url, logo_url, ... }
    tools: [{ name, description, output_template, ... }]
    |
    v
Optionally uploads to MCP server's deployment target
```

## Integration Points: New vs. Modified vs. Unchanged

### New Components

| Component | Location | Integrates With | Purpose |
|-----------|----------|----------------|---------|
| `BridgeInjector` | `crates/mcp-preview/src/bridge/mod.rs` | `handlers/widget.rs` | HTML injection of preview bridge |
| `handlers::widget` | `crates/mcp-preview/src/handlers/widget.rs` | `McpProxy`, `BridgeInjector` | `/widget-proxy` endpoint |
| `handlers::demo` | `crates/mcp-preview/src/handlers/demo.rs` | Static assets | Demo landing page serving |
| `mcp-bridge-js` crate | `crates/mcp-bridge-js/` | `mcp-preview` assets, widget HTML | Shared JS bridge library |
| `WasmClient::WidgetRenderer` | `examples/wasm-client/src/widget_renderer.rs` | `WasmClient`, DOM | In-browser widget lifecycle |
| `pmcp::widget!` macro | `src/server/mcp_apps.rs` | App server `main.rs` files | File-based widget loading |
| `cargo pmcp landing` | `cargo-pmcp/src/commands/landing/mod.rs` | `pmcp.toml`, example HTML | Standalone demo generation |
| `cargo pmcp manifest` | `cargo-pmcp/src/commands/manifest/mod.rs` | `pmcp.toml`, MCP server | ChatGPT manifest generation |
| Playwright tests | `tests/playwright/` | Preview server, example apps | E2E widget testing |

### Modified Components

| Component | File | Change | Impact |
|-----------|------|--------|--------|
| `McpProxy` | `crates/mcp-preview/src/proxy.rs` | Add `list_resources()`, `read_resource()`, session persistence, proper ID counter | Requires adding `resources` module methods |
| `PreviewServer` | `crates/mcp-preview/src/server.rs` | Add `/widget-proxy`, `/api/resources`, `/api/resources/read` routes; demo mode | New Axum routes, no breaking changes |
| `api.rs` | `crates/mcp-preview/src/handlers/api.rs` | Add resource list/read endpoints | Additional handler functions |
| `WasmClient` | `examples/wasm-client/src/lib.rs` | Add `list_resources()`, `read_resource()`, fix request ID atomics | Additive only |
| `cargo pmcp new` | `cargo-pmcp/src/commands/new.rs` | Add `--mcp-apps` flag + template | New flag, existing behavior unchanged |
| `cargo pmcp preview` | `cargo-pmcp/src/commands/preview.rs` | Add `--demo` flag | New flag, existing behavior unchanged |
| `src/server/mcp_apps.rs` | Core | Add `widget!` macro, manifest types | Additive under `mcp-apps` feature |

### Unchanged Components

| Component | Why Unchanged |
|-----------|---------------|
| `pmcp::types::mcp_apps` | All types ship and work; no protocol changes needed |
| `ChatGptAdapter` | Bridge injection for ChatGPT already correct; preview uses its own injector |
| `ServerBuilder` / `StreamableHttpServer` | No new hooks needed at server level |
| `TaskRouter`, `pmcp-tasks` | MCP Apps is orthogonal to task management |
| `cargo pmcp deploy` | Add manifest step without touching existing deploy paths |
| `handlers::page`, `handlers::assets`, `handlers::websocket` | Still valid as-is |
| `mcp-apps-chess`, `mcp-apps-map` source | Widget pattern works; only file organization changes |

## Build Order (Dependency-Aware)

### Step 1: Preview Bridge Infrastructure (~3 days)

**Deliver:** `McpProxy.read_resource()` + `handlers::widget` + `BridgeInjector`

**Why first:** Everything else in the preview story depends on being able to render a widget in an iframe with a working bridge. This is the core integration gap.

**Implementation sequence within step:**
1. Add `resources/list` + `resources/read` to `McpProxy` (~50 LOC)
2. Add `/api/resources` and `/api/resources/read` routes to `PreviewServer`
3. Write `BridgeInjector::inject()` — HTML string manipulation, pure function, easily testable
4. Write `handlers::widget` — fetch resource, inject, serve

**Validation:** Start chess server, run `cargo pmcp preview`, navigate to widget — it should render and tool calls should work.

### Step 2: WASM Widget Renderer (~2 days)

**Deliver:** `WasmClient::WidgetRenderer` with postMessage bridge

**Depends on:** Step 1 (validates the bridge protocol being implemented)

**Why second:** The WASM client is the in-browser alternative to the preview server proxy. It needs to implement the same bridge protocol that Step 1 establishes.

**Implementation:** `widget_renderer.rs` — iframe creation, `srcdoc` injection, postMessage listener, call routing through `WasmClient`.

**Validation:** Load wasm-client test page, render chess widget, play a move.

### Step 3: Shared Bridge Library (`mcp-bridge-js`) (~2 days)

**Deliver:** A versioned `bridge.js` CDN artifact embedded in `mcp-preview`

**Depends on:** Steps 1 and 2 (defines the API contract both injectors must implement)

**Why third:** Once the bridge API is validated in Steps 1 and 2, extract into a shared library. Update existing chess/map widgets to use it.

**Implementation:** Pure JavaScript/TypeScript. No Rust changes. Build with esbuild or tsc. Embed in `mcp-preview::Assets`.

### Step 4: File-Based Widget Authoring + `cargo pmcp new --mcp-apps` (~2 days)

**Deliver:** `pmcp::widget!()` macro + scaffolding template + updated chess/map examples

**Depends on:** Step 3 (template must reference the bridge library from step 3)

**Why fourth:** DX improvement that makes widget authoring pleasant. Refactoring chess/map to use `widgets/` directory structure validates the pattern.

**Implementation:**
1. Add `pmcp::widget!()` macro in `src/server/mcp_apps.rs`
2. Move chess/map HTML files to `widgets/` directory
3. Update `new.rs` with `--mcp-apps` flag and template
4. Write template files for `widgets/main.html` and `src/main.rs` skeleton

### Step 5: Demo Landing Page + ChatGPT Manifest (~2 days)

**Deliver:** `cargo pmcp landing` + `cargo pmcp manifest`

**Depends on:** Step 4 (demo page must work with the file-based widget structure)

**Why fifth:** Publishing tooling. Uses the validated widget artifacts from Steps 1-4.

**Implementation:**
1. `commands/landing/mod.rs`: HTML inlining + mock bridge bundling
2. `commands/manifest/mod.rs`: pmcp.toml parsing + JSON generation

### Step 6: Ship Examples + Playwright Tests (~2 days)

**Deliver:** Chess and map examples in final form + Playwright E2E test suite

**Depends on:** All previous steps

**Why last:** Integration validation. Examples exercise every piece of the toolchain.

**Implementation:**
1. Final chess/map example polish (README, clear comments)
2. Playwright configuration and widget tests
3. CI integration for Playwright tests

## Anti-Patterns

### Anti-Pattern 1: Re-Initializing MCP Session Per Request

**What people do:** Call `McpProxy::initialize()` before every `list_tools()` or `call_tool()` (current behavior in proxy.rs lines 148-151).

**Why it's wrong:** MCP session initialization is a handshake that establishes capabilities. Re-doing it per request is wasteful and breaks session-based MCP servers (ones that use session ID from initialize response for auth).

**Do this instead:** Cache the initialized session state in `McpProxy`. Only call `initialize()` once on construction or first use. Track `session_id` from the response and send it in subsequent requests.

```rust
pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session_id: tokio::sync::RwLock<Option<String>>, // NEW
}
```

### Anti-Pattern 2: Hardcoded Request IDs in WasmClient

**What people do:** Use literal integers (1, 2, 3) as JSON-RPC request IDs in `wasm-client/src/lib.rs` (current code at lines 115, 189, 245).

**Why it's wrong:** Concurrent calls will collide IDs. The MCP server may reject or misroute responses. When `WidgetRenderer` is added, many simultaneous bridge calls will break.

**Do this instead:** Use an `AtomicU64` counter in `WasmClient`, same as `McpProxy` already does. This is already done correctly in the Rust server side; port the pattern to the WASM client.

### Anti-Pattern 3: Inline HTML as Rust String Literals

**What people do:** Embed hundreds of lines of HTML inside `const WIDGET_HTML: &str = r#"..."#` or `include_str!` buried inside handler methods.

**Why it's wrong:** No HTML syntax highlighting, no IDE support, no ability to open as a webpage for standalone development. Refactoring means changing Rust files.

**Do this instead:** Use the `widgets/` directory convention with `pmcp::widget!("board.html")` or `include_str!("../widgets/board.html")` at module level. Widget files are standalone HTML files that work both with the preview bridge and in isolation.

### Anti-Pattern 4: iframe srcdoc for Large Widgets

**What people do:** Inject widget HTML via `<iframe srcdoc="...">` for all widget sizes.

**Why it's wrong:** `srcdoc` attribute has browser-specific size limits (varies, but often ~64KB). Large widget HTML with embedded game assets will fail silently.

**Do this instead:** For preview rendering, use the `/widget-proxy?uri=...` URL as `iframe src`. Only use `srcdoc` in the WASM renderer where there is no server to proxy through, and document the size constraint.

### Anti-Pattern 5: Separate Bridge Implementations Without Shared Contract

**What people do:** Write three different `mcpBridge` JavaScript implementations: one injected by `BridgeInjector`, one in `preview.html` mock, one in `WidgetRenderer`. Each drifts independently.

**Why it's wrong:** Widgets that work in one context fail in another. Adding a new bridge method (e.g., `mcpBridge.listTools()`) requires updating all three implementations.

**Do this instead:** `mcp-bridge-js` is the single canonical implementation. Preview uses it (injecting the real HTTP transport). Mock `preview.html` is a thin overriding layer. WidgetRenderer uses the same JS but wires it to postMessage transport.

### Anti-Pattern 6: Serving Widget HTML with Strict CSP from mcp-preview

**What people do:** Let the default Axum response headers flow through to iframe-rendered widget HTML, including any CSP that blocks the injected `<script>` tag.

**Why it's wrong:** The injected bridge script is inline JavaScript. If the MCP server's original resource response included `Content-Security-Policy: script-src 'self'`, the injected bridge will be blocked.

**Do this instead:** The `/widget-proxy` handler overrides CSP to `Content-Security-Policy: script-src 'unsafe-inline' 'self'` for preview mode only. Document that this is intentional for development. Production widgets should not rely on `unsafe-inline`.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single developer, local | `cargo pmcp preview` running locally against local MCP server — all current defaults work |
| Team demo / stakeholder review | `cargo pmcp landing` generates static page, host on GitHub Pages — no server needed |
| Published ChatGPT App | Deploy MCP server, run `cargo pmcp manifest`, register at platform.openai.com |
| Multiple widgets / large apps | `widgets/` directory structure scales naturally; one HTML file per widget |

### First Bottleneck: McpProxy Session Affinity

Preview server runs as a single instance pointing at one MCP server. When that MCP server uses session-based state (via `mcp-session-id` header), the preview proxy must forward the session ID on all requests after initialization. Current `McpProxy` loses this. Fix is session caching in `McpProxy` (Step 1 of build order).

### Second Bottleneck: WASM Bundle Size

`wasm-client` currently pulls in the full `pmcp` crate for WASM. As widgets grow, the WASM bundle will grow. Mitigation: ensure `pmcp` has minimal WASM-only features; profile with `twiggy` or `wasm-opt`. Not an immediate concern for v1.3 scope.

## Sources

### Codebase Analysis (PRIMARY — HIGH confidence)

- `crates/mcp-preview/src/server.rs` — PreviewServer, AppState, route definitions
- `crates/mcp-preview/src/proxy.rs` — McpProxy: HTTP->JSON-RPC translation, current gaps (session, IDs)
- `crates/mcp-preview/src/handlers/page.rs` — Minimal handler, serves static HTML
- `crates/mcp-preview/src/handlers/api.rs` — tool list/call handlers over HTTP
- `crates/mcp-preview/src/handlers/websocket.rs` — WsMessage, tool call over WebSocket
- `crates/mcp-preview/src/handlers/assets.rs` — rust-embed static asset serving
- `examples/wasm-client/src/lib.rs` — WasmClient: WebSocket + HTTP transports, hardcoded IDs
- `examples/mcp-apps-chess/src/main.rs` — Full app example: ResourceHandler, ChatGptAdapter, StreamableHttpServer
- `examples/mcp-apps-chess/preview.html` — Mock bridge pattern, dev toolbar, iframe rendering
- `src/types/mcp_apps.rs` — All MCP Apps types: WidgetCSP, ChatGptToolMeta, UIAction, ExtendedUIMimeType, HostType
- `cargo-pmcp/src/commands/preview.rs` — CLI entry point for preview
- `cargo-pmcp/src/commands/new.rs` — Scaffolding pattern (tier-based)
- `cargo-pmcp/src/commands/secret/mod.rs` — CLI command module pattern to follow
- `.planning/PROJECT.md` — v1.3 milestone definition, out-of-scope items

### MCP Protocol (MEDIUM confidence — verified against existing codebase implementation)

- MCP `resources/read` method: used by ChessResources handler, same pattern as `tools/call` in McpProxy
- MCP `resources/list` method: listed in ResourceHandler trait but not in McpProxy yet

---
*Architecture research for: MCP Apps Developer Experience (v1.3) in PMCP SDK*
*Researched: 2026-02-24*
