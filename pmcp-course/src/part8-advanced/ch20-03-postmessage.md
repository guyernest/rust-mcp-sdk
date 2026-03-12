# Widget Communication with ext-apps

In this section, you'll learn how to build widgets using the `@modelcontextprotocol/ext-apps` SDK, implement the required protocol handlers, bundle with Vite, and follow cross-host best practices.

## Learning Objectives

After completing this section, you will be able to:

- Create an `App` instance and connect to the host
- Register all required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) before calling `connect()`
- Read initial data from `hostContext` and handle ongoing tool results via `ontoolresult`
- Call server tools from the widget using `app.callServerTool()`
- Bundle widgets with Vite + `vite-plugin-singlefile` into self-contained HTML
- Build a React widget using `useApp` and `useHostStyles` hooks
- Load external images safely across all hosts using the fetch-blob pattern

## The ext-apps SDK

Widgets use the `@modelcontextprotocol/ext-apps` SDK to communicate with MCP hosts. The SDK provides the `App` class -- a single entry point that handles the postMessage protocol for you.

### Widget Lifecycle

The lifecycle follows five steps:

1. **Create `App`** -- no capabilities needed (do not pass `tools`)
2. **Register ALL protocol handlers before `connect()`** -- missing handlers cause connection teardown
3. **Call `app.connect()`** -- performs the `ui/initialize` handshake with the host
4. **Read `app.getHostContext()`** -- some hosts deliver data only at init time
5. **Use `app.callServerTool()`** -- for interactive widgets that call back to the server

### Required Protocol Handlers

```
+-----------------------------------------------------------------------+
|                    CRITICAL: Handler Registration                      |
+-----------------------------------------------------------------------+
|                                                                       |
|  You MUST register onteardown, ontoolinput, ontoolcancelled, and      |
|  onerror handlers BEFORE calling connect().                           |
|                                                                       |
|  Without these, hosts like Claude Desktop and Claude.ai will TEAR     |
|  DOWN THE ENTIRE MCP CONNECTION after the first tool result.          |
|                                                                       |
|  The widget briefly appears, then everything dies. This is the #1     |
|  issue when porting widgets from mcp-preview to real hosts.           |
|                                                                       |
+-----------------------------------------------------------------------+
```

Register all handlers before connecting:

```js
// ALL of these are required -- not just ontoolresult
app.onteardown = async () => { return {}; };
app.ontoolinput = (params) => { console.debug("Tool input:", params); };
app.ontoolcancelled = (params) => { console.debug("Cancelled:", params.reason); };
app.onerror = (err) => { console.error("App error:", err); };
app.ontoolresult = (result) => { /* your data handler */ };
```

The handlers can be minimal stubs -- they just need to be registered. The host sends protocol messages to each handler, and if the handler is missing, the host considers the connection broken.

| Handler | When It Fires | Required? |
|---------|--------------|-----------|
| `onteardown` | Host is shutting down the connection | Yes -- must return `{}` |
| `ontoolinput` | Host is about to call a tool | Yes |
| `ontoolcancelled` | Tool call was cancelled | Yes |
| `onerror` | Protocol error occurred | Yes |
| `ontoolresult` | Tool call completed with result | Recommended -- this is where you get data |

### Capabilities Declaration

Do **not** pass `tools` capability to `new App()`. ChatGPT's adapter rejects it with a Zod validation error (`-32603: expected "object"`):

```js
// Correct -- receives tool results, can call server tools
const app = new App({ name: "my-widget", version: "1.0.0" });

// WRONG -- ChatGPT rejects the tools capability
// const app = new App({ name: "my-widget", version: "1.0.0", capabilities: { tools: true } });
```

Tool results are delivered via `hostContext.toolOutput` and the `ontoolresult` callback without needing the `tools` capability. The `callServerTool()` API also works without it.

## Hands-On: Minimal Widget

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

Walk through the four steps:

1. **Create `App`** with just name and version -- no capabilities
2. **Register all five handlers** before `connect()` -- even `onteardown` which just returns `{}`
3. **Call `connect()`** -- the SDK performs the `ui/initialize` handshake
4. **Read `hostContext`** -- some hosts (like ChatGPT) deliver tool data only at initialization

**Try this:** Remove the `onteardown` handler and test in `cargo pmcp preview`. Then imagine deploying to Claude Desktop -- the widget would appear briefly, then the entire MCP connection would die.

## Calling Server Tools from Widgets

Interactive widgets can call tools on the MCP server using `app.callServerTool()`:

```js
const result = await app.callServerTool({
  name: "search_images",
  arguments: { class_name: "Dog" },
});

if (result.structuredContent) {
  // Render the structured data
  renderData(result.structuredContent);
}
```

This works on hosts that support it (mcp-preview, Claude Desktop). Add a `.catch()` fallback for hosts that don't:

```js
try {
  const result = await app.callServerTool({
    name: "search_images",
    arguments: { class_name: "Dog" },
  });
  renderData(result.structuredContent);
} catch (err) {
  console.warn("callServerTool not supported on this host:", err);
  // Fallback: display cached data or show a message
}
```

## React Widget

For React-based widgets, the ext-apps SDK provides hooks:

```tsx
import { useApp, useHostStyles } from "@modelcontextprotocol/ext-apps/react";

export default function MyWidget() {
  const { app, isConnected } = useApp({
    appInfo: { name: "my-widget", version: "1.0.0" },
    // Do not pass capabilities.tools -- ChatGPT rejects it
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

The `useApp` hook handles the full lifecycle: creating the App instance, registering required handlers, and calling `connect()`. The `useHostStyles` hook applies the host's theme CSS variables and fonts to your component.

> **Note:** The `onAppCreated` callback is where you register `ontoolresult` and other data handlers. Required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) are registered automatically by the hook.

## Loading External Images

Hosts enforce strict CSP on widget iframes. To load external images reliably across all hosts:

1. **Server side:** Declare the image CDN in `WidgetCSP` (see ch20-01)
2. **Widget side:** Use fetch-to-blob as defense-in-depth:

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
        // Fallback to direct URL -- works on permissive hosts (ChatGPT, mcp-preview).
        // On strict hosts, onerror fires and shows a placeholder.
        img.src = url;
    });
}
```

**Why both server-side CSP and client-side fetch-blob?** The `_meta.ui.csp` declaration tells the host to relax its CSP for the declared domains. The fetch-blob approach works even if the host ignores `_meta.ui.csp` -- `blob:` URLs are typically allowed in `img-src`. Together they maximize cross-host compatibility.

> **Memory:** Always call `URL.revokeObjectURL()` after the image loads. Without this, each fetched image leaks a blob URL that persists for the page lifetime.

## Bundling Widgets with Vite

Widgets must be self-contained HTML files with all JavaScript inlined. Use **Vite + vite-plugin-singlefile** to bundle the ext-apps SDK into each widget.

### Why Bundling Is Required

Claude Desktop's iframe CSP blocks external script loading. If you use a CDN import like `<script src="https://cdn.example.com/ext-apps.js">`, it will **fail silently** -- no error, no widget, just a blank iframe. Vite bundles the ext-apps SDK directly into the HTML file, bypassing this restriction.

### Setup

```bash
cd widget/
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
    target: "esnext",  // required -- ext-apps SDK uses top-level await
    rollupOptions: {
      input: `${widget}.html`,
    },
    outDir: "dist",
    emptyOutDir: false,  // preserve other widgets' output
  },
});
```

### Building Multiple Widgets

```json
{
  "scripts": {
    "build": "rm -rf dist && WIDGET=image-explorer vite build && WIDGET=relationship-viewer vite build"
  }
}
```

### Build and Embed

```bash
npm run build
# -> dist/image-explorer.html (~130KB, self-contained)
```

Embed the built output in your Rust binary:

```rust
const WIDGET_HTML: &str = include_str!("../../widget/dist/image-explorer.html");
```

