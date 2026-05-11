# Building MCP Apps with PMCP SDK

Guide for MCP server developers building interactive widget UIs.

## Architecture Overview

Building an MCP App involves two parts:

1. **Server side (Rust, PMCP SDK)** — registers tools with UI metadata, serves widget HTML as resources, returns `structuredContent` from tool calls
2. **Widget side (JS/TS, ext-apps SDK)** — the interactive UI that runs in the host's iframe, communicates with the host via the `App` class

```
┌─────────────────────────────────────────────────────┐
│  Host (Claude Desktop, ChatGPT, VS Code, etc.)      │
│                                                      │
│  tools/list ─── _meta.ui.resourceUri ──► knows which │
│                                          tool has UI │
│  tools/call ─── structuredContent ─────► data for UI │
│  resources/read ── HTML ───────────────► widget code │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Widget iframe                                  │  │
│  │  @modelcontextprotocol/ext-apps (App class)     │  │
│  │  ← hostContext (theme, toolInput, toolOutput) → │  │
│  │  ← app.callServerTool() → tools/call            │  │
│  └────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Server Side (Rust)

### 1. Enable ChatGPT host layer

Register the ChatGPT host layer so the server enriches `_meta` with `openai/*` descriptor keys. This is required for ChatGPT and harmless for other hosts:

```rust
use pmcp::types::mcp_apps::HostType;

Server::builder()
    .name("my-server")
    .version("1.0.0")
    .with_host_layer(HostType::ChatGpt)  // adds openai/* keys to _meta
    // ... tools, resources, etc.
    .build()
```

Without this, `tools/list` and `resources/read` will be missing `openai/outputTemplate`, `openai/widgetAccessible`, and `openai/toolInvocation/*` keys that ChatGPT requires.

### 2. Register tools with UI metadata

Associate a tool with its widget using `ToolInfo::with_ui()`:

```rust
use pmcp::types::protocol::ToolInfo;
use serde_json::json;

let tool = ToolInfo::with_ui(
    "search_images",
    Some("Search for images by class name".to_string()),
    json!({
        "type": "object",
        "properties": {
            "class_name": { "type": "string" }
        },
        "required": ["class_name"]
    }),
    "ui://my-app/explorer.html",  // points to the widget resource
);
```

This produces `_meta: { "ui": { "resourceUri": "ui://my-app/explorer.html" } }` in the `tools/list` response, which tells hosts like Claude Desktop and ChatGPT that this tool has a widget.

For ChatGPT-specific metadata (e.g., border preference, invoking messages), use `WidgetMeta`:

```rust
use pmcp::types::mcp_apps::WidgetMeta;

let tool = ToolInfo::with_ui("my_tool", None, schema, "ui://my-app/widget.html")
    .with_widget_meta(WidgetMeta::new().prefers_border(true));
```

### 3. Return structuredContent from tool calls

Return data alongside text content so the widget can render it:

```rust
use pmcp::types::protocol::{CallToolResult, Content};
use serde_json::json;

let result = CallToolResult::new(vec![
    Content::text("Found 42 images of dogs"),
])
.with_structured_content(json!({
    "columns": [
        { "name": "image_id", "data_type": "varchar" },
        { "name": "thumbnail_url", "data_type": "varchar" }
    ],
    "rows": [
        { "image_id": "abc123", "thumbnail_url": "https://..." }
    ]
}));
```

- `content` — text for the AI model to understand
- `structuredContent` — data for the widget to render (also visible to the model)

### 4. Register the widget HTML as a resource

Use `UIResource` and `UIResourceContents` to register with the correct MIME type:

```rust
use pmcp::types::ui::{UIResource, UIResourceContents};

// Create the resource declaration (for resources/list)
let resource = UIResource::html_mcp_app(
    "ui://my-app/explorer.html",
    "Image Explorer",
);

// Create the resource content (for resources/read)
let contents = UIResourceContents::html(
    "ui://my-app/explorer.html",
    &html_content,  // your widget HTML string
);
// contents.mime_type = "text/html;profile=mcp-app"

// Register with ResourceCollection
resources.add_ui_resource(resource, contents);
```

Both `UIResource::html_mcp_app()` and `UIResourceContents::html()` produce `mimeType: "text/html;profile=mcp-app"` — the standard MIME type recognized by Claude Desktop, ChatGPT, and other MCP hosts.

> **Important:** Do not use the legacy `UIResource::html_mcp()` constructor — it produces `text/html+mcp` which is not recognized by Claude Desktop.

<a id="csp-external-resources"></a>
### 5. Declare CSP for external domains

If your widget loads external resources (images, API calls, fonts), you **must** declare them in `_meta.ui.csp` on the resource contents. Without this, hosts like Claude.ai block all external domains via Content-Security-Policy.

```rust
use pmcp::types::mcp_apps::{WidgetCSP, WidgetMeta};

let csp = WidgetCSP::new()
    .resources("https://*.staticflickr.com")  // img-src: images, scripts, fonts
    .connect("https://*.staticflickr.com");   // connect-src: fetch/XHR

let meta = WidgetMeta::new()
    .resource_uri("ui://my-app/explorer.html")
    .prefers_border(true)
    .csp(csp);
```

This produces `_meta.ui.csp` with `connectDomains` and `resourceDomains` arrays on the `resources/read` response, which the host merges into its iframe CSP.

> **Important:** CSP metadata goes on the **resource contents** (returned by `resources/read`), not just the resource listing. See the [ext-apps CSP spec](https://apps.extensions.modelcontextprotocol.io/api/documents/csp-and-cors.html) for details.

### 6. Add outputSchema (optional but recommended)

`outputSchema` tells the host the shape of `structuredContent`, enabling validation:

```rust
let tool = ToolInfo::with_ui("search_images", None, input_schema, "ui://my-app/explorer.html")
    .with_output_schema(json!({
        "type": "object",
        "properties": {
            "columns": { "type": "array" },
            "rows": { "type": "array" }
        }
    }));
```

## Widget Side (JavaScript/TypeScript)

Widgets use the `@modelcontextprotocol/ext-apps` SDK and **must be bundled into self-contained HTML** using Vite + vite-plugin-singlefile. This is required because Claude Desktop's iframe CSP blocks external script loading — CDN imports will fail silently.

### Widget lifecycle

1. **Create `App`** — no capabilities needed (do not pass `tools`)
2. **Register ALL protocol handlers before `connect()`** — missing handlers cause connection teardown
3. **Call `app.connect()`** — performs the `ui/initialize` handshake with the host
4. **Read `app.getHostContext()`** — some hosts deliver data only at init time
5. **Use `app.callServerTool()`** — for interactive widgets that call back to the server

<a id="handlers-before-connect"></a>
### Required protocol handlers

> **Critical:** You MUST register `onteardown`, `ontoolinput`, `ontoolcancelled`, and `onerror` handlers before calling `connect()`. Without these, hosts like Claude Desktop and Claude.ai will **tear down the entire MCP connection** after the first tool result — the widget briefly appears then everything dies.

```js
// ALL of these are required — not just ontoolresult
app.onteardown = async () => { return {}; };
app.ontoolinput = (params) => { console.debug("Tool input:", params); };
app.ontoolcancelled = (params) => { console.debug("Tool cancelled:", params.reason); };
app.onerror = (err) => { console.error("App error:", err); };
app.ontoolresult = (result) => { /* your data handler */ };
```

<a id="do-not-pass-tools"></a>
### Capabilities declaration

Do **not** pass `tools` capability to `new App()`. ChatGPT's adapter rejects it with a Zod validation error (`-32603: expected "object"`). Widgets still receive tool results via `hostContext.toolOutput` and `ontoolresult` notifications without this capability. The `callServerTool()` API also works without it on hosts that support it (mcp-preview, Claude Desktop) — add a `.catch()` fallback for hosts that don't.

```js
// Standard widget — receives tool results, can call server tools
const app = new App({ name: "my-widget", version: "1.0.0" });
```

> **Warning:** Do NOT pass `{ tools: true }` or `{ tools: {} }` as capabilities. ChatGPT's adapter validates `appCapabilities` with a strict Zod schema that rejects the `tools` key entirely, causing the widget to fail to initialize.

### Minimal widget example

Create your source HTML with a bare import (Vite resolves it from `node_modules`):

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>My Widget</title>
</head>
<body>
  <div id="output">Waiting for data...</div>

  <script type="module">
    import { App } from "@modelcontextprotocol/ext-apps";

    const app = new App({ name: "my-widget", version: "1.0.0" });

    // 1. Register ALL handlers BEFORE connecting (required by protocol)
    app.onteardown = async () => { return {}; };
    app.ontoolinput = (params) => { console.debug("Tool input:", params); };
    app.ontoolcancelled = (params) => { console.debug("Cancelled:", params.reason); };
    app.onerror = (err) => { console.error("App error:", err); };

    app.ontoolresult = (result) => {
      if (result.structuredContent) {
        renderData(result.structuredContent);
      }
    };

    // 2. Connect to host
    await app.connect();

    // 3. Read initial data from hostContext
    const ctx = app.getHostContext();
    if (ctx?.toolOutput) {
      renderData(ctx.toolOutput);
    }

    // 4. Optionally call server tools from the widget
    async function refresh() {
      const result = await app.callServerTool({
        name: "search_images",
        arguments: { class_name: "Dog" },
      });
      if (result.structuredContent) {
        renderData(result.structuredContent);
      }
    }

    function renderData(data) {
      document.getElementById("output").textContent = JSON.stringify(data);
    }
  </script>
</body>
</html>
```

