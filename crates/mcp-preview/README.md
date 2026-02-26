# mcp-preview

Browser-based preview server for MCP Apps widgets.

## Features

- **Widget Preview**: Renders MCP Apps widgets in an iframe with live MCP bridge
- **Dual Bridge Modes**: Proxy bridge (default) routes through HTTP; WASM bridge connects directly
- **DevTools Panel**: Real-time bridge call logging with timing, expandable details, and badge counts
- **Resource Picker**: Switch between multiple UI resources when the server exposes more than one
- **Connection Lifecycle**: Status indicator (connected/disconnected/reconnecting) with reconnect button
- **Hot Reload**: File-based widgets reload on browser refresh without server restart
- **WASM Builder**: Automated wasm-pack build orchestration with artifact caching

## Usage

```bash
# Preview a running MCP server's widgets
cargo pmcp preview --url http://localhost:3000 --open

# With file-based widgets directory
cargo pmcp preview --url http://localhost:3000 --widgets-dir ./widgets --open
```

## Architecture

- `proxy.rs` — Session-persistent MCP proxy with RwLock double-checked locking
- `handlers/api.rs` — REST endpoints: resources, reconnect, status, widget serving
- `handlers/wasm.rs` — WASM build trigger, status, and artifact serving
- `wasm_builder.rs` — Async wasm-pack orchestration with status tracking
- `assets/index.html` — Preview UI with AppBridge from shared widget-runtime library
- `assets/widget-runtime.mjs` — Compiled ESM bridge library (embedded via rust_embed)

## Bridge Modes

| Mode | How It Works | When to Use |
|------|-------------|-------------|
| **Proxy** (default) | Widget → postMessage → host → HTTP fetch → MCP server | Always works, no build step |
| **WASM** | Widget → postMessage → host → WASM client → MCP server | Direct connection, requires wasm-pack |
