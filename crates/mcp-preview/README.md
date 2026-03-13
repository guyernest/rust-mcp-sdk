# mcp-preview

Browser-based development environment for testing and debugging MCP Apps widgets. Renders your widgets in an isolated iframe with a live MCP bridge, DevTools diagnostics, and multi-host preview modes.

## Features

- **Widget Preview**: Renders MCP Apps widgets in an iframe with a live MCP bridge connected to your server
- **Dual Bridge Modes**: Proxy bridge (default) routes widget calls through HTTP; WASM bridge connects directly to the MCP server
- **Multi-Host Preview**: Standard mode (default) for general MCP hosts; `--mode chatgpt` for ChatGPT strict protocol emulation with `window.openai` API stub
- **DevTools Panel**: Real-time bridge call logging with timing, expandable request/response details, and badge counts
- **Protocol Tab**: Metadata compliance checks for `_meta`, `ui.resourceUri`, `openai/*` keys, `structuredContent`, MIME types, and CSP
- **Bridge Diagnostics Tab**: PostMessage traffic inspector with handshake trace and message-level detail
- **Resource Picker**: Switch between multiple UI resources when the server exposes more than one widget
- **Connection Lifecycle**: Status indicator (connected/disconnected/reconnecting) with manual reconnect button
- **Environment Controls**: Theme toggle (light/dark), locale selection, display mode switching
- **Hot Reload**: File-based widgets reload on browser refresh without server restart (via `--widgets-dir`)
- **Theme CSS Variables**: Sends `styles.variables` in host context for ext-apps widget theming via CSS custom properties
- **WASM Builder**: Automated wasm-pack build orchestration with artifact caching
- **ChatGPT API Stub**: In ChatGPT mode, provides a `window.openai` stub so widgets using the OpenAI Apps SDK work without a real ChatGPT host

## Installation

### Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/paiml/rust-mcp-sdk/releases). Available platforms: Linux x86_64, macOS x86_64, Windows x86_64.

```bash
# Linux
curl -L https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-preview-linux-x86_64 -o mcp-preview && chmod +x mcp-preview

# macOS
curl -L https://github.com/paiml/rust-mcp-sdk/releases/latest/download/mcp-preview-macos-x86_64 -o mcp-preview && chmod +x mcp-preview
```

### Via Cargo

```bash
cargo install mcp-preview
```

### Via cargo-pmcp

Install the PMCP CLI toolkit, which includes a `cargo pmcp preview` wrapper:

```bash
cargo install cargo-pmcp
# Then use: cargo pmcp preview --url <URL> [OPTIONS]
```

## Usage

```bash
# Preview a running MCP server's widgets
mcp-preview --url http://localhost:3000

# Open browser automatically
mcp-preview --url http://localhost:3000 --open

# ChatGPT strict protocol mode
mcp-preview --url http://localhost:3000 --mode chatgpt --open

# With file-based widgets directory (hot-reload)
mcp-preview --url http://localhost:3000 --widgets-dir ./widgets --open

# Custom port
mcp-preview --url http://localhost:3000 --port 9000 --open
```

When using the `cargo pmcp` wrapper:

```bash
cargo pmcp preview --url http://localhost:3000 --open
cargo pmcp preview --url http://localhost:3000 --mode chatgpt --open
```

### Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--url <URL>` | URL of the target MCP server | `http://localhost:3000` |
| `--mode <MODE>` | Preview mode: `standard` or `chatgpt` | `standard` |
| `--open` | Open browser automatically on start | off |
| `--widgets-dir <PATH>` | Directory containing widget `.html` files for file-based authoring (hot-reload) | none |
| `--port <PORT>` | Port for the preview server | `8765` |

## DevTools Tabs

The preview UI includes a DevTools panel at the bottom of the page with three tabs for inspecting widget behavior:

### Bridge Tab

Logs every MCP bridge call between the widget and the server in real time. Each entry shows:

- Method name (e.g., `tools/call`, `resources/read`)
- Call timing (duration in milliseconds)
- Expandable request/response details with full JSON payloads
- Badge count of total calls

Use this tab to verify your widget is making the expected MCP calls and receiving correct responses.

### Protocol Tab

Runs metadata compliance checks against your server's `tools/list` and `resources/read` responses. Checks include:

| Check | What it validates |
|-------|-------------------|
| `tools/list` has `_meta.ui` | `{ "ui": { "resourceUri": "ui://..." } }` present |
| `tools/list` has openai keys | `openai/outputTemplate`, `openai/widgetAccessible`, etc. (ChatGPT mode) |
| `tools/call` returns `structuredContent` | JSON data object alongside text content |
| `tools/call` has `_meta` | `openai/toolInvocation/*` keys (ChatGPT mode) |
| `resources/read` mimeType | `"text/html;profile=mcp-app"` (standard MIME) |
| `resources/read` has `_meta` | `ui/resourceUri` + openai keys propagated |
| `resources/read` has CSP | `_meta.ui.csp.resourceDomains` / `connectDomains` (if external resources) |

All checks should show PASS for a correctly configured MCP Apps server.

### Bridge Diagnostics Tab

Inspects the raw PostMessage traffic between the preview host and the widget iframe. Shows:

- Every `postMessage` sent and received with timestamps
- Handshake trace (`ui/initialize` request and response)
- Message-level detail for debugging widget connection issues

## Bridge Modes

| Mode | How It Works | When to Use |
|------|-------------|-------------|
| **Proxy** (default) | Widget -> postMessage -> host -> HTTP fetch -> MCP server | Always works, no build step |
| **WASM** | Widget -> postMessage -> host -> WASM client -> MCP server | Direct connection, requires wasm-pack |

## Multi-Host Preview

### Standard Mode (default)

Standard MCP Apps preview mode. The host provides an AppBridge that implements the ext-apps protocol (`ui/initialize`, `ui/toolResult`, `ui/teardown`). Widgets use the `@modelcontextprotocol/ext-apps` SDK's `App` class to communicate.

```bash
mcp-preview --url http://localhost:3000 --open
```

### ChatGPT Mode

Activates ChatGPT strict protocol emulation. In this mode, mcp-preview:

- Provides a `window.openai` API stub so widgets using the OpenAI Apps SDK work correctly
- Enriches tool and resource `_meta` with ChatGPT-specific `openai/*` keys
- Enables stricter Protocol tab checks (validates `openai/outputTemplate`, `openai/widgetAccessible`, and other ChatGPT-required keys)
- Emulates ChatGPT's host context delivery pattern

Use this mode to verify your widgets work in ChatGPT before deploying:

```bash
mcp-preview --url http://localhost:3000 --mode chatgpt --open
```

## Architecture

- `server.rs` -- Preview server with Axum router, `PreviewMode` enum, and `PreviewConfig`
- `proxy.rs` -- Session-persistent MCP proxy with RwLock double-checked locking
- `handlers/api.rs` -- REST endpoints: resources, reconnect, status, widget serving
- `handlers/wasm.rs` -- WASM build trigger, status, and artifact serving
- `handlers/page.rs` -- Main preview page serving
- `handlers/websocket.rs` -- WebSocket for live updates
- `wasm_builder.rs` -- Async wasm-pack orchestration with status tracking
- `assets/index.html` -- Preview UI with AppBridge from shared widget-runtime library
- `assets/widget-runtime.mjs` -- Compiled ESM bridge library (embedded via rust_embed)

## License

MIT - See LICENSE file in the repository root.
