# MCP Apps Preview - Manual UI Testing

## Overview

The MCP Apps Preview system provides a browser-based development environment for manually testing MCP servers that return widget UI. It simulates the ChatGPT Apps runtime environment, allowing developers to:

- See widgets render in a browser
- Call tools interactively and observe responses
- Test bridge APIs (`callTool`, `setState`, theme switching, etc.)
- Debug with DevTools (state viewer, console, network log)
- Iterate quickly without complex setup

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Developer Workflow                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Terminal 1                    Terminal 2                           │
│  ┌────────────────┐           ┌────────────────────────────────┐   │
│  │ cargo pmcp dev │           │ cargo pmcp preview             │   │
│  │ --server chess │           │ --url http://localhost:3000    │   │
│  │                │           │ --open                         │   │
│  │ MCP Server     │◄──HTTP───►│ Preview Server                 │   │
│  │ (port 3000)    │           │ (port 8765)                    │   │
│  └────────────────┘           └───────────────┬────────────────┘   │
│                                               │                     │
│                                               │ WebSocket           │
│                                               ▼                     │
│                               ┌────────────────────────────────┐   │
│                               │          Browser               │   │
│                               │  ┌──────────────────────────┐  │   │
│                               │  │    Preview Environment   │  │   │
│                               │  │    - Tool Panel          │  │   │
│                               │  │    - Widget iframe       │  │   │
│                               │  │    - DevTools            │  │   │
│                               │  │    - Environment Controls│  │   │
│                               │  └──────────────────────────┘  │   │
│                               └────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## CLI Usage

### Basic Usage

```bash
# Start your MCP server (in one terminal)
cargo pmcp dev --server chess

# Start preview (in another terminal)
cargo pmcp preview --url http://localhost:3000 --open
```

### Command Options

```bash
cargo pmcp preview [OPTIONS]

OPTIONS:
    --url <URL>           URL of running MCP server [required]
    --port <PORT>         Preview server port [default: 8765]
    --open                Open browser automatically
    --tool <NAME>         Auto-select this tool on start
    --theme <THEME>       Initial theme: light, dark [default: light]
    --locale <LOCALE>     Initial locale [default: en-US]
```

### Examples

```bash
# Preview chess server with dark theme
cargo pmcp preview --url http://localhost:3000 --theme dark --open

# Preview and auto-select a specific tool
cargo pmcp preview --url http://localhost:3000 --tool show_chess_board --open

# Preview on custom port
cargo pmcp preview --url http://localhost:3000 --port 9000
```

## Preview Interface

