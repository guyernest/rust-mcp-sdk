# Phase 14: Preview Bridge Infrastructure - Research

**Researched:** 2026-02-24
**Domain:** MCP preview server - widget iframe rendering, bridge proxy, session management, DevTools integration
**Confidence:** HIGH

## Summary

Phase 14 transforms the existing `mcp-preview` crate from a tool-execution UI into a full widget preview environment. The crate already has approximately 80% of the infrastructure: an Axum HTTP server with routes for tool listing/calling, a JSON-RPC proxy (`McpProxy`), embedded static assets via `rust-embed`, a WebSocket handler, a complete DevTools panel (State/Console/Network/Events tabs), and a bridge injection system (`wrapWidgetHtml()`) that provides `window.mcpBridge` inside iframes. What is missing is: (1) `resources/list` and `resources/read` proxy methods so the preview can fetch widget HTML from the MCP server, (2) session persistence so `McpProxy` does not call `initialize()` on every request, (3) a resource picker UI in the sidebar, (4) automatic widget loading on startup, (5) bridge call logging with badge counts in the Network tab, (6) a connection status indicator with reconnect capability, and (7) postMessage origin hardening.

The existing bridge in `index.html` uses direct parent frame access (`window.parent.previewRuntime`) rather than postMessage. This works because the iframe uses `srcdoc` (same origin). The CONTEXT.md decisions confirm staying with `srcdoc` wrapping for resource-loaded widgets, which means the current same-origin bridge approach can be preserved and extended. The postMessage wildcard origin issue flagged in STATE.md applies specifically to the `McpAppsAdapter` and `McpUiAdapter` bridge scripts in `src/server/mcp_apps/adapter.rs` -- the preview's own bridge does not use postMessage at all (it accesses `window.parent.previewRuntime` directly). However, the `emitGlobalsUpdate()` call in `index.html` line 1004 does use `frame.contentWindow.postMessage({...}, '*')` which should use the iframe's actual origin or `'*'` is acceptable for same-origin srcdoc iframes (srcdoc iframes have origin `'null'`, so `'*'` is the only valid target -- this is not a security issue for srcdoc).

**Primary recommendation:** Extend `McpProxy` with `list_resources()` and `read_resource()` methods, add `OnceLock`-based session persistence, add `/api/resources` and `/api/resources/read` API routes, modify the frontend to auto-load the first UI resource on startup, add a resource picker above the tool list, enhance the Network tab with bridge call logging and badge counts, and add a reconnect button to the header.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Auto-load the first UI resource on startup -- developer runs `cargo pmcp preview` and immediately sees their widget rendered
- Fetch widget HTML via `resources/read` JSON-RPC proxy call to the MCP server (PREV-04)
- On startup: call `resources/list`, filter to UI resources (HTML MIME types), auto-load the first one into the iframe
- Existing tool panel (left sidebar with tool list, args editor, execute button) stays visible alongside the resource-based widget
- When a widget's bridge `callTool()` returns HTML content, the iframe auto-replaces with the new HTML (current behavior preserved)
- Resource picker sits at the top of the left sidebar, above the existing tool list
- Shows name + description for each resource entry
- Only shows UI resources (HTML/widget MIME types) -- non-UI resources are filtered out
- When server has only one UI resource: hide the picker, show just a resource name label
- When multiple UI resources exist: show the picker list, clicking switches the iframe to that resource
- Keep all 4 existing devtools tabs: State, Console, Network, Events
- Badge count on the Network tab when new bridge calls happen (non-intrusive notification)
- Each bridge call log entry shows: tool name, arguments sent, response content, and duration in ms -- full request/response pair, expandable/collapsible
- Per-tab clear buttons for Console, Network, and Events tabs
- Initialize MCP session once on preview server startup, reuse across all subsequent requests (PREV-03)
- Minimal status display -- just connected/disconnected dot in the header, no session ID or duration
- When MCP server is unreachable: inline error message in the widget area with a Retry button
- Reconnect button in the header to re-initialize session and refresh tools/resources without restarting the preview server