### React widget

```tsx
import { useApp, useHostStyles } from "@modelcontextprotocol/ext-apps/react";

export default function MyWidget() {
  const { app, isConnected } = useApp({
    appInfo: { name: "my-widget", version: "1.0.0" },
    // Do not pass capabilities.tools — ChatGPT rejects it
    onAppCreated: (app) => {
      app.ontoolresult = (result) => {
        // handle new tool results
      };
    },
  });

  // Apply host theme, CSS variables, and fonts
  useHostStyles(app, app?.getHostContext());

  if (!isConnected) return <div>Connecting...</div>;

  const handleSearch = async () => {
    const result = await app.callServerTool({
      name: "search_images",
      arguments: { class_name: "Dog" },
    });
    // use result.structuredContent
  };

  return <button onClick={handleSearch}>Search</button>;
}
```

### Loading external images

Hosts enforce strict CSP on widget iframes. To load external images reliably across all hosts:

1. **Server side:** Declare the image CDN in `WidgetCSP` (see section 5 above)
2. **Widget side:** Use fetch→blob as defense-in-depth for hosts that don't support `_meta.ui.csp`:

```js
function loadImage(img, url) {
    fetch(url, { mode: 'cors' }).then(function(r) {
        if (!r.ok) throw new Error(r.status);
        return r.blob();
    }).then(function(blob) {
        img.src = URL.createObjectURL(blob);
        // Revoke blob URL after render to prevent memory leaks
        img.onload = function() { URL.revokeObjectURL(img.src); img.onload = null; };
    }).catch(function() {
        // Fallback to direct URL — works on permissive hosts (ChatGPT, mcp-preview).
        // On strict hosts, onerror fires and shows a placeholder.
        img.src = url;
    });
}
```

