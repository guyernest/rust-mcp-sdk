# Phase 15: WASM Widget Bridge - Research

**Researched:** 2026-02-24
**Domain:** WASM MCP client in browser iframe, bridge adapter, wasm-pack build automation, widget-runtime.js bundling
**Confidence:** HIGH

## Summary

Phase 15 adds an in-browser WASM MCP client as an alternative bridge mode in the preview UI. The existing `examples/wasm-client/` crate provides a fully working `WasmClient` that handles `connect()`, `list_tools()`, and `call_tool()` over both HTTP and WebSocket transports using `wasm-bindgen` and the browser's Fetch API. The `pmcp` SDK already has CORS headers built into `StreamableHttpServer` (`Access-Control-Allow-Origin: *` with the full set of MCP-specific exposed headers), so the WASM client running in an iframe can talk directly to any PMCP server over HTTP without a proxy. The main work is: (1) fix the hardcoded request IDs in the WASM client that corrupt concurrent calls, (2) build a JavaScript bridge adapter that translates `WasmClient.call_tool()` responses into the `window.mcpBridge.callTool()` shape established in Phase 14, (3) add a Proxy/WASM toggle button in the preview header that destroys/recreates the iframe with the selected bridge injected, (4) automate `wasm-pack build` when the developer first toggles to WASM mode, and (5) bundle the WASM client and adapter as a standalone `widget-runtime.js`.

The existing WASM binary is 711KB (uncompressed). With gzip (typical for HTTP), this drops to ~200-250KB. The `wasm-bindgen` generated JS glue is 33KB. For the standalone `widget-runtime.js` bundle, the recommended approach is a two-file strategy: a JS loader + the `.wasm` binary fetched at runtime. This avoids the 33% base64 inflation penalty and works with `WebAssembly.instantiateStreaming()` for optimal load performance. The `initSync` approach (inlined base64) is available as a fallback but increases the bundle from ~750KB to ~1MB and blocks the main thread during compilation.