### Claude's Discretion
- Exact CSS styling for resource picker entries, badge count, and error states
- Network tab expandable/collapsible entry implementation details
- How the reconnect flow handles in-flight bridge calls
- Session initialization error handling and retry logic internals
- Bridge injection approach for resource-loaded widgets (srcdoc wrapping vs other methods)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PREV-01 | Developer can preview widget in iframe rendered from MCP resource URI via `cargo pmcp preview` | Auto-load first UI resource on startup; `resources/list` + `resources/read` proxy methods; `srcdoc` iframe wrapping with bridge injection |
| PREV-02 | Widget `window.mcpBridge.callTool()` calls route to real MCP server through preview proxy | Existing bridge already routes `callTool()` through `/api/tools/call`; extend to log bridge calls in Network tab |
| PREV-03 | MCP proxy initializes session once and reuses across all subsequent requests | Replace per-request `initialize()` with `OnceLock<Value>` or `tokio::sync::OnceCell`; store session ID from init response; forward `Mcp-Session-Id` header |
| PREV-04 | Preview fetches widget HTML via `resources/read` proxy call to MCP server | New `McpProxy::read_resource(uri)` method sending `resources/read` JSON-RPC; new `McpProxy::list_resources()` method sending `resources/list` JSON-RPC |
| PREV-05 | DevTools panel updates in real time when bridge calls are made | Bridge `callTool()` already logs to Network tab; enhance with expandable/collapsible entries, badge count, duration, and per-tab clear buttons |
| PREV-06 | Connection status indicator shows connected/disconnected state | Status dot already exists in header (`#status-dot`); wire to actual MCP session state; add Reconnect button |
| PREV-07 | Resource picker shows multiple UI resources when server exposes more than one | New resource picker component above tool list; filter by HTML MIME types; single-resource label mode; multi-resource picker mode |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.7 (mcp-preview) | HTTP server for preview | Already in use; 0.7 is what mcp-preview depends on today. Upgrading to 0.8 is optional and not required for Phase 14. |
| tokio | 1.46 | Async runtime | Already in workspace |
| reqwest | 0.12 | HTTP client for MCP proxy | Already in mcp-preview Cargo.toml |
| rust-embed | 8 | Embedded static assets | Already in use for `assets/index.html` |
| serde / serde_json | 1 | JSON serialization | Already in use |
| tower-http | 0.6 | CORS middleware | Already in use with `cors` feature |
| uuid | 1 | Request ID generation | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::sync::OnceCell | (part of tokio 1) | Session initialization singleton | For one-time MCP session init with async support |
| tracing | 0.1 | Structured logging | Already in use for server logging |
| anyhow | 1 | Error handling | Already in use |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tokio::sync::OnceCell` for session | `std::sync::OnceLock` | OnceLock does not support async initialization; OnceCell does. Since `initialize()` is async (HTTP call), OnceCell is required. |
| Axum 0.7 (current) | Axum 0.8 (root crate) | 0.8 changes route syntax and WebSocket types. Not required for Phase 14 functionality. Can upgrade in a future phase to eliminate duplicate compilation. |
| `srcdoc` iframe loading | Separate `/widget-proxy` endpoint | srcdoc is simpler (no extra route, no CORS issues, same-origin access). The proxy endpoint approach would be needed if widgets load external assets, but for HTML-only widgets srcdoc is sufficient and matches existing behavior. |

**No new dependencies needed for Phase 14.** All required libraries are already in `crates/mcp-preview/Cargo.toml`.

## Architecture Patterns

### Recommended Project Structure
```
crates/mcp-preview/
├── src/
│   ├── lib.rs              # Public API (unchanged)
│   ├── server.rs           # PreviewServer, AppState, routes (modified: add resource routes, session state)
│   ├── proxy.rs            # McpProxy (modified: add session persistence, resources/list, resources/read)
│   ├── assets.rs           # Embedded assets (unchanged)
│   └── handlers/
│       ├── mod.rs          # Handler module (modified: add resource handlers)
│       ├── api.rs          # API handlers (modified: add resource endpoints)
│       ├── assets.rs       # Static asset handler (unchanged)
│       ├── page.rs         # Main page handler (unchanged)
│       └── websocket.rs    # WebSocket handler (unchanged)
├── assets/
│   └── index.html          # Preview UI (modified: add resource picker, enhance devtools, add reconnect)
└── Cargo.toml              # Dependencies (unchanged)
```

### Pattern 1: Session-Once Initialization with OnceCell
**What:** Initialize MCP session exactly once using `tokio::sync::OnceCell`, storing the initialize response. All subsequent proxy calls reuse the session.
**When to use:** When the MCP server requires session initialization before accepting other requests, and re-initialization is wasteful.
**Example:**
```rust
// In McpProxy
use tokio::sync::OnceCell;

pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session: OnceCell<SessionInfo>,
}

struct SessionInfo {
    session_id: Option<String>,
    server_info: Value,
}

impl McpProxy {
    pub async fn ensure_initialized(&self) -> Result<&SessionInfo> {
        self.session.get_or_try_init(|| async {
            let result = self.send_request("initialize", Some(init_params())).await?;
            // Send initialized notification
            let _ = self.send_notification("notifications/initialized").await;
            let session_id = /* extract from response headers if present */;
            Ok(SessionInfo { session_id, server_info: result })
        }).await
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let _ = self.ensure_initialized().await?;
        let result = self.send_request("tools/list", None).await?;
        // ... parse tools
    }

    pub async fn list_resources(&self) -> Result<Vec<ResourceInfo>> {
        let _ = self.ensure_initialized().await?;
        let result = self.send_request("resources/list", None).await?;
        // ... parse resources
    }

    pub async fn read_resource(&self, uri: &str) -> Result<ResourceReadResult> {
        let _ = self.ensure_initialized().await?;
        let params = json!({ "uri": uri });
        let result = self.send_request("resources/read", Some(params)).await?;
        // ... parse resource contents
    }
}
```

### Pattern 2: UI Resource Filtering
**What:** Filter `resources/list` results to only show UI resources (HTML MIME types) in the resource picker.
**When to use:** When the MCP server exposes both data resources and UI resources, and only UI resources should appear in the widget picker.
**Example:**
```rust
// In proxy.rs
pub fn is_ui_resource(resource: &ResourceInfo) -> bool {
    resource.mime_type.as_deref().map_or(false, |mime| {
        mime.contains("html")
            || mime == "text/html+skybridge"
            || mime == "text/html+mcp"
    })
}