```
┌─────────────────────────────────────────────────────────────────────┐
│  MCP Apps Preview                                    [Light] [Dark] │
├─────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────┐ ┌───────────────────────┐ ┌──────────────────┐ │
│ │  Tool Panel     │ │   Widget Preview      │ │  DevTools Panel  │ │
│ │                 │ │                       │ │                  │ │
│ │ Server: ✓       │ │ ┌───────────────────┐ │ │ [State] [Console]│ │
│ │ Connected       │ │ │                   │ │ │ [Network][Events]│ │
│ │                 │ │ │                   │ │ │                  │ │
│ │ Tools:          │ │ │   Widget iframe   │ │ │ {                │ │
│ │ ┌─────────────┐ │ │ │   (isolated)      │ │ │   "widgetState": │ │
│ │ │show_board  ○│ │ │ │                   │ │ │   {},            │ │
│ │ │make_move   ●│ │ │ │                   │ │ │   "toolInput":   │ │
│ │ │get_state   ○│ │ │ │                   │ │ │   {"from":"e2"}  │ │
│ │ └─────────────┘ │ │ │                   │ │ │ }                │ │
│ │                 │ │ └───────────────────┘ │ │                  │ │
│ │ Arguments:      │ │                       │ │ ────────────────  │ │
│ │ ┌─────────────┐ │ │ Environment:          │ │ 12:34:56 callTool│ │
│ │ │{            │ │ │ [Inline▼] [en-US▼]   │ │ > make_move      │ │
│ │ │  "from":"e2"│ │ │ [600px▼] [Safe Area] │ │ < {success:true} │ │
│ │ │  "to": "e4" │ │ │                       │ │                  │ │
│ │ │}            │ │ │                       │ │                  │ │
│ │ └─────────────┘ │ │                       │ │                  │ │
│ │                 │ │                       │ │                  │ │
│ │ [▶ Execute]     │ │                       │ │                  │ │
│ └─────────────────┘ └───────────────────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Tool Panel

- **Server Status:** Connection indicator
- **Tool List:** Available tools from the server (radio selection)
- **Argument Editor:** JSON editor for tool arguments
- **Execute Button:** Call the selected tool

### Widget Preview

- **Iframe Container:** Isolated widget rendering
- **Environment Controls:**
  - Display Mode: Inline / PiP / Fullscreen
  - Locale: Language selector
  - Max Height: Slider for height constraints
  - Safe Area: Mobile device simulation

### DevTools Panel

Four tabs for debugging:

1. **State Tab:** Live JSON view of:
   - `widgetState` - Current widget state
   - `toolInput` - Arguments passed to tool
   - `toolOutput` - Structured content from tool
   - `toolResponseMetadata` - `_meta` payload

2. **Console Tab:** Widget `console.log/warn/error` output

3. **Network Tab:** MCP message log with:
   - Request/response bodies
   - Timing information
   - Success/failure status

4. **Events Tab:** Lifecycle events:
   - Widget ready
   - State updates
   - Display mode changes
   - Bridge method calls

## Bridge Simulation

The preview injects a full `window.mcpBridge` and `window.openai` implementation:

```javascript
window.mcpBridge = {
  // Core functionality
  callTool: async (name, args) => { /* proxy to MCP server */ },
  getState: () => widgetState,
  setState: (state) => { /* update and persist */ },

  // Tool context (populated from tool response)
  get toolInput() { return currentToolInput; },
  get toolOutput() { return currentToolOutput; },
  get toolResponseMetadata() { return currentMeta; },

  // Communication
  sendMessage: (msg) => { /* log to devtools */ },
  openExternal: (url) => window.open(url, '_blank'),

  // File operations (simulated)
  uploadFile: (file) => { /* mock file upload */ },
  getFileDownloadUrl: (id) => { /* mock download URL */ },

  // Display modes (controlled by UI)
  requestDisplayMode: (mode) => { /* update preview */ },
  requestClose: () => { /* close widget */ },
  notifyIntrinsicHeight: (h) => { /* update container */ },

  // Environment (reactive to controls)
  get theme() { return currentTheme; },
  get locale() { return currentLocale; },
  get displayMode() { return currentDisplayMode; },
  get maxHeight() { return currentMaxHeight; },
  get safeArea() { return currentSafeArea; },
  get view() { return currentView; },
  get userAgent() { return navigator.userAgent; },
};

// ChatGPT compatibility
window.openai = window.mcpBridge;
```

When environment controls change, the preview dispatches `openai/setGlobals` events to the widget:

```javascript
window.dispatchEvent(new CustomEvent('openai/setGlobals', {
  detail: {
    globals: { theme, locale, displayMode, maxHeight, safeArea, view }
  }
}));
```

## Implementation

### Crate Structure

```
crates/mcp-preview/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── server.rs           # Axum HTTP server
│   ├── proxy.rs            # MCP HTTP client
│   ├── assets.rs           # Embedded static files (rust-embed)
│   └── handlers/
│       ├── mod.rs
│       ├── page.rs         # Serve preview HTML
│       ├── api.rs          # Tool listing and calling
│       └── websocket.rs    # Live updates
└── assets/
    ├── index.html          # Main preview page
    ├── preview.js          # Preview runtime
    ├── bridge.js           # Widget bridge simulator
    ├── devtools.js         # DevTools functionality
    └── styles.css          # Preview styling
```

### Dependencies

```toml
[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "fs"] }
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.11", features = ["json"] }
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
```

### Server Implementation

```rust
pub struct PreviewServer {
    config: PreviewConfig,
}

