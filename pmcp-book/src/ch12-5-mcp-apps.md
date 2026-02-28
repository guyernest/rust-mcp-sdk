# Chapter 12.5: MCP Apps Extension -- Interactive UIs

The MCP Apps Extension lets your server serve rich, interactive UIs -- charts, maps, games, dashboards -- as **widgets** alongside your tools. Widgets are plain HTML files that communicate with your Rust backend through a bridge API, and adapters transform them for different hosts (ChatGPT, MCP Apps, MCP-UI) without any changes to your widget code.

The modern developer experience is straightforward: write HTML files in a `widgets/` directory, point `WidgetDir` at that directory, and the server reads them from disk on every request. A browser refresh shows your latest changes instantly -- no server restart required.

This chapter covers:

1. **Widget authoring** with `WidgetDir` -- the file-based convention, API, and hot-reload workflow
2. **Bridge communication** -- the `window.mcpBridge` API that widgets use to call tools, read resources, and manage state
3. **Developer workflow** -- scaffolding, live preview, and building for distribution with `cargo pmcp`
4. **Adapter pattern** -- how a single widget works across ChatGPT, MCP Apps, and MCP-UI hosts (Chapter 12.5 continued)
5. **Example walkthroughs** -- the chess, map, and dataviz examples step by step (Chapter 12.5 continued)

**Feature flag requirement:** Enable the `mcp-apps` feature in your `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
```

---

## Quick Start: Your First Widget (5 Minutes)

Let's go from zero to a working interactive widget in three steps.

### Step 1: Scaffold the Project

```bash
cargo pmcp app new my-widget-app
cd my-widget-app
```

This creates a ready-to-run project:

```
my-widget-app/
  src/
    main.rs          # MCP server with tool handlers
  widgets/
    hello.html       # Starter widget (add more .html files here)
  mock-data/
    hello.json       # Mock data for landing page generation
  Cargo.toml
  README.md
```

### Step 2: Write a Widget

The scaffold includes `widgets/hello.html`, which demonstrates the bridge pattern. Here is a minimal widget that calls a tool and displays the result:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Hello Widget</title>
    <!-- The bridge script tag is auto-injected by the server.
         Do NOT add it manually. -->
    <style>
        body {
            font-family: system-ui, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: #f8f9fa;
        }
        .card {
            background: white;
            border-radius: 12px;
            box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
            padding: 32px;
            max-width: 400px;
            width: 100%;
        }
    </style>
</head>
<body>
    <div class="card">
        <h1>Say Hello</h1>
        <input type="text" id="name" placeholder="Enter a name..." />
        <button id="greet">Say Hello</button>
        <div id="result"></div>
    </div>

    <script>
        document.getElementById('greet').addEventListener('click', async () => {
            const name = document.getElementById('name').value.trim();
            if (!name) return;

            try {
                // Call the "hello" tool via the MCP bridge
                const response = await window.mcpBridge.callTool('hello', { name });
                document.getElementById('result').textContent = response.greeting;
            } catch (err) {
                document.getElementById('result').textContent = 'Error: ' + err.message;
            }
        });
    </script>
</body>
</html>
```

### Step 3: Run and Preview

```bash
# Build and start the server
cargo run &

# Open the browser-based preview
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser. Type a name, click the button, and the widget calls the `hello` tool on your MCP server, displaying the greeting it returns.

### What the Server Looks Like

The scaffolded `src/main.rs` uses `ServerBuilder` with `WidgetDir` and `ChatGptAdapter`:

```rust
use async_trait::async_trait;
use pmcp::server::mcp_apps::{ChatGptAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, WidgetMeta};
use pmcp::types::protocol::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

// Tool input type
#[derive(Deserialize, JsonSchema)]
struct HelloInput {
    name: String,
}

// Tool handler -- a pure function
fn hello_handler(input: HelloInput, _extra: RequestHandlerExtra) -> Result<serde_json::Value> {
    Ok(json!({
        "greeting": format!("Hello, {}!", input.name),
        "name": input.name
    }))
}

// Resource handler that serves widgets from the widgets/ directory
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}

impl AppResources {
    fn new(widgets_path: PathBuf) -> Self {
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("my widget app widget");
        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);
        Self { chatgpt_adapter, widget_dir }
    }
}

#[async_trait]
impl ResourceHandler for AppResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let transformed = self.chatgpt_adapter.transform(uri, widget_name, &html);

            Ok(ReadResourceResult {
                contents: vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(ExtendedUIMimeType::HtmlSkybridge.to_string()),
                }],
            })
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                description: Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(ExtendedUIMimeType::HtmlSkybridge.to_string()),
            })
            .collect();

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }
}
```

The server registers the tool and the resource handler with `ServerBuilder`, then runs on an HTTP transport:

```rust
let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

let server = ServerBuilder::new()
    .name("my-widget-app")
    .version("0.1.0")
    .tool_typed_sync_with_description("hello", "Greet someone by name", hello_handler)
    .resources(AppResources::new(widgets_path))
    .build()?;
```