> **Why both?** The `_meta.ui.csp` declaration tells the host to relax its CSP for the declared domains. The fetch→blob approach works even if the host ignores `_meta.ui.csp` — `blob:` URLs are typically allowed in `img-src`. Together they maximize cross-host compatibility.

> **Memory:** Always call `URL.revokeObjectURL()` after the image loads. Without this, each fetched image leaks a blob URL that persists for the page lifetime.

## Bundling widgets with Vite

Widgets must be self-contained HTML files with all JavaScript inlined. Use **Vite + vite-plugin-singlefile** to bundle the ext-apps SDK into each widget.

<a id="vite-singlefile"></a>
### Setup

```bash
cd widget/  # or widgets/
npm init -y
npm install @modelcontextprotocol/ext-apps
npm install -D vite vite-plugin-singlefile
```

### vite.config.ts

`vite-plugin-singlefile` uses `inlineDynamicImports` which only supports a single input per build. Use the `WIDGET` env var to select which widget to build:

```ts
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

const widget = process.env.WIDGET || "mcp-app";

export default defineConfig({
  plugins: [viteSingleFile()],
  build: {
    target: "esnext",  // required — ext-apps SDK uses top-level await
    rollupOptions: {
      input: `${widget}.html`,
    },
    outDir: "dist",
    emptyOutDir: false,  // preserve other widgets' output
  },
});
```

### package.json scripts

For a single widget:
```json
{
  "scripts": {
    "build": "vite build"
  }
}
```

For multiple widgets:
```json
{
  "scripts": {
    "build": "rm -rf dist && WIDGET=image-explorer vite build && WIDGET=relationship-viewer vite build && WIDGET=tree-browser vite build"
  }
}
```

### Build and embed

```bash
npm run build
# → dist/image-explorer.html (~130KB, self-contained)
# → dist/relationship-viewer.html
# → dist/tree-browser.html
```

Embed the built output in your Rust binary:

```rust
const WIDGET_HTML: &str = include_str!("../../widget/dist/image-explorer.html");
```

### Build order