// In api.rs handler
pub async fn list_resources(State(state): State<Arc<AppState>>) -> Json<ResourcesResponse> {
    match state.proxy.list_resources().await {
        Ok(resources) => {
            let ui_resources: Vec<_> = resources
                .into_iter()
                .filter(|r| is_ui_resource(r))
                .collect();
            Json(ResourcesResponse { resources: ui_resources, error: None })
        }
        Err(e) => Json(ResourcesResponse { resources: vec![], error: Some(e.to_string()) }),
    }
}
```

### Pattern 3: srcdoc Widget Loading with Bridge Injection
**What:** Fetch widget HTML via the proxy, wrap it with the bridge script, and set it as `iframe.srcdoc`. This gives same-origin access between the preview UI and the widget iframe.
**When to use:** For all resource-loaded widgets in the preview environment.
**Example:**
```javascript
// In index.html PreviewRuntime
async loadResourceWidget(uri) {
    try {
        const response = await fetch(`/api/resources/read?uri=${encodeURIComponent(uri)}`);
        const data = await response.json();

        if (data.error) {
            this.showWidgetError(data.error);
            return;
        }

        // Find HTML content in resource response
        const htmlContent = data.contents?.find(c =>
            c.mime_type && c.mime_type.includes('html')
        );

        if (htmlContent && htmlContent.text) {
            this.loadWidget(htmlContent.text);
        } else {
            this.showWidgetError('Resource does not contain HTML content');
        }
    } catch (e) {
        this.showWidgetError(`Failed to load resource: ${e.message}`);
    }
}
```

### Pattern 4: Reconnect Flow
**What:** A reconnect button in the header that resets the session state and re-fetches tools/resources.
**When to use:** When the developer restarts their MCP server and wants to reconnect without restarting the preview.
**Example:**
```rust
// In McpProxy -- add a reset method
impl McpProxy {
    pub async fn reset_session(&self) {
        // OnceCell doesn't have reset, so we need a different approach:
        // Use RwLock<Option<SessionInfo>> instead of OnceCell for resettable session
    }
}
```

**Note on OnceCell vs RwLock for session:** Since the user requires a Reconnect button (which resets the session), `OnceCell` is insufficient because it cannot be reset after initialization. Use `tokio::sync::RwLock<Option<SessionInfo>>` instead, with a `ensure_initialized()` that checks the RwLock and initializes if None, and a `reset()` that clears it back to None.

### Anti-Patterns to Avoid
- **Re-initializing per request:** The current `list_tools()` calls `self.initialize().await` every time. This creates a new MCP session on every tool list request. Must be fixed.
- **Direct parent frame access from resource widgets:** The current bridge uses `window.parent.previewRuntime` which works for srcdoc iframes but would break if the iframe origin changed. Since we are staying with srcdoc (per CONTEXT.md), this is acceptable but should be documented.
- **Storing session ID in AtomicU64:** Session IDs are opaque strings, not numbers. Store as `Option<String>` in the session info struct.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async singleton initialization | Custom locking with Mutex | `tokio::sync::RwLock<Option<SessionInfo>>` with init-if-None pattern | Handles concurrent init attempts safely; RwLock allows read access during normal operation |
| JSON-RPC request/response matching | Custom protocol parser | Extend existing `McpProxy::send_request()` | Already handles JSON-RPC 2.0 correctly; just add new method names |
| MIME type detection for UI resources | Custom MIME parser | String matching on known UI MIME types | Only 3-4 known UI MIME types (`text/html`, `text/html+skybridge`, `text/html+mcp`); no need for full MIME parsing |
| Badge count UI component | Custom notification system | Simple counter + CSS pseudo-element or span | Browser devtools pattern; just a number in a circle |
| Expandable/collapsible log entries | Custom accordion widget | `<details>/<summary>` HTML elements | Native browser element; no JavaScript needed for expand/collapse |

**Key insight:** This phase is entirely about wiring existing components together. The proxy, the bridge, the iframe, the DevTools panel -- all exist. The work is adding new proxy methods, new API routes, and enhancing the frontend JavaScript. No new architectural components are needed.

## Common Pitfalls

### Pitfall 1: McpProxy Re-initialization Per Request
**What goes wrong:** `list_tools()` calls `self.initialize().await` before every `tools/list` request. Each initialize creates a new MCP session. Stateful servers may reset state. Performance degrades with unnecessary RTTs.
**Why it happens:** The original implementation treated each proxy method as independent, not sharing session state.
**How to avoid:** Initialize once using `RwLock<Option<SessionInfo>>`. All proxy methods call `ensure_initialized()` first. The reconnect endpoint calls `reset_session()` then `ensure_initialized()`.
**Warning signs:** Multiple `initialize` calls in the MCP server logs for a single page load.

### Pitfall 2: Missing `notifications/initialized` After Initialize
**What goes wrong:** The MCP protocol requires clients to send a `notifications/initialized` notification after receiving the `initialize` response. Omitting this may cause some servers to reject subsequent requests.
**Why it happens:** Easy to forget since it is a notification (no response expected).
**How to avoid:** Send the notification immediately after successful `initialize()` in the `ensure_initialized()` path.
**Warning signs:** Server returns errors on first `tools/list` call after initialize.

### Pitfall 3: Resource Read Returns Wrapped Content Structure
**What goes wrong:** The `resources/read` response has a `contents` array containing `Content` items (with `type`, `uri`, `text`, `mimeType` fields). Developers may expect raw HTML directly.
**Why it happens:** MCP protocol wraps resource content in a structured envelope.
**How to avoid:** Parse the `contents` array, find the item with HTML MIME type, extract the `text` field. The proxy should handle this parsing and return a clean `ResourceReadResult` struct.
**Warning signs:** Widget iframe shows `[object Object]` instead of rendered HTML.

### Pitfall 4: HTML Double-Injection with `str::replace`
**What goes wrong:** The `wrapWidgetHtml()` function in `index.html` uses string concatenation to wrap HTML. If the widget HTML already contains bridge code (e.g., from `ChatGptAdapter::inject_bridge()`), the widget ends up with two bridges.
**Why it happens:** Resources from the MCP server may already have adapter-injected bridges if the server uses `ChatGptAdapter::transform()`.
**How to avoid:** The preview bridge should be authoritative. The `wrapWidgetHtml()` function already wraps the HTML in a completely new document structure (new `<html>`, `<head>`, `<body>`), so any existing bridge in the original HTML's `<head>` is preserved but the preview's bridge takes precedence. Since both define `window.mcpBridge`, the preview's definition (in the `<head>` of the wrapper) executes first. This is acceptable -- the preview bridge wins.
**Warning signs:** Two `window.mcpBridge` definitions in widget iframe. The last one to define properties wins, but this could cause subtle bugs if one bridge partially initializes before the other overwrites it.

### Pitfall 5: Reconnect Race Condition
**What goes wrong:** User clicks Reconnect while a bridge `callTool()` is in-flight. The session resets, the in-flight request uses the old session, and the response either fails or goes to the wrong session.
**Why it happens:** Concurrent async operations sharing mutable session state.
**How to avoid:** The reconnect flow should: (1) set connection status to "reconnecting", (2) let any in-flight calls complete or fail naturally, (3) reset session state, (4) re-initialize, (5) refresh tools and resources. In-flight bridge calls will get errors from the old session -- this is acceptable; the widget will be reloaded anyway.
**Warning signs:** Error responses after clicking Reconnect.

### Pitfall 6: Empty Resource List Blocks Widget Loading
**What goes wrong:** If the MCP server does not implement `resources/list` or returns no UI resources, the preview shows nothing and the developer does not understand why.
**Why it happens:** Not all MCP servers implement the resources capability.
**How to avoid:** If `resources/list` returns an error or empty list, show a clear message in the widget area: "No UI resources found. Your MCP server needs to expose HTML resources via `resources/list`." Keep the tool panel functional even without resources.
**Warning signs:** Blank widget area with no error message.

## Code Examples

### Example 1: McpProxy with Session Persistence and Resource Methods

```rust
use tokio::sync::RwLock;