**Primary recommendation:** Use the existing `examples/wasm-client/` as the foundation. Fix the hardcoded request IDs with an `AtomicU64` counter. Write a thin JS adapter layer that wraps `WasmClient` to expose the `window.mcpBridge` contract. Add a toggle button to the preview header. Automate `wasm-pack build --target web` from the preview server. For `widget-runtime.js`, use two-file delivery (JS + `.wasm`) with a `data-mcp-url` attribute for server URL injection.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Toggle button in the preview UI header: "Proxy" / "WASM" -- developer clicks to swap modes
- Default mode is Proxy (simpler, faster startup)
- Toggling triggers a full widget reload -- iframe is destroyed and recreated with the selected bridge injected
- The toggle button itself shows which mode is active (highlighted state) -- no separate label needed
- Strategic context: Rust-first with PMCP SDK is the primary use case; wasm-pack and the wasm32 target are acceptable prerequisites. However, architect so the WASM binary could be pre-built and distributed separately later (for non-Rust developers using dev/test tools)
- Auto-build on toggle: when developer toggles to WASM mode, preview server runs wasm-pack build if needed, caches the output
- WASM artifacts cached (Claude's discretion on location)
- First toggle may be slow (WASM compilation); subsequent toggles use cached artifacts
- Developer needs wasm-pack and the wasm32 target installed (acceptable for Rust-first use case)
- API surface matches Phase 14 proxy bridge exactly -- drop-in replacement: callTool, getState, setState, sendMessage, openExternal, theme/locale/displayMode getters
- Auto-connect on script load -- reads server URL and connects immediately, no manual init code needed
- Bundle approach at Claude's discretion (single file with inlined WASM vs JS + separate .wasm fetch)
- Server URL specification at Claude's discretion (data attribute vs init call -- pick simplest for widget authors)
- HTTP (Streamable HTTP) transport for the WASM bridge -- matches production MCP server patterns
- CORS handling at Claude's discretion (investigate existing SDK CORS support in StreamableHttpServer)
- Non-CORS fallback at Claude's discretion (Rust-first strategy means PMCP servers have CORS by default)
- Bridge calls in devtools look identical regardless of mode -- transparent, no [WASM]/[Proxy] tags
- Existing `examples/wasm-client/` has a working `WasmClient` with HTTP and WebSocket transports -- use it as the foundation, don't rewrite
- The `WasmClient` already handles `connect()`, `list_tools()`, `call_tool()`, and session management
- `wasm-bindgen` and `serde-wasm-bindgen` are already dependencies
- Widget authors should be able to include `widget-runtime.js` with a single `<script>` tag and have `window.mcpBridge` ready immediately

### Claude's Discretion
- WASM artifact cache location (target/wasm-bridge/ or user-global cache)
- widget-runtime.js bundle approach (inlined WASM vs separate .wasm file)
- Server URL injection method for standalone widget-runtime.js
- CORS handling strategy (SDK built-in vs preview proxy fallback)
- Non-CORS server fallback approach
- wasm-pack build configuration and flags
- Error handling when wasm-pack is not installed

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WASM-01 | WASM MCP client loads in preview iframe context and connects to MCP server | Existing `WasmClient` from `examples/wasm-client/` connects via HTTP transport using browser Fetch API; needs request ID fix and integration into preview iframe's bridge injection |
| WASM-02 | Bridge adapter translates WASM `call_tool()` response to `window.mcpBridge.callTool()` shape | Thin JS adapter wrapping `WasmClient` -- translates WASM response format (MCP `CallToolResult` with `content[]` array) to same shape proxy bridge returns; same-origin `srcdoc` iframe means direct `window.parent.previewRuntime` access |
| WASM-03 | WASM client handles CORS for cross-origin HTTP transport to local MCP server | `StreamableHttpServer` already adds `Access-Control-Allow-Origin: *` and exposes `mcp-session-id`, `mcp-protocol-version` headers; CORS is handled at the SDK level with no additional work needed |
| WASM-04 | MCP server URL is injected into WASM client from preview server configuration | Preview server already has `config.mcp_url`; inject via `data-mcp-url` attribute on script tag or via `window.__mcpConfig.serverUrl` global set by the bridge injection wrapper |
| WASM-05 | Standalone `widget-runtime.js` bundles WASM client as drop-in `window.mcpBridge` polyfill | Two-file delivery: JS loader (~35KB) + `.wasm` binary (~711KB, ~250KB gzipped); JS reads `data-mcp-url` from its own script tag, calls `init()` then `WasmClient.connect(url)`, exposes `window.mcpBridge` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasm-bindgen | 0.2.x | Rust-to-JS FFI bindings | Already used in `examples/wasm-client/`; generates JS glue code and `.wasm` binary |
| wasm-bindgen-futures | 0.4.x | Async Rust futures to JS Promises | Already a dependency; enables `async fn` methods on `WasmClient` |
| serde-wasm-bindgen | 0.6.x | Serde to/from JsValue conversion | Already a dependency; handles `Value <-> JsValue` for tool arguments and results |
| web-sys | 0.3.x | Web API bindings (Fetch, Headers, WebSocket) | Already a dependency; provides `fetch()` and `Response` types for HTTP transport |
| wasm-pack | 0.13+ | Build tool: Rust to WASM with JS bindings | Standard Rust-WASM toolchain; generates `--target web` output with ES module init function |
| console_error_panic_hook | 0.1.x | Route Rust panics to browser console | Already a dependency; critical for debugging WASM client issues |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rust-embed | (existing) | Embed static assets in binary | Already used in `mcp-preview` for `assets/` directory; serves WASM artifacts at runtime |
| axum | (existing) | HTTP server framework | Already used in `mcp-preview`; add route for WASM build trigger and artifact serving |
| tokio::process | (existing) | Async subprocess execution | Run `wasm-pack build` as child process when developer toggles to WASM mode |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Two-file delivery (JS + .wasm) | Base64-inlined single .js file | Single file is simpler to distribute but ~33% larger (~950KB vs ~750KB); blocks main thread with `initSync`; use two-file for performance, single-file as optional build |
| `data-mcp-url` attribute | `window.__mcpConfig` global | Data attribute is self-contained per script tag; global requires coordination; recommend data attribute for standalone, global for preview injection |
| `wasm-pack build --target web` | `wasm-pack build --target no-modules` | `--target web` produces ES module with `init()` and `initSync()` exports; `--target no-modules` produces IIFE but lacks streaming compilation; web target is standard |

## Architecture Patterns

### Recommended Project Structure
```
crates/mcp-preview/
  src/
    handlers/
      api.rs          # Add /api/wasm/build and /api/wasm/status endpoints
      wasm.rs         # NEW: WASM build trigger and artifact serving
    wasm_builder.rs   # NEW: wasm-pack build orchestration and caching
    server.rs         # Add toggle state, WASM routes
  assets/
    index.html        # Add toggle button, WASM bridge injection variant

examples/wasm-client/
  src/lib.rs          # Fix hardcoded request IDs (AtomicU64 counter)
  Cargo.toml          # No changes needed

# Build output (not checked in):
target/wasm-bridge/   # Cached WASM build artifacts
  pkg/
    mcp_wasm_client_bg.wasm
    mcp_wasm_client.js
    snippets/
```

### Pattern 1: Bridge Adapter Layer
**What:** A thin JavaScript layer that wraps the WASM `WasmClient` to expose the identical `window.mcpBridge` API as the proxy bridge.
**When to use:** When the WASM client's response format differs from the bridge contract.
**Why needed:** The proxy bridge returns `{ success: true, content: [...] }` from its `/api/tools/call` HTTP endpoint. The WASM `call_tool()` returns the raw MCP `CallToolResult` shape (which has `content[]` and `isError` but not `success`). The adapter normalizes this.
**Example:**
```javascript
// Bridge adapter: wraps WasmClient to match window.mcpBridge contract
class WasmBridgeAdapter {
  constructor(wasmClient, previewRuntime) {
    this.client = wasmClient;
    this.preview = previewRuntime;
  }

  async callTool(name, args) {
    const startTime = Date.now();
    let result, success = true;
    try {
      // WasmClient.call_tool returns MCP CallToolResult shape
      const mcpResult = await this.client.call_tool(name, args || {});
      // Normalize to proxy bridge shape
      result = {
        success: !mcpResult.isError,
        content: mcpResult.content || [],
        _meta: mcpResult._meta || null
      };
      success = result.success;
    } catch (e) {
      result = { success: false, error: e.message };
      success = false;
    }
    const duration = Date.now() - startTime;
    this.preview.logBridgeCall(name, args || {}, result, duration, success);
    return result;
  }

  getState()  { return this.preview.widgetState; }
  setState(s) { this.preview.widgetState = { ...this.preview.widgetState, ...s }; this.preview.updateStateView(); }
  // ... remaining methods mirror proxy bridge exactly
}
```

### Pattern 2: Toggle-Triggered Iframe Rebuild
**What:** Toggling Proxy/WASM destroys the current iframe and creates a new one with the selected bridge injected via `srcdoc`.
**When to use:** On every toggle click.
**Why:** The bridge is injected at iframe creation time (in `wrapWidgetHtml()`). Swapping bridge modes requires re-injecting the bridge code. A full iframe rebuild is the cleanest approach -- avoids stale state from the previous bridge.
**Example:**
```javascript
// In PreviewRuntime:
async toggleBridgeMode(mode) {
  this.bridgeMode = mode; // 'proxy' or 'wasm'
  if (mode === 'wasm' && !this.wasmReady) {
    // Trigger build if needed
    await this.ensureWasmBuilt();
  }
  // Reload current resource widget with new bridge
  if (this.activeResourceUri) {
    await this.loadResourceWidget(this.activeResourceUri);
  }
}

wrapWidgetHtml(html) {
  if (this.bridgeMode === 'wasm') {
    return this.wrapWidgetHtmlWasm(html);
  }
  return this.wrapWidgetHtmlProxy(html); // existing implementation
}
```

### Pattern 3: Lazy WASM Build with Cache
**What:** Preview server runs `wasm-pack build` on first WASM toggle, caches output.
**When to use:** Developer clicks WASM toggle for the first time.
**Why:** WASM compilation is slow (10-30s first time). Caching avoids repeated builds. The preview server checks for cached artifacts before building.
**Example (Rust):**
```rust
// In wasm_builder.rs
pub struct WasmBuilder {
    cache_dir: PathBuf,    // target/wasm-bridge/pkg/
    source_dir: PathBuf,   // examples/wasm-client/ (or embedded)
    build_status: RwLock<BuildStatus>,
}

enum BuildStatus {
    NotBuilt,
    Building,
    Ready(PathBuf), // Path to pkg/ directory
    Failed(String),
}

impl WasmBuilder {
    pub async fn ensure_built(&self) -> Result<PathBuf> {
        // Fast path: already built
        if let BuildStatus::Ready(path) = &*self.build_status.read().await {
            return Ok(path.clone());
        }
        // Slow path: build
        self.build().await
    }

    async fn build(&self) -> Result<PathBuf> {
        // Run: wasm-pack build --target web --out-dir <cache_dir>/pkg --no-opt
        let output = Command::new("wasm-pack")
            .args(["build", "--target", "web", "--out-dir"])
            .arg(self.cache_dir.join("pkg").to_str().unwrap())
            .arg("--no-opt")
            .current_dir(&self.source_dir)
            .output()
            .await?;
        // ...
    }
}
```

### Pattern 4: Standalone widget-runtime.js
**What:** A self-contained JS file that loads the WASM client, connects to a server, and exposes `window.mcpBridge`.
**When to use:** Outside the preview context -- widget authors including bridge in their standalone HTML.
**Example:**
```html
<!-- Widget author's HTML -->
<script src="widget-runtime.js" data-mcp-url="http://localhost:3000/mcp"></script>
<script>
  // window.mcpBridge is available after the script loads and connects
  window.addEventListener('mcpBridgeReady', async () => {
    const result = await window.mcpBridge.callTool('my_tool', { arg: 'value' });
  });
</script>
```

### Anti-Patterns to Avoid
- **Re-implementing the MCP client in JavaScript:** The WASM client already handles MCP protocol correctly. Writing a JS MCP client duplicates effort and diverges from the Rust SDK.
- **Using postMessage for WASM bridge calls:** The preview iframe uses `srcdoc` (same origin), so direct `window.parent` access works. postMessage adds complexity and async overhead for no security benefit in this context.
- **Embedding WASM in index.html via base64:** The index.html is served by rust-embed. Embedding 950KB of base64 WASM would bloat every page load even when WASM mode is not used. Serve WASM artifacts only on demand.
- **Hardcoded request IDs:** The current WASM client uses `id: 1i64`, `id: 2i64`, `id: 3i64` for init, list_tools, and call_tool respectively. This means two concurrent `call_tool` calls get the same ID, causing response routing corruption. Use `AtomicU64`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rust to WASM compilation | Custom cargo/rustc invocation | `wasm-pack build --target web` | Handles wasm-bindgen CLI version matching, JS glue generation, TypeScript declarations |
| CORS headers on MCP server | Custom CORS middleware | Built-in `add_cors_headers()` in `StreamableHttpServer` | Already adds `Access-Control-Allow-Origin: *` with all MCP-specific headers exposed |
| MCP protocol implementation | JavaScript MCP client | `WasmClient` (Rust compiled to WASM) | Reuses the full PMCP SDK protocol implementation; stays in sync with SDK updates |
| JS-Rust FFI serialization | Manual JSON string passing | `serde-wasm-bindgen` | Handles complex types, nested objects, Options, enums without manual marshalling |
| WASM loading and initialization | Custom fetch + compile | `wasm-bindgen` generated `init()` function | Handles streaming compilation, memory initialization, import wiring |

**Key insight:** The existing `examples/wasm-client/` crate already solves the hard problems (MCP protocol, transport, session management, error handling). The remaining work is glue code: (1) a JS adapter to match the bridge API contract, (2) a Rust module to automate the build, and (3) UI changes to the preview page.

## Common Pitfalls

### Pitfall 1: Hardcoded Request IDs Corrupt Concurrent Calls
**What goes wrong:** Two simultaneous `call_tool()` calls both use `id: 3i64`. The server sends two responses with `id: 3`. The client cannot match responses to the correct caller.
**Why it happens:** The existing WASM client in `examples/wasm-client/src/lib.rs` uses hardcoded integer literals for request IDs instead of an incrementing counter.
**How to avoid:** Add an `AtomicU64` request ID counter to `WasmClient`. Increment on each `call_tool()` / `list_tools()` call. This is explicitly flagged in STATE.md as a known issue.
**Warning signs:** Tool call results appearing in the wrong callback; intermittent "invalid response" errors when making rapid calls.

### Pitfall 2: WASM Binary Served Without Correct MIME Type
**What goes wrong:** Browser refuses to compile WASM via `WebAssembly.instantiateStreaming()` and falls back to slower `WebAssembly.instantiate()`.
**Why it happens:** The preview server's asset handler uses `mime_guess` which correctly maps `.wasm` to `application/wasm`. However, if WASM artifacts are served from a different route (e.g., a generic file handler), the MIME type might be `application/octet-stream`.
**How to avoid:** Ensure the WASM artifact route explicitly sets `Content-Type: application/wasm`. The existing `handlers/assets.rs` using `mime_guess::from_path` handles this correctly if the file has a `.wasm` extension.
**Warning signs:** Console warning: "`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type."

### Pitfall 3: WASM Module Size Blocks Main Thread
**What goes wrong:** Using `initSync()` with a 711KB WASM module blocks the browser main thread for 100-500ms, causing a visible UI freeze.
**Why it happens:** `initSync()` calls `WebAssembly.Module()` and `WebAssembly.Instance()` synchronously. This is fine for small modules but problematic at 700KB+.
**How to avoid:** Always use the async `init()` function (which uses `WebAssembly.instantiateStreaming()`) for the preview and standalone runtime. Reserve `initSync()` only for edge cases where async is impossible.
**Warning signs:** Widget area appears frozen briefly when switching to WASM mode.

### Pitfall 4: wasm-pack Not Installed
**What goes wrong:** Developer toggles to WASM mode, build fails silently, widget area shows a cryptic error.
**Why it happens:** `wasm-pack` and the `wasm32-unknown-unknown` target are not installed by default.
**How to avoid:** Check for `wasm-pack` availability before attempting build. Show a clear error message: "WASM mode requires wasm-pack. Install with: cargo install wasm-pack && rustup target add wasm32-unknown-unknown". The toggle button should show a loading/error state during build.
**Warning signs:** Build process exits with non-zero status; `wasm-pack` command not found.

### Pitfall 5: Bridge API Divergence Between Proxy and WASM Modes
**What goes wrong:** Widget code works in Proxy mode but fails in WASM mode (or vice versa) because the response shapes differ.
**Why it happens:** The proxy bridge returns `{ success: true, content: [...] }` while the WASM `call_tool()` returns the raw MCP `CallToolResult` (which has `content[]` and `isError` but uses different field names).
**How to avoid:** The bridge adapter must normalize WASM responses to exactly match the proxy bridge's response shape. Write tests that exercise the same widget code against both bridge modes.
**Warning signs:** Widget displays "undefined" for fields that exist in one mode but not the other.

### Pitfall 6: First WASM Build Takes 10-30 Seconds
**What goes wrong:** Developer toggles to WASM mode and assumes the UI is broken because nothing happens for a long time.
**Why it happens:** `wasm-pack build` compiles the full WASM client from Rust source. First build downloads/compiles all dependencies.
**How to avoid:** Show a clear "Building WASM client..." progress indicator when the build is triggered. Disable the toggle during build. Cache the output so subsequent toggles are instant.
**Warning signs:** Toggle button appears stuck; no feedback during build.

## Code Examples

### Fix Hardcoded Request IDs
```rust
// Source: examples/wasm-client/src/lib.rs (modification)
use std::sync::atomic::{AtomicU64, Ordering};

#[wasm_bindgen]
pub struct WasmClient {
    connection_type: Option<ConnectionType>,
    ws_client: Option<Client<WasmWebSocketTransport>>,
    http_client: Option<WasmHttpClient>,
    next_request_id: AtomicU64, // ADD THIS
}

#[wasm_bindgen]
impl WasmClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
        Self {
            connection_type: None,
            ws_client: None,
            http_client: None,
            next_request_id: AtomicU64::new(1), // ADD THIS
        }
    }

    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed) as i64
    }

    // Then replace all `id: 1i64.into()`, `id: 2i64.into()`, `id: 3i64.into()`
    // with `id: self.next_id().into()`
}
```

### WASM Bridge Injection in Preview (wrapWidgetHtmlWasm)
```javascript
// In PreviewRuntime.wrapWidgetHtmlWasm(html):
wrapWidgetHtmlWasm(html) {
  // WASM artifacts served from preview server
  const wasmJsUrl = '/wasm/mcp_wasm_client.js';
  const wasmBinaryUrl = '/wasm/mcp_wasm_client_bg.wasm';
  const serverUrl = this.mcpUrl; // from /api/config

  return `
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <style>body { margin: 0; padding: 16px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }</style>
  <script type="module">
    import init, { WasmClient } from '${wasmJsUrl}';

    const preview = window.parent.previewRuntime;
    await init('${wasmBinaryUrl}');

    const client = new WasmClient();
    await client.connect('${serverUrl}/mcp');

    window.mcpBridge = {
      callTool: async (name, args) => {
        preview.logConsole('log', 'callTool: ' + name);
        preview.logEvent('bridgeCall', { method: 'callTool', name, args });
        const startTime = Date.now();
        let result, success = true;
        try {
          const mcpResult = await client.call_tool(name, args || {});
          result = { success: !mcpResult.isError, content: mcpResult.content || [], _meta: mcpResult._meta || null };
          success = result.success;
        } catch (e) {
          result = { success: false, error: e.message };
          success = false;
        }
        const duration = Date.now() - startTime;
        preview.logBridgeCall(name, args || {}, result, duration, success);
        return result;
      },
      getState: () => preview.widgetState,
      setState: (state) => { preview.widgetState = { ...preview.widgetState, ...state }; preview.updateStateView(); preview.logEvent('setState', state); },
      sendMessage: (msg) => { preview.logConsole('log', 'sendMessage: ' + msg); preview.logEvent('sendMessage', { message: msg }); },
      openExternal: (url) => { preview.logEvent('openExternal', { url }); window.open(url, '_blank'); },
      get theme() { return preview.theme; },
      get locale() { return preview.locale; },
      get displayMode() { return preview.displayMode; },
    };
    window.openai = window.mcpBridge;

    preview.logEvent('wasmBridgeReady', {});
  </script>
</head>
<body>
${html}
</body>
</html>`;
}
```

### WASM Build Trigger API Endpoint
```rust
// In handlers/wasm.rs:
pub async fn trigger_build(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    match state.wasm_builder.ensure_built().await {
        Ok(path) => json_response(json!({
            "status": "ready",
            "artifactPath": path.display().to_string()
        })),
        Err(e) => json_response(json!({
            "status": "error",
            "error": e.to_string()
        })),
    }
}

pub async fn build_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let status = state.wasm_builder.status().await;
    json_response(json!({ "status": status }))
}

pub async fn serve_wasm_artifact(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    // Serve from cached build output
    let artifact_path = state.wasm_builder.artifact_path().join(&path);
    // ... read file, set correct MIME type (application/wasm for .wasm)
}
```

### Standalone widget-runtime.js Pattern
```javascript
// widget-runtime.js - standalone version
(async function() {
  // Find our own script tag to read data attributes
  const scriptTag = document.currentScript
    || document.querySelector('script[data-mcp-url]');
  const serverUrl = scriptTag?.getAttribute('data-mcp-url');

  if (!serverUrl) {
    console.error('widget-runtime.js: data-mcp-url attribute is required');
    return;
  }

  // Determine WASM artifact URLs relative to this script
  const scriptUrl = new URL(scriptTag.src);
  const wasmJsUrl = new URL('mcp_wasm_client.js', scriptUrl);
  const wasmBinaryUrl = new URL('mcp_wasm_client_bg.wasm', scriptUrl);

  // Dynamic import of the WASM JS module
  const { default: init, WasmClient } = await import(wasmJsUrl.href);
  await init(wasmBinaryUrl.href);

  const client = new WasmClient();
  await client.connect(serverUrl);

  // Expose window.mcpBridge with same API as proxy bridge
  window.mcpBridge = {
    callTool: async (name, args) => {
      const mcpResult = await client.call_tool(name, args || {});
      return { success: !mcpResult.isError, content: mcpResult.content || [] };
    },
    getState: () => (window.__mcpState || {}),
    setState: (s) => { window.__mcpState = { ...(window.__mcpState || {}), ...s }; },
    sendMessage: (msg) => { console.log('[mcpBridge] sendMessage:', msg); },
    openExternal: (url) => { window.open(url, '_blank'); },
    get theme() { return document.documentElement.dataset.theme || 'light'; },
    get locale() { return navigator.language; },
    get displayMode() { return 'inline'; },
  };
  window.openai = window.mcpBridge;

  // Signal ready
  window.dispatchEvent(new Event('mcpBridgeReady'));
})();
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `--target no-modules` IIFE output | `--target web` ES module with `init()` | wasm-pack 0.10+ | ES module enables streaming compilation and tree-shaking; use `--target web` |
| `wasm2es6js` base64 embedding | `initSync()` with BufferSource | wasm-bindgen 0.2.80+ | `initSync` is the supported way to do synchronous initialization; `wasm2es6js` is legacy |
| Manual CORS configuration | SDK built-in `add_cors_headers()` | PMCP SDK current | All `StreamableHttpServer` responses include CORS headers automatically |
| Separate `mcp-session-id` header handling | `WasmHttpTransport` handles session | PMCP SDK current | Transport automatically extracts and forwards session ID from response headers |

**Deprecated/outdated:**
- `wasm-pack --target no-modules`: Still works but produces IIFE that cannot use `WebAssembly.instantiateStreaming()`. Use `--target web` instead.
- `wasm2es6js`: Legacy tool for base64 embedding. Use `initSync()` with manually base64-decoded bytes if single-file is needed.

## Discretion Recommendations

### WASM Artifact Cache Location
**Recommendation:** `target/wasm-bridge/` inside the project root.
**Rationale:** Follows Cargo convention of build artifacts in `target/`. Easy to clean with `cargo clean`. Gitignored by default (`target/` is in `.gitignore`). Avoids polluting global user directories.

### widget-runtime.js Bundle Approach
**Recommendation:** Two-file delivery (JS loader + `.wasm` binary) as the primary approach.
**Rationale:** The WASM binary is 711KB. Base64 encoding inflates to ~950KB. Two-file delivery enables `WebAssembly.instantiateStreaming()` (fastest loading) and is the standard wasm-bindgen pattern. The JS loader is ~35KB. Total: ~750KB (gzips to ~280KB). The three files (`widget-runtime.js`, `mcp_wasm_client.js`, `mcp_wasm_client_bg.wasm`) all co-locate in the same directory.

### Server URL Injection Method
**Recommendation:** `data-mcp-url` attribute on the `<script>` tag for standalone use.
**Rationale:** Self-contained, no global coordination needed. Widget authors write: `<script src="widget-runtime.js" data-mcp-url="http://localhost:3000/mcp"></script>`. For preview context, the URL is injected directly into the `srcdoc` template (no attribute needed -- the preview server knows the URL).

### CORS Handling Strategy
**Recommendation:** Rely on SDK built-in CORS support. No additional work needed.
**Rationale:** `StreamableHttpServer::add_cors_headers()` already adds `Access-Control-Allow-Origin: *` with all required MCP headers exposed (`mcp-session-id`, `mcp-protocol-version`). The preview iframe's WASM client talks directly to the MCP server -- the CORS preflight is handled by the server automatically.

### Non-CORS Fallback
**Recommendation:** No fallback needed for v1.3. Document that WASM mode requires a CORS-enabled MCP server.
**Rationale:** All PMCP servers include CORS by default. Non-PMCP servers are out of scope for v1.3. If needed later, the preview server could add a proxy fallback route.

### wasm-pack Build Configuration
**Recommendation:** `wasm-pack build --target web --out-name mcp_wasm_client --no-opt` with `CARGO_PROFILE_RELEASE_LTO=false`.
**Rationale:** Matches the existing `build.sh` in `examples/wasm-client/`. `--no-opt` skips wasm-opt (avoids download and compatibility issues). LTO disabled for faster builds. The existing `[package.metadata.wasm-pack] wasm-opt = false` in Cargo.toml also disables it.

### Error Handling When wasm-pack Not Installed
**Recommendation:** Check `wasm-pack` binary availability at toggle time. Show inline error with install instructions.
**Rationale:** Failing fast with a clear message is better than a cryptic build failure. The toggle button should show: "WASM mode requires wasm-pack. Run: `cargo install wasm-pack && rustup target add wasm32-unknown-unknown`"

## Open Questions

1. **WASM binary size optimization**
   - What we know: Current binary is 711KB uncompressed, ~250KB gzipped. The `--no-opt` flag skips wasm-opt.
   - What's unclear: How much smaller can it get with `wasm-opt -Os`? Is it worth the additional build complexity?
   - Recommendation: Ship with `--no-opt` for v1.3 (faster builds). Add wasm-opt optimization as a future enhancement.

2. **WASM client as separate crate vs embedded**
   - What we know: `examples/wasm-client/` is currently an example, not a workspace member. The `mcp-preview` crate needs to know where to find the WASM source to build it.
   - What's unclear: Should the WASM client source be moved to `crates/mcp-wasm-bridge/` or kept as an example?
   - Recommendation: Keep in `examples/wasm-client/` for now. The preview server knows the relative path. Moving to `crates/` can happen in Phase 16 (shared bridge library) if needed.

3. **Concurrent WASM build requests**
   - What we know: Multiple browser tabs or rapid toggles could trigger simultaneous builds.
   - What's unclear: Does wasm-pack handle concurrent builds gracefully?
   - Recommendation: Use a build lock (`RwLock<BuildStatus>` with `Building` state) to serialize builds. Second toggle while building waits for the first build to complete.

## Sources

### Primary (HIGH confidence)
- `examples/wasm-client/src/lib.rs` - Existing WasmClient implementation with connect/list_tools/call_tool
- `examples/wasm-client/Cargo.toml` - Dependencies and wasm-pack configuration
- `examples/wasm-client/build.sh` - Existing build script with flags
- `examples/wasm-client/pkg/mcp_wasm_client.js` - Generated JS glue with `init()` and `initSync()` exports
- `examples/wasm-client/pkg/mcp_wasm_client.d.ts` - Generated TypeScript declarations
- `crates/mcp-preview/src/proxy.rs` - McpProxy with session management, tool/resource methods
- `crates/mcp-preview/src/server.rs` - PreviewServer with AppState, routes, CORS layer
- `crates/mcp-preview/assets/index.html` - Preview UI with bridge injection in wrapWidgetHtml()
- `src/server/streamable_http_server.rs` lines 1350-1376 - Built-in CORS headers
- `src/shared/wasm_http.rs` - WasmHttpTransport and WasmHttpClient implementations

### Secondary (MEDIUM confidence)
- [wasm-pack build documentation](https://rustwasm.github.io/docs/wasm-pack/commands/build.html) - Build targets and options
- [wasm-bindgen synchronous instantiation guide](https://rustwasm.github.io/docs/wasm-bindgen/examples/synchronous-instantiation.html) - initSync usage
- [wasm-pack Cargo.toml configuration](https://rustwasm.github.io/docs/wasm-pack/cargo-toml-configuration.html) - wasm-opt configuration

### Tertiary (LOW confidence)
- [wasm-pack inline WASM discussion](https://github.com/rustwasm/wasm-pack/issues/1074) - Community approaches to single-file bundles; no official support yet

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in use in `examples/wasm-client/`; no new dependencies needed
- Architecture: HIGH - Clear separation between WASM client (existing), bridge adapter (thin JS), build automation (wasm-pack invocation), and UI toggle (preview HTML)
- Pitfalls: HIGH - Hardcoded request ID bug is documented in STATE.md; CORS support verified in SDK source; MIME type handling verified in asset handler; build time expectations set from existing build.sh

**Research date:** 2026-02-24
**Valid until:** 2026-03-24 (stable domain; wasm-bindgen/wasm-pack ecosystem is mature)
