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

### 1. Register tools with UI metadata

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

### 2. Return structuredContent from tool calls

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

### 3. Register the widget HTML as a resource

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

### 4. Add outputSchema (optional but recommended)

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

### Recommended SDK: `@modelcontextprotocol/ext-apps`

This is the official MCP Apps SDK that works across all major hosts:
- Claude Desktop
- ChatGPT
- VS Code
- Goose, Postman, MCPJam, and more

Install: `npm install @modelcontextprotocol/ext-apps`

### Minimal vanilla JS widget

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

    const app = new App(
      { name: "my-widget", version: "1.0.0" },
      { tools: true }  // declare capabilities
    );

    // 1. Register handlers BEFORE connecting (to avoid missing early notifications)
    app.ontoolresult = (result) => {
      // Called when a new tool result arrives after initial load
      if (result.structuredContent) {
        renderData(result.structuredContent);
      }
    };

    app.onhostcontextchanged = (ctx) => {
      // React to theme changes, etc.
      if (ctx.theme) applyTheme(ctx.theme);
    };

    // 2. Connect to host
    await app.connect();

    // 3. Read initial data from hostContext (delivered at init time)
    const ctx = app.getHostContext();
    if (ctx?.toolOutput) {
      renderData(ctx.toolOutput);
    }
    if (ctx?.theme) {
      applyTheme(ctx.theme);
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

    function applyTheme(theme) {
      document.documentElement.setAttribute("data-theme", theme);
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
    capabilities: { tools: true },
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

### Key patterns

**Data delivery varies by host.** The `App` class abstracts this:
- **Claude Desktop** delivers `toolOutput` inside `hostContext` at init time
- **ChatGPT** delivers via `window.openai.toolOutput` and globals events
- The `App` class normalizes both into `app.getHostContext()` and `app.ontoolresult`

**Always read `hostContext` after `connect()`.** Some hosts only provide data at initialization, not through post-init notifications:
```js
await app.connect();
const ctx = app.getHostContext();
// ctx.toolInput, ctx.toolOutput, ctx.theme, ctx.locale, etc.
```

**Register handlers before `connect()`.** Ensures you don't miss early notifications:
```js
app.ontoolresult = (result) => { /* ... */ };
app.onhostcontextchanged = (ctx) => { /* ... */ };
await app.connect();  // now handlers are ready
```

### Building self-contained widgets

For PMCP servers, widgets are typically served as self-contained HTML files via `resources/read`. Bundle the ext-apps SDK into your widget HTML:

**Option A: CDN import (simplest)**
```html
<script type="module">
  import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps@1.2.2";
  // ...
</script>
```

**Option B: Build tool (Vite, esbuild, etc.)**
```bash
npm install @modelcontextprotocol/ext-apps
npx vite build  # or esbuild --bundle
```
Then embed the built output in your Rust binary as a string constant.

**Option C: Inline without SDK (simple widgets)**

For very simple widgets that only need to receive data (no tool calls back), you can use raw postMessage. However, this is NOT recommended for production — the ext-apps SDK handles host differences, protocol evolution, and edge cases:

```html
<!-- NOT RECOMMENDED — use ext-apps SDK instead -->
<script>
  // Fallback pattern (fragile, host-specific)
  window.addEventListener('message', (e) => {
    if (e.data?.method === 'ui/toolResult') {
      renderData(e.data.params?.structuredContent);
    }
  });
</script>
```

### 5. Enable ChatGPT compatibility (required for ChatGPT)

Register the ChatGPT host layer so the server enriches `_meta` with `openai/*` descriptor keys:

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

## Widget bundling strategies

### Option A: Vite + vite-plugin-singlefile (recommended)

Bundle the ext-apps SDK into a single self-contained HTML file. This is the most reliable approach — works in Claude Desktop (which blocks external script loading), ChatGPT, and all other hosts.

```bash
# widgets/package.json
npm install @modelcontextprotocol/ext-apps
npm install -D vite vite-plugin-singlefile typescript

# widgets/vite.config.ts
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
export default defineConfig({
  plugins: [viteSingleFile()],
  build: { target: "esnext", rollupOptions: { input: "mcp-app.html" } },
});

# Build → widgets/dist/mcp-app.html (single file, ~120KB)
npm run build
```

Then embed in Rust:
```rust
const WIDGET_HTML: &str = include_str!("../../../widgets/dist/mcp-app.html");
```

> **Important:** Set `target: "esnext"` in the Vite config — the ext-apps SDK uses top-level `await` which requires ESNext target.

### Option B: CDN import (simple but limited)

```html
<script type="module">
  import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps@1.2.2";
</script>
```

Works in ChatGPT and mcp-preview, but **fails in Claude Desktop** due to iframe CSP blocking external scripts.

### Option C: Minimal hand-rolled postMessage (NOT recommended)

A hand-rolled JSON-RPC postMessage implementation seems simpler but is fragile:
- Missing protocol handlers (`ui/teardown`, `ui/toolInput`, etc.) cause the host to tear down the MCP connection entirely
- Different hosts expect different handshake parameters
- No automatic theme/context handling

**Always use the ext-apps SDK** — it handles host differences, protocol evolution, and edge cases.

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

### Common failures

**Widget shows briefly then connection drops (Claude Desktop/Claude.ai):**
- The widget is missing protocol handlers. Use the full ext-apps SDK, not hand-rolled postMessage.
- After receiving a tool result, if the widget doesn't respond to `ui/teardown` properly, the host kills the entire MCP connection.

**"Received a response for an unknown message ID" (mcp-preview):**
- Two App instances on the same postMessage channel. This happens when:
  - mcp-preview's wrapper injects its own App, AND the widget bundles its own App
  - mcp-preview detects bundled SDKs automatically and skips its wrapper App
  - If your Vite-bundled widget still triggers this, check that the build output is being used (not the source HTML)

**resources/read missing openai/* keys (ChatGPT mode):**
- Server needs `.with_host_layer(HostType::ChatGpt)` in the builder
- Without this, the `uri_to_tool_meta` propagation index has no openai keys to propagate

**Widget not rendered at all (Claude Desktop):**
- Check `resources/read` returns `_meta.ui.resourceUri` — required by Claude Desktop
- Check MIME type is `text/html;profile=mcp-app` (not `text/html+mcp`)
- Check CORS header `access-control-allow-origin: *` is present

## Reference implementations

- **ext-apps examples:** https://github.com/modelcontextprotocol/ext-apps/tree/main/examples
  - `customer-segmentation-server` — Chart.js data visualization
  - `three-js-server` — Three.js 3D rendering
  - `maps-server` — Cesium.js globe
  - Starter templates for React, Vue, Svelte, Solid, Vanilla JS

- **PMCP examples:** `examples/mcp-apps-chess/` — chess game widget

## SDK links

- **ext-apps SDK (widget):** `npm install @modelcontextprotocol/ext-apps` ([GitHub](https://github.com/modelcontextprotocol/ext-apps))
- **PMCP SDK (server):** `cargo add pmcp --features mcp-apps` ([crates.io](https://crates.io/crates/pmcp))