struct SessionInfo {
    session_id: Option<String>,
    server_info: Value,
}

pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session: RwLock<Option<SessionInfo>>,
}

impl McpProxy {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
            session: RwLock::new(None),
        }
    }

    async fn ensure_initialized(&self) -> Result<()> {
        // Fast path: session already initialized
        {
            let guard = self.session.read().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        // Slow path: initialize session
        let mut guard = self.session.write().await;
        // Double-check after acquiring write lock
        if guard.is_some() {
            return Ok(());
        }

        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": false },
                "sampling": {}
            },
            "clientInfo": {
                "name": "mcp-preview",
                "version": "0.1.0"
            }
        });

        let result = self.send_request("initialize", Some(params)).await?;

        // Send initialized notification (fire-and-forget)
        let _ = self.send_notification("notifications/initialized").await;

        *guard = Some(SessionInfo {
            session_id: None, // Extract from response header if available
            server_info: result,
        });

        Ok(())
    }

    pub async fn reset_session(&self) {
        let mut guard = self.session.write().await;
        *guard = None;
    }

    async fn send_notification(&self, method: &str) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        let url = format!("{}/mcp", self.base_url);
        let _ = self.client.post(&url).json(&request).send().await;
        Ok(())
    }

    pub async fn list_resources(&self) -> Result<Vec<ResourceInfo>> {
        self.ensure_initialized().await?;
        let result = self.send_request("resources/list", None).await?;
        let resources: Vec<ResourceInfo> = serde_json::from_value(
            result.get("resources").cloned().unwrap_or(Value::Array(vec![]))
        )?;
        Ok(resources)
    }

    pub async fn read_resource(&self, uri: &str) -> Result<ResourceReadResult> {
        self.ensure_initialized().await?;
        let params = json!({ "uri": uri });
        let result = self.send_request("resources/read", Some(params)).await?;
        let contents: Vec<ContentItem> = serde_json::from_value(
            result.get("contents").cloned().unwrap_or(Value::Array(vec![]))
        )?;
        Ok(ResourceReadResult { contents })
    }
}
```

### Example 2: New API Routes for Resources

```rust
// In server.rs -- add routes
let app = Router::new()
    .route("/", get(handlers::page::index))
    .route("/api/config", get(handlers::api::get_config))
    .route("/api/tools", get(handlers::api::list_tools))
    .route("/api/tools/call", post(handlers::api::call_tool))
    .route("/api/resources", get(handlers::api::list_resources))
    .route("/api/resources/read", get(handlers::api::read_resource))
    .route("/api/reconnect", post(handlers::api::reconnect))
    .route("/assets/{*path}", get(handlers::assets::serve))
    .route("/ws", get(handlers::websocket::handler))
    .layer(cors)
    .with_state(state);