### Build Order

Widget HTML must exist before `cargo build` (since `include_str!` runs at compile time):

```bash
cd widget && npm ci && npm run build && cd ..
cargo build --release
```

## Common Failures

**Widget shows briefly then connection drops (Claude Desktop/Claude.ai):**

The widget is missing protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`). ALL handlers must be registered before `connect()`, even if they only log a debug message. This is the #1 issue when porting widgets from mcp-preview (which is more forgiving) to real hosts.

**Widget loads but never shows tool results:**

Do NOT pass `tools` capability to `new App()`. ChatGPT rejects it. Check that `ontoolresult` is registered BEFORE `connect()`, not after.

**"Received a response for an unknown message ID" (mcp-preview):**

Two App instances on the same postMessage channel. This happens when mcp-preview's wrapper injects its own App and the widget bundles its own App via Vite. For Vite-bundled widgets, use `UIResource::html_mcp_app()` + `UIResourceContents::html()` directly.

**Images/external resources blocked (Claude.ai):**

Claude.ai enforces strict CSP: `img-src 'self' data: blob:` -- external image URLs are blocked. Declare external domains in `WidgetCSP` on the resource metadata, and use fetch-blob in the widget as a fallback. Always use HTTPS.

**Widget not rendered at all (Claude Desktop):**

Check `resources/read` returns `_meta.ui.resourceUri` (required by Claude Desktop). Check MIME type is `text/html;profile=mcp-app` (not `text/html+mcp`). Check widget is self-contained (no external script imports).

## Chapter Summary

Here's what you've learned across all three sections of Chapter 20:

| Concept | What You Learned |
|---------|-----------------|
| **UIResource::html_mcp_app()** | Register widgets with the correct MIME type for all hosts |
| **WidgetCSP** | Declare external domains for images, APIs, and fonts |
| **ToolInfo::with_ui()** | Associate tools with widgets via `_meta.ui.resourceUri` |
| **structuredContent** | Return data for widget rendering alongside text for the model |
| **with_host_layer()** | Enable multi-host support (ChatGPT, Claude Desktop, VS Code) |
| **ext-apps App class** | Cross-host widget communication with required protocol handlers |
| **Vite bundling** | Self-contained HTML required by Claude Desktop CSP |

The standard development workflow:

1. **Server side:** `ToolInfo::with_ui()` + `UIResource::html_mcp_app()` + `with_host_layer()` + `with_structured_content()`
2. **Widget side:** Create `App`, register ALL handlers, `connect()`, read `hostContext`, render data
3. **Bundling:** Vite + vite-plugin-singlefile into self-contained HTML
4. **Preview:** `cargo pmcp preview --url http://localhost:3000 --open`
5. **Validate:** `mcp-tester apps http://localhost:3000` or `cargo pmcp test apps`

## Practice Ideas

Ready to experiment? Here are some exercises to deepen your understanding:

1. **Build a minimal widget from scratch.** Follow the lifecycle: create App, register handlers, connect, read hostContext. Return `structuredContent` from a tool and render it in the widget. Use `mcp-tester apps` to validate your metadata.

2. **Add external image loading.** Create a widget that loads images from an external CDN. Declare the CDN in `WidgetCSP` on the server side, and use the fetch-blob pattern in the widget. Test in `cargo pmcp preview` to verify images load.

3. **Build a React widget.** Use the `useApp` and `useHostStyles` hooks from `@modelcontextprotocol/ext-apps/react`. Add a button that calls `app.callServerTool()` and renders the result.

4. **Test with different modes.** Run `cargo pmcp preview --mode chatgpt` and verify your widget works in ChatGPT emulation mode. Then run `mcp-tester apps --mode chatgpt` to check ChatGPT-specific metadata.

5. **Validate your MCP App server.** Run `mcp-tester apps http://localhost:3000 --strict` and fix any warnings. Try `--mode claude-desktop` to check Claude Desktop compatibility.

---

*<- Back to [Chapter Index](./ch20-mcp-apps.md)*