This pattern -- `WidgetDir` for widget discovery, `ChatGptAdapter` for bridge injection, `ResourceHandler` for serving -- is identical across all three shipped examples (chess, map, and dataviz).

---

## Widget Authoring with WidgetDir

### Convention

Widgets are `.html` files in a `widgets/` directory. The filename maps directly to an MCP resource URI:

| File                   | MCP Resource URI    |
|------------------------|---------------------|
| `widgets/board.html`   | `ui://app/board`    |
| `widgets/map.html`     | `ui://app/map`      |
| `widgets/hello.html`   | `ui://app/hello`    |

Each widget is a single, self-contained HTML file. The server auto-injects the bridge script tag via the adapter -- widget authors never add bridge boilerplate manually.

### WidgetDir API

`WidgetDir` lives in `pmcp::server::mcp_apps` and provides three operations:

**Construction:**

```rust
use pmcp::server::mcp_apps::WidgetDir;

// Point at the widgets directory
let widget_dir = WidgetDir::new("widgets");

// The path does not need to exist at construction time.
// Errors are returned when discover() or read_widget() are called.
```

**Discovery:**

```rust
// Scan for .html files, returns Vec<WidgetEntry> sorted by filename
let entries = widget_dir.discover()?;

for entry in &entries {
    println!("{} -> {}", entry.filename, entry.uri);
    // "board" -> "ui://app/board"
    // "map"   -> "ui://app/map"
}
```

The `WidgetEntry` struct has three fields:

| Field      | Type       | Description                                    |
|------------|------------|------------------------------------------------|
| `filename` | `String`   | Stem of the HTML file (e.g., `"board"`)        |
| `uri`      | `String`   | MCP resource URI (e.g., `"ui://app/board"`)    |
| `path`     | `PathBuf`  | Absolute path to the `.html` file on disk      |

**Reading:**

```rust
// Read widget HTML from disk -- fresh on every call
let html = widget_dir.read_widget("board");
```

`read_widget` reads from disk on every call. There is no cache. This is intentional -- it enables the hot-reload development workflow described below.

If the file does not exist or cannot be read, `read_widget` returns a styled HTML error page showing the widget name, the file path that was attempted, and the error message. The error page includes a hint: "Create or fix the widget file and refresh the browser to retry."

**Bridge injection:**

```rust
// Insert a <script> tag into widget HTML
let html_with_bridge = WidgetDir::inject_bridge_script(
    &html,
    "/assets/widget-runtime.mjs",
);
```

The injection strategy inserts the script tag just before `</head>` if present, at the start of `<body>` otherwise, or at the very beginning of the document if neither tag is found. This is how the bridge script reaches the widget without the author adding it manually.

### Hot-Reload Development

Because `WidgetDir` reads from disk on every request, the development workflow feels like frontend development:

1. Start your server: `cargo run`
2. Open the preview: `cargo pmcp preview --url http://localhost:3000 --open`
3. Edit your HTML file in `widgets/`
4. Refresh the browser -- your changes appear instantly

No server restart is needed. The server re-reads the file from disk each time a client requests the widget resource. This is safe because widgets are small HTML files and disk I/O is negligible compared to network latency.

### The ResourceHandler Pattern

Every MCP Apps server needs a `ResourceHandler` implementation that connects `WidgetDir` to the MCP resource protocol. The pattern is the same across all shipped examples:

```rust
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}

#[async_trait]
impl ResourceHandler for AppResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra)
        -> Result<ReadResourceResult>
    {
        // 1. Extract widget name from URI
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            // 2. Read HTML from disk (hot-reload)
            let html = self.widget_dir.read_widget(widget_name);

            // 3. Transform for target host (injects bridge script)
            let transformed = self.chatgpt_adapter
                .transform(uri, widget_name, &html);

            Ok(ReadResourceResult {
                contents: vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(
                        ExtendedUIMimeType::HtmlSkybridge.to_string()
                    ),
                }],
            })
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Discover all widgets and map to ResourceInfo
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                description: Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(
                    ExtendedUIMimeType::HtmlSkybridge.to_string()
                ),
            })
            .collect();

        Ok(ListResourcesResult { resources, next_cursor: None })
    }
}
```

The three steps in `read()` are always the same:

1. **Extract widget name** from the `ui://app/{name}` URI
2. **Read from disk** via `widget_dir.read_widget(name)` -- no cache, hot-reload
3. **Transform** via `adapter.transform()` -- injects the bridge script for the target host

The `list()` method calls `widget_dir.discover()` and maps each `WidgetEntry` to a `ResourceInfo` for the MCP protocol.

This pattern is identical in the chess example (`ChessResources`), the map example, and the dataviz example. Once you understand it, you can build any widget-based MCP server.

---

## Bridge Communication

Widgets communicate with the MCP server through the `window.mcpBridge` API. The bridge script is auto-injected by the adapter -- widget authors never write `postMessage` code or manage JSON-RPC framing manually.