// In api.rs -- new handlers
pub async fn list_resources(
    State(state): State<Arc<AppState>>,
) -> Json<ResourcesResponse> {
    match state.proxy.list_resources().await {
        Ok(resources) => Json(ResourcesResponse { resources, error: None }),
        Err(e) => Json(ResourcesResponse { resources: vec![], error: Some(e.to_string()) }),
    }
}

pub async fn read_resource(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ReadResourceParams>,
) -> Json<ResourceReadResponse> {
    match state.proxy.read_resource(&params.uri).await {
        Ok(result) => Json(ResourceReadResponse {
            contents: Some(result.contents),
            error: None,
        }),
        Err(e) => Json(ResourceReadResponse {
            contents: None,
            error: Some(e.to_string()),
        }),
    }
}

pub async fn reconnect(
    State(state): State<Arc<AppState>>,
) -> Json<ReconnectResponse> {
    state.proxy.reset_session().await;
    match state.proxy.list_tools().await {
        Ok(tools) => Json(ReconnectResponse { success: true, tool_count: tools.len(), error: None }),
        Err(e) => Json(ReconnectResponse { success: false, tool_count: 0, error: Some(e.to_string()) }),
    }
}
```

### Example 3: Frontend Resource Picker and Auto-Load

```javascript
// In PreviewRuntime class in index.html
async init() {
    this.setupThemeToggle();
    this.setupDevToolsTabs();
    this.setupEnvironmentControls();
    this.setupExecuteButton();
    await this.loadConfig();
    await this.initSession();  // Combined: load tools + resources, auto-load first widget
}