impl PreviewServer {
    pub async fn start(config: PreviewConfig) -> Result<()> {
        let state = Arc::new(AppState {
            mcp_url: config.mcp_url.clone(),
            client: reqwest::Client::new(),
        });

        let app = Router::new()
            .route("/", get(handlers::page::index))
            .route("/api/tools", get(handlers::api::list_tools))
            .route("/api/tools/call", post(handlers::api::call_tool))
            .route("/ws", get(handlers::websocket::handler))
            .nest_service("/assets", ServeEmbedded::<Assets>::new())
            .layer(CorsLayer::permissive())
            .with_state(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
        println!("Preview server running at http://{}", addr);

        axum::serve(
            TcpListener::bind(addr).await?,
            app.into_make_service()
        ).await?;

        Ok(())
    }
}
```

### CLI Integration

Add to `cargo-pmcp/src/commands/`:

```rust
// preview.rs
#[derive(Parser)]
pub struct PreviewCommand {
    /// URL of running MCP server
    #[arg(long)]
    url: String,

    /// Port for preview server
    #[arg(long, default_value = "8765")]
    port: u16,

    /// Open browser automatically
    #[arg(long)]
    open: bool,

    /// Specific tool to auto-select
    #[arg(long)]
    tool: Option<String>,

    /// Initial theme
    #[arg(long, default_value = "light")]
    theme: String,

    /// Initial locale
    #[arg(long, default_value = "en-US")]
    locale: String,
}

impl PreviewCommand {
    pub async fn execute(&self) -> Result<()> {
        let config = PreviewConfig {
            mcp_url: self.url.clone(),
            port: self.port,
            initial_tool: self.tool.clone(),
            theme: self.theme.clone(),
            locale: self.locale.clone(),
        };

        if self.open {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = open::that(format!("http://localhost:{}", config.port));
            });
        }

        mcp_preview::PreviewServer::start(config).await
    }
}
```

## Development Workflow

### Typical Session

1. **Start MCP Server:**
   ```bash
   cargo pmcp dev --server chess
   ```

2. **Start Preview:**
   ```bash
   cargo pmcp preview --url http://localhost:3000 --open
   ```

3. **Test Widget:**
   - Select a tool from the list
   - Enter arguments in JSON editor
   - Click "Execute"
   - Widget renders in iframe

4. **Test Interactions:**
   - Click on widget elements (e.g., chess pieces)
   - Observe tool calls in Network tab
   - Check state updates in State tab

5. **Test Environment:**
   - Toggle dark mode
   - Change locale
   - Switch display modes
   - Adjust height constraints

6. **Debug Issues:**
   - Check Console tab for errors
   - Inspect Network tab for failed calls
   - Review State tab for unexpected values

### Hot Reload

When a tool returns new widget HTML, the preview automatically:
- Updates the iframe content
- Preserves widget state
- Maintains environment settings
- Logs the transition in Events tab

## Future Enhancements

### Automated Testing (Phase 2)

Integration with Playwright for CI:

```bash
cargo pmcp test ui --url http://localhost:3000 --scenario chess_game
```

This will:
1. Start preview server programmatically
2. Run Playwright tests against the preview
3. Generate visual regression reports
4. Support CI environments

### Device Frames

Simulate specific devices:

```bash
cargo pmcp preview --url http://localhost:3000 --device "iPhone 15"
```

### State Snapshots

Save and restore widget state for testing:

```bash
# Save current state
cargo pmcp preview snapshot save my-state.json

# Restore state
cargo pmcp preview snapshot load my-state.json
```

### Multi-Widget Testing

Test interactions between multiple widgets:

```bash
cargo pmcp preview --url http://localhost:3000 --multi
```

## Related Documentation

- [ChatGPT Apps Integration](./chatgpt-apps-integration.md)
- [Widget Runtime Package](../../packages/widget-runtime/README.md)
- [MCP Apps Implementation](../mcp-apps-implementation.md)
- [Testing with MCP Tester](../TESTING_WITH_MCP_TESTER.md)