Widget HTML must exist before `cargo build` (since `include_str!` runs at compile time):

```bash
cd widget && npm ci && npm run build && cd ..
cargo lambda build --release --arm64 -p my-server-lambda
```

## Debugging with mcp-preview

### Protocol tab

Run `cargo pmcp preview` and check the Protocol tab. All checks should show PASS:

| Check | Expected | Source |
|-------|----------|--------|
| `tools/list` has `_meta.ui` | `{ "ui": { "resourceUri": "ui://..." } }` | `ToolInfo::with_ui()` |
| `tools/list` has openai keys | `openai/outputTemplate`, `openai/widgetAccessible`, etc. | `.with_host_layer(HostType::ChatGpt)` |
| `tools/call` returns `structuredContent` | JSON data object | `TypedToolWithOutput` or `CallToolResult::with_structured_content()` |
| `tools/call` has `_meta` | `openai/toolInvocation/*` keys | `with_widget_enrichment()` |
| `resources/read` mimeType | `"text/html;profile=mcp-app"` | `UIResourceContents::html()` |
| `resources/read` has `_meta` | `ui/resourceUri` + openai keys | `ResourceCollection` + `uri_to_tool_meta` |
| `resources/read` has CSP (if external resources) | `_meta.ui.csp.resourceDomains` / `connectDomains` | `WidgetMeta::csp()` |

<a id="common-failures-claude"></a>
### Common failures

**Widget shows briefly then connection drops (Claude Desktop/Claude.ai):**
- The widget is missing protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`). ALL handlers must be registered before `connect()`, even if they only log a debug message.
- After receiving a tool result, if the widget doesn't respond to `ui/teardown` properly, the host kills the entire MCP connection.
- This is the #1 issue when porting widgets from mcp-preview (which is more forgiving) to real hosts.

**Widget loads but never shows tool results:**
- Do NOT pass `tools` capability to `new App()`. ChatGPT rejects it. Tool results are delivered via `hostContext.toolOutput` and `ontoolresult` without it.
- Check `ontoolresult` is registered BEFORE `connect()`, not after.

**"Received a response for an unknown message ID" (mcp-preview) / dual-App conflict:**
- Two App instances on the same postMessage channel. This happens when:
  - `McpAppsAdapter::transform()` is applied to a widget that already bundles the ext-apps SDK via Vite — do NOT use `McpAppsAdapter` on pre-bundled widgets
  - mcp-preview's wrapper injects its own App, AND the widget bundles its own App (mcp-preview auto-detects this and skips, but `McpAppsAdapter` does not)
  - If your Vite-bundled widget still triggers this, check that the build output is being used (not the source HTML)
- **Fix:** For Vite-bundled widgets, use `UIResource::html_mcp_app()` + `UIResourceContents::html()` directly, or set `mime_type: "text/html;profile=mcp-app"` and serve the HTML as-is.

**resources/read missing openai/* keys (ChatGPT mode):**
- Server needs `.with_host_layer(HostType::ChatGpt)` in the builder
- Without this, the `uri_to_tool_meta` propagation index has no openai keys to propagate

**Images/external resources blocked (Claude.ai):**
- Claude.ai enforces strict CSP on widget iframes: `img-src 'self' data: blob:` — external image URLs are blocked
- **Fix:** Declare external domains in `WidgetCSP` on the resource metadata (see section 5). Also use `fetch→blob→createObjectURL` in the widget as a fallback (see "Loading external images")
- Always use HTTPS — `http://` URLs are blocked by mixed-content policy even with CSP declarations

**Widget not rendered at all (Claude Desktop):**
- Check `resources/read` returns `_meta.ui.resourceUri` — required by Claude Desktop
- Check MIME type is `text/html;profile=mcp-app` (not `text/html+mcp`)
- Check widget is self-contained (no external script imports — CSP blocks them)

## Reference implementations

- **Open Images** (`built-in/sql-api/servers/open-images/`) — multi-widget server with image grid, relationship viewer, tree browser
- **Calculator** (`examples/mcp-apps-calculator/`) — single-widget TypeScript example
- **ext-apps examples:** https://github.com/modelcontextprotocol/ext-apps/tree/main/examples

## SDK links

- **ext-apps SDK (widget):** `npm install @modelcontextprotocol/ext-apps` ([GitHub](https://github.com/modelcontextprotocol/ext-apps))
- **PMCP SDK (server):** `cargo add pmcp --features mcp-apps` ([crates.io](https://crates.io/crates/pmcp))