async initSession() {
    try {
        // Load tools and resources in parallel
        const [toolsResp, resourcesResp] = await Promise.all([
            fetch('/api/tools').then(r => r.json()),
            fetch('/api/resources').then(r => r.json()),
        ]);

        // Handle tools
        if (toolsResp.error) {
            this.setStatus(false, `Error: ${toolsResp.error}`);
        } else {
            this.tools = toolsResp.tools;
            this.renderToolList();
        }

        // Handle resources
        if (resourcesResp.resources && resourcesResp.resources.length > 0) {
            this.uiResources = resourcesResp.resources;
            this.renderResourcePicker();
            // Auto-load first UI resource
            await this.loadResourceWidget(this.uiResources[0].uri);
            this.setStatus(true, `Connected (${this.tools.length} tools, ${this.uiResources.length} resources)`);
        } else {
            this.uiResources = [];
            this.renderResourcePicker(); // Shows "no resources" state
            this.setStatus(true, `Connected (${this.tools.length} tools)`);
        }
    } catch (e) {
        this.setStatus(false, 'Connection failed');
        this.showWidgetError(`Failed to connect to MCP server: ${e.message}`);
    }
}
```

### Example 4: Network Tab Badge Count

```javascript
// Badge count tracking
this.networkUnreadCount = 0;

logBridgeCall(toolName, args, result, duration, success) {
    // Increment unread count
    this.networkUnreadCount++;
    this.updateNetworkBadge();

    // Log the entry (expandable)
    const container = document.getElementById('network-log');
    if (container.querySelector('.empty-state')) {
        container.innerHTML = '';
    }

    const entry = document.createElement('details');
    entry.className = `network-entry ${success ? 'success' : 'error'}`;
    entry.innerHTML = `
        <summary class="network-header">
            <span class="network-method">${toolName}</span>
            <span class="network-time">${duration}ms</span>
        </summary>
        <div class="network-detail">
            <div class="network-label">Arguments</div>
            <pre class="network-body">${JSON.stringify(args, null, 2)}</pre>
            <div class="network-label">Response</div>
            <pre class="network-body">${JSON.stringify(result, null, 2)}</pre>
        </div>
    `;
    container.appendChild(entry);
    container.scrollTop = container.scrollHeight;
}

updateNetworkBadge() {
    const tab = document.querySelector('[data-tab="network"]');
    let badge = tab.querySelector('.badge');
    if (this.networkUnreadCount > 0) {
        if (!badge) {
            badge = document.createElement('span');
            badge.className = 'badge';
            tab.appendChild(badge);
        }
        badge.textContent = this.networkUnreadCount;
    } else if (badge) {
        badge.remove();
    }
}

// Clear badge when Network tab is selected
setupDevToolsTabs() {
    document.querySelectorAll('.devtools-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            // ... existing tab switching code ...
            if (tab.dataset.tab === 'network') {
                this.networkUnreadCount = 0;
                this.updateNetworkBadge();
            }
        });
    });
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-request `initialize()` in proxy | Session-once init with async singleton | Phase 14 (now) | Eliminates N-1 wasted round trips per session |
| Tool-execution-only preview | Resource-based widget preview | Phase 14 (now) | Widgets load from `resources/read`, not just tool responses |
| No resource picker | Resource picker in sidebar | Phase 14 (now) | Multi-widget servers can switch between UI resources |

**Deprecated/outdated:**
- The `McpProxy::list_tools()` pattern of calling `initialize()` before every request is deprecated by session persistence.
- The `widget-placeholder` "Select a tool and click Execute" message is replaced by auto-loaded resource widgets.

## Open Questions

1. **Axum version alignment (0.7 vs 0.8)**
   - What we know: Root pmcp crate uses axum 0.8.5. mcp-preview uses axum 0.7. Both compile in the workspace.
   - What's unclear: Whether having two axum versions causes issues in the workspace. The route syntax in mcp-preview's `server.rs` uses `/{*path}` which is actually 0.8 syntax.
   - Recommendation: Keep 0.7 for now. Phase 14 does not require 0.8 features. If compile issues arise, bump to 0.8. This is a tactical decision, not architectural.