### window.mcpBridge API

The bridge exposes these methods:

**Core Operations:**

| Method                        | Returns     | Description                              |
|-------------------------------|-------------|------------------------------------------|
| `mcpBridge.callTool(name, args)` | `Promise`  | Call an MCP tool, get the result         |
| `mcpBridge.readResource(uri)`    | `Promise`  | Read an MCP resource                     |
| `mcpBridge.getPrompt(name, args)`| `Promise`  | Get a prompt                             |
| `mcpBridge.notify(method, params)` | `void`   | Send a notification (fire-and-forget)    |

**State Management (ChatGPT host only):**

| Method                        | Returns     | Description                              |
|-------------------------------|-------------|------------------------------------------|
| `mcpBridge.setState(state)`   | `void`      | Update widget state (persists in session)|
| `mcpBridge.getState()`        | `Object`    | Read current widget state                |

**Communication (ChatGPT host only):**

| Method                           | Returns  | Description                              |
|----------------------------------|----------|------------------------------------------|
| `mcpBridge.sendMessage(message)` | `void`   | Send a follow-up message to the conversation |
| `mcpBridge.openExternal(url)`    | `void`   | Open an external URL                     |

**Display Modes (ChatGPT host only):**

| Method                                | Returns   | Description                             |
|---------------------------------------|-----------|-----------------------------------------|
| `mcpBridge.requestDisplayMode(mode)`  | `Promise` | Request inline, pip, or fullscreen      |
| `mcpBridge.requestClose()`            | `void`    | Close the widget                        |
| `mcpBridge.notifyIntrinsicHeight(h)`  | `void`    | Report the widget's content height      |
| `mcpBridge.setOpenInAppUrl(href)`     | `void`    | Set the "Open in App" button URL        |

**Environment Context (read-only properties, ChatGPT host only):**

| Property                | Type     | Description                              |
|-------------------------|----------|------------------------------------------|
| `mcpBridge.theme`       | `string` | Current theme (`'light'` or `'dark'`)    |
| `mcpBridge.locale`      | `string` | Current locale (e.g., `'en-US'`)         |
| `mcpBridge.displayMode` | `string` | Current display mode                     |
| `mcpBridge.toolInput`   | `object` | Arguments supplied when the tool was invoked |
| `mcpBridge.toolOutput`  | `object` | The structuredContent returned by the tool   |

The core operations (`callTool`, `readResource`, `getPrompt`, `notify`) work across all hosts. The state management, communication, and display mode methods are available when running inside ChatGPT.

### Communication Flow

Here is how a tool call flows through the system:

```
Widget (iframe)                    Host                      MCP Server
     │                              │                            │
     │  mcpBridge.callTool(         │                            │
     │    'hello', { name: 'World' }│                            │
     │  )                           │                            │
     │                              │                            │
     │  ──── bridge script ────►    │                            │
     │       (postMessage or        │                            │
     │        window.openai)        │                            │
     │                              │                            │
     │                              │  ── tools/call ──────►     │
     │                              │     { name: 'hello',       │
     │                              │       arguments: {...} }   │
     │                              │                            │
     │                              │  ◄── result ──────────     │
     │                              │     { greeting: '...' }    │
     │                              │                            │
     │  ◄── response ─────────     │                            │
     │      Promise resolves        │                            │
     │      with { greeting: '...' }│                            │
     ▼                              ▼                            ▼
```

The bridge script handles all the protocol plumbing:

- **ChatGPT host:** Uses `window.openai.callTool()` under the hood
- **MCP Apps host:** Uses `postMessage` with JSON-RPC 2.0 framing
- **MCP-UI host:** Uses `postMessage` with JSON-RPC 2.0 framing

Widget authors write the same `mcpBridge.callTool()` call regardless of which host runs their widget. The adapter selects the correct bridge implementation at serve time.

### Error Handling in Widgets

Always wrap bridge calls in try/catch:

```javascript
async function greet(name) {
    try {
        const result = await window.mcpBridge.callTool('hello', { name });
        document.getElementById('result').textContent = result.greeting;
    } catch (err) {
        document.getElementById('result').textContent =
            'Error: ' + (err.message || String(err));
    }
}
```

If your widget needs to wait for the bridge to initialize before making calls, listen for the `mcpBridgeReady` event:

```javascript
// Wait for bridge initialization
window.addEventListener('mcpBridgeReady', () => {
    // Bridge is ready -- safe to call mcpBridge methods
    loadInitialData();
});

// If bridge is already ready (script loaded synchronously)
if (window.mcpBridge) {
    loadInitialData();
}
```

The `mcpBridgeReady` event is dispatched by the injected bridge script after it sets up `window.mcpBridge`. In most cases the bridge is available immediately, but the event pattern is useful for widgets that load asynchronously.

---

<!-- CONTINUED IN PLAN 21-02: Adapter Pattern and Example Walkthroughs -->