2. **Session ID header forwarding**
   - What we know: MCP Streamable HTTP transport uses `Mcp-Session-Id` response header on initialize, which must be forwarded in subsequent requests.
   - What's unclear: Whether the example servers (chess, map) use session IDs. They use `session_id_generator: None` in their config, suggesting stateless mode.
   - Recommendation: Capture the session ID header from the initialize response if present, and forward it in subsequent requests. If not present, skip. This makes the proxy work with both stateful and stateless servers.

3. **Bridge script stripping from resource HTML**
   - What we know: Resources from servers using `ChatGptAdapter::transform()` already have bridge scripts injected. The preview wraps in its own bridge.
   - What's unclear: Whether dual bridge injection causes issues in practice.
   - Recommendation: Accept dual injection for now. The preview bridge (defined in the outer `<head>`) executes first and wins for `window.mcpBridge`. The inner bridge (from the server adapter) may define `window.mcpBridge` again but the preview bridge's closures over `preview` object keep working. Test with chess example to verify.

4. **Error state for `resources/read` failures**
   - What we know: User wants inline error in widget area with Retry button.
   - What's unclear: What "retry" means -- retry the resource read, or retry the full session?
   - Recommendation: Retry button should retry the `resources/read` call only. If that fails again, show the error. The Reconnect button in the header handles full session reset.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/mcp-preview/src/proxy.rs` -- McpProxy implementation, JSON-RPC protocol, per-request initialization pattern (verified by reading lines 130-160)
- Codebase analysis: `crates/mcp-preview/src/server.rs` -- AppState, route structure, PreviewConfig (verified by reading full file)
- Codebase analysis: `crates/mcp-preview/assets/index.html` -- Full preview UI with PreviewRuntime class, bridge injection via wrapWidgetHtml(), DevTools tabs (verified by reading 1083 lines)
- Codebase analysis: `crates/mcp-preview/src/handlers/api.rs` -- API endpoint handlers for config, tools list, tool call (verified by reading full file)
- Codebase analysis: `examples/mcp-apps-chess/src/main.rs` -- ResourceHandler implementation with `resources/list` returning `ResourceInfo` with URI and MIME type (verified by reading lines 444-483)
- Codebase analysis: `src/types/protocol.rs` -- `Content::Resource` variant structure with uri, text, mime_type fields (verified by reading lines 496-523)
- Codebase analysis: `src/types/mcp_apps.rs` -- `ExtendedUIMimeType` enum with all known UI MIME types (verified by reading lines 624-711)
- Codebase analysis: `src/server/mcp_apps/adapter.rs` -- ChatGptAdapter and McpAppsAdapter bridge injection patterns (verified by reading full file)

### Secondary (MEDIUM confidence)
- `.planning/research/SUMMARY.md` -- Prior research identifying session persistence issue, bridge contract divergence, postMessage origin concerns
- `.planning/STATE.md` -- Blocker documentation: McpProxy re-initialization, postMessage wildcard origin
- `.planning/phases/14-preview-bridge-infrastructure/14-CONTEXT.md` -- User decisions constraining implementation approach

### Tertiary (LOW confidence)
- Training knowledge: `tokio::sync::OnceCell` and `RwLock` patterns for async singleton initialization -- standard Tokio patterns, verified against Tokio docs in training data. The double-check locking pattern is well-established.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies needed; all libraries already in use
- Architecture: HIGH -- Direct codebase analysis of all affected files; changes are extensions of existing patterns
- Pitfalls: HIGH -- Six pitfalls identified from direct code analysis; session re-initialization confirmed in proxy.rs line 151; postMessage origin assessed for srcdoc context
- Code examples: HIGH -- All examples based on existing code patterns in the codebase; Rust patterns from proxy.rs, JavaScript patterns from index.html

**Research date:** 2026-02-24
**Valid until:** 2026-03-24 (stable domain; MCP protocol and Axum APIs not expected to change)
