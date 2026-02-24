# Pitfalls Research

**Domain:** MCP Apps developer tooling — widget preview (Axum + iframe), WASM bridge client, publishing flow (cargo-pmcp deploy + ChatGPT manifest), and authoring DX (file-based HTML, shared bridge library, scaffolding) added to an existing PMCP SDK
**Researched:** 2026-02-24
**Confidence:** HIGH (codebase analysis of proxy.rs, wasm-client/src/lib.rs, mcp_apps.rs, adapter.rs, server.rs, and the chess/map example apps; domain research on ChatGPT Apps SDK, iframe/postMessage security, wasm-bindgen async pitfalls, and CORS with Axum)

## Critical Pitfalls

### Pitfall 1: McpProxy Re-Initializes on Every Tool List — No Session Stickiness

**What goes wrong:**
`McpProxy::list_tools()` calls `self.initialize().await` before every `tools/list` request. The MCP protocol requires initialization to happen exactly once per session. On stateful MCP servers that track sessions, re-sending `initialize` mid-session can cause servers to reset session state, re-negotiate capabilities, or return an error because a second `initialize` is not valid after the session is established. More concretely, on the existing PMCP `StreamableHttpServer`, the session ID issued in the `initialize` response is stored in an `Mcp-Session-Id` header — but `McpProxy` never stores or forwards this header, so every subsequent request is treated as a sessionless request. The preview server appears to work only because `enable_json_response: true` is set in the chess example, which degrades to stateless mode.

**Why it happens:**
The proxy was written for stateless HTTP testing where every round-trip is self-contained. The comment `// First, ensure we're initialized` looks defensive but is actually incorrect for stateful servers. There is no session tracking state in `McpProxy`, and the `AtomicU64` request ID counter starts at 1 on each `McpProxy::new()`, meaning concurrent requests within a session will have colliding or mismatched IDs if the proxy is ever reused across requests.

**How to avoid:**
Add explicit session lifecycle management to `McpProxy`:

```rust
pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session_id: tokio::sync::OnceCell<Option<String>>,  // set once after initialize
}

impl McpProxy {
    pub async fn ensure_initialized(&self) -> Result<()> {
        self.session_id.get_or_try_init(|| async {
            let result = self.send_request("initialize", Some(params)).await?;
            // Extract Mcp-Session-Id from response headers
            // Store it for subsequent requests
            Ok(session_id_from_result)
        }).await?;
        Ok(())
    }
}
```

Forward the stored session ID via `Mcp-Session-Id` header on every subsequent request. For the preview server's use case (stateless development servers), document that the preview currently requires `enable_json_response: true` (stateless mode) on the target server.

**Warning signs:**
- Preview server works with chess example (stateless) but fails with servers that use `Mcp-Session-Id`
- Each tool list call logs two JSON-RPC requests instead of one
- Server logs show multiple `initialize` method calls from the same client within seconds
- `list_tools()` always returns the full tool list even if the server's state has changed (because it re-initializes each time)

**Phase to address:**
Phase 1 (mcp-preview completion). Fix `McpProxy` session management before adding WASM bridge support that relies on the same proxy pattern.

---

### Pitfall 2: postMessage Bridge Uses Wildcard Target Origin — Security and ChatGPT Incompatibility

**What goes wrong:**
The `McpAppsAdapter::inject_bridge()` sends all postMessage calls with wildcard target origin `'*'`:

```javascript
window.parent.postMessage({
    jsonrpc: '2.0',
    id,
    method,
    params
}, '*');   // <-- wildcard: sends to ANY window
```

This has two consequences. First, it is a security vulnerability: if the widget is embedded inside a malicious page that sits between ChatGPT and the widget (e.g., via a compromised CDN or ad injection), that malicious page receives all tool call payloads including any sensitive arguments. The Microsoft CVE-2024-49038 (CVSS 9.3) exploited exactly this pattern. Second, ChatGPT's Skybridge runtime validates the postMessage target origin — widgets using `'*'` may have their messages silently dropped or flagged in stricter ChatGPT App review contexts.

The `message` event listener in the same bridge also lacks origin validation:

```javascript
window.addEventListener('message', (event) => {
    const msg = event.data;
    if (msg.jsonrpc !== '2.0') return;
    // No event.origin check — accepts messages from any window
```

Any page on the internet that can embed the widget iframe can inject arbitrary JSON-RPC responses, causing the widget to execute fake tool results.

**Why it happens:**
Using `'*'` is the path of least resistance during development — it avoids having to know the parent's origin at widget load time. The widget cannot know its parent's origin without the parent passing it in. This creates a chicken-and-egg problem that most developers resolve by using `'*'` and never revisiting it.

**How to avoid:**
Pass the trusted parent origin to the widget during initialization. The standard pattern is for the parent (preview server or ChatGPT) to send the first message containing the allowed origin:

```javascript
// Parent sends origin declaration first
iframe.contentWindow.postMessage({
    type: 'init',
    allowedOrigin: window.location.origin
}, iframeOrigin);

// Widget stores and validates
let trustedOrigin = null;
window.addEventListener('message', (event) => {
    if (!trustedOrigin && event.data?.type === 'init') {
        trustedOrigin = event.data.allowedOrigin;
        return;
    }
    if (event.origin !== trustedOrigin) return; // reject unknown origins
    // ... process message
});

// Widget sends to known origin only
window.parent.postMessage(msg, trustedOrigin);
```

For the preview server, the preview page knows it is the parent and its origin is `http://localhost:{port}` — pass this to the iframe via `srcdoc` or a query parameter. For ChatGPT, the Skybridge runtime manages origin policy; use `window.openai.callTool()` instead of postMessage directly.

**Warning signs:**
- Bridge scripts contain `postMessage({...}, '*')` in production code (not just dev stubs)
- No `event.origin` check in the `window.addEventListener('message', ...)` handler
- Widget tool calls include user credentials or session tokens passed as arguments
- The WASM bridge in the preview server relays arbitrary postMessage content to the MCP server without validating source origin

**Phase to address:**
Phase 1 (mcp-preview completion) for the preview bridge. Phase 2 (WASM bridge) for the WASM-side postMessage handler. Must be addressed before shipping examples that demonstrate `widget_accessible: true` tools.

---

### Pitfall 3: WASM Client Has Hardcoded Request IDs — Concurrent Calls Corrupt Responses

**What goes wrong:**
The HTTP path of `WasmClient::call_tool()` uses a hardcoded request ID of `3i64`:

```rust
let request = pmcp::shared::TransportMessage::Request {
    id: 3i64.into(), // TODO: Implement proper request ID tracking
    ...
};
```

`list_tools()` uses `2i64` and `connect()` uses `1i64`. If a widget calls `callTool` twice concurrently (e.g., getting valid moves while another move is resolving), both requests have ID `3`. The MCP server and the `WasmHttpClient` response demultiplexer match responses to requests by ID. Two concurrent requests with the same ID will cause one response to be matched to the wrong request or lost entirely, producing incorrect game state or silent failures.

The `WasmClient` struct uses `&mut self` on `list_tools` and `call_tool`, which prevents concurrent JavaScript calls from the same `WasmClient` instance — but this is enforced at compile time only. From JavaScript, the user calls async functions sequentially but the JavaScript event loop can interleave them, triggering the wasm-bindgen "recursive use of an object detected which would lead to unsafe aliasing in rust" runtime panic.

**Why it happens:**
The HTTP client path was added to mirror the WebSocket path but without the request ID tracking that the existing WebSocket implementation handles through the protocol. The `// TODO` comment acknowledges the gap but it is easy to miss during widget development where single-tool calls rarely expose concurrency issues.

**How to avoid:**
Add an atomic counter to the request ID generator, mirroring what `McpProxy` already does correctly with `AtomicU64`:

```rust
pub struct WasmClient {
    connection_type: Option<ConnectionType>,
    ws_client: Option<Client<WasmWebSocketTransport>>,
    http_client: Option<WasmHttpClient>,
    next_id: std::sync::atomic::AtomicI64,  // add this
}

impl WasmClient {
    fn next_request_id(&self) -> i64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
```

For the `&mut self` / concurrent-call problem: redesign the bridge API to use `RefCell<WasmClient>` internally or restructure so that the WASM-exposed async methods take ownership of what they need before the first await point. Document in the bridge API that concurrent JS calls to the same `WasmClient` are not supported without explicit queuing.

**Warning signs:**
- Widget makes two rapid tool calls (e.g., `chess_valid_moves` immediately after `chess_new_game`) and receives wrong results
- Browser console shows `"recursive use of an object detected which would lead to unsafe aliasing in rust"` panic
- The `// TODO: Implement proper request ID tracking` comment is still present
- HTTP-path tool calls are never tested concurrently in integration tests

**Phase to address:**
Phase 2 (WASM bridge). Fix before exposing the WASM client to widget authors as a supported API.

---

### Pitfall 4: Bridge Injection Breaks on HTML with Multiple `</head>` Occurrences or Template Strings

**What goes wrong:**
The `inject_bridge` implementations in both `ChatGptAdapter` and `McpAppsAdapter` use `html.replace("</head>", ...)`:

```rust
if html.contains("</head>") {
    html.replace("</head>", &format!("{bridge_script}</head>"))
```

Rust's `str::replace` replaces **all occurrences**, not just the first. An HTML template that contains `</head>` inside a JavaScript string literal, HTML comment, or `<template>` tag will have the bridge script injected multiple times. Example:

```html
<head><title>Chess</title></head>
<body>
  <template id="error-tmpl">
    <!-- loaded from </head> snippet -->
  </template>
</body>
```

Additionally, `html.replace` is O(n) per call and creates a new allocation. For large widget HTML files (say, 500KB with embedded SVGs), this is a measurable allocation hit on every resource read request.

For the HTML injection path without `</head>`, the code falls back to wrapping the entire document in `<head>...</head>` tags, which produces invalid HTML if there is already a `<head>` element with a different case (`<HEAD>`) or in XML-ish content.

**Why it happens:**
Simple string substitution is fast to implement and handles the common case (a normal HTML file with one `</head>`). Widget authors write clean HTML so the edge cases rarely surface during development, but they can appear when including third-party HTML fragments, SVG embeds with XML processing instructions, or when scaffolded templates include comment blocks.

**How to avoid:**
Use `html.replacen("</head>", ..., 1)` (replace only the first occurrence) as the immediate fix. For a robust solution, use a lightweight HTML injection approach that searches for the last `<head>` tag position and inserts before `</head>`:

```rust
fn inject_before_head_close(html: &str, script: &str) -> String {
    // Case-insensitive search for the closing head tag
    let lower = html.to_lowercase();
    if let Some(pos) = lower.find("</head>") {
        let mut result = String::with_capacity(html.len() + script.len());
        result.push_str(&html[..pos]);
        result.push_str(script);
        result.push_str(&html[pos..]);
        result
    } else {
        // No <head>: prepend script
        format!("{script}{html}")
    }
}
```

**Warning signs:**
- Widget HTML contains commented-out code blocks with HTML tags
- Bridge script appears twice in the rendered widget's source
- Widget with embedded `<template>` tags or SVG `<defs>` fails to initialize
- Tests only use minimal `<html><head></head><body></body></html>` fixtures, never real widget HTML

**Phase to address:**
Phase 1 (mcp-preview). Fix `inject_bridge` before the file-based widget authoring feature (Phase 3 authoring DX) makes this code path critical for real widget files.

---

### Pitfall 5: ChatGPT Manifest Generation Without HTTPS Causes Silent Rejection

**What goes wrong:**
The ChatGPT Apps runtime requires an HTTPS endpoint for MCP server registration. ChatGPT will not connect to an HTTP server even for development mode. The OAuth protected resource endpoint must be at `https://your-mcp.example.com/.well-known/oauth-protected-resource`. If `cargo pmcp deploy` generates a manifest with an HTTP URL (e.g., because the deploy target outputs an HTTP load balancer URL before HTTPS redirect is configured), ChatGPT will silently fail to register the MCP App with no useful error message — it simply will not appear in the ChatGPT Apps panel.

The existing deploy flow uses `npx cdk deploy` which outputs a CloudFormation stack URL. Lambda Function URLs can be HTTP or HTTPS depending on configuration. If the generated manifest uses the HTTP variant of the Function URL, the ChatGPT registration fails.

**Why it happens:**
Developers test the manifest locally with an HTTP preview server (mcp-preview runs on `http://localhost:8765`), which works for the preview tool itself. When they deploy and generate the manifest for ChatGPT submission, they copy the pattern from the preview configuration and use the deployed HTTP URL. ChatGPT's error reporting for manifest problems is minimal — it shows a generic "could not connect" message.

**How to avoid:**
In the manifest generation step of `cargo pmcp deploy`:
1. Validate that the MCP server URL starts with `https://` before generating the manifest. Fail with a clear error: `"ChatGPT requires HTTPS. The deployed URL must use https:// — configure HTTPS on your deployment target first."`
2. Auto-generate the `.well-known/oauth-protected-resource` endpoint as part of the manifest.
3. For AWS Lambda: ensure the CloudFormation template configures `AuthType: NONE` (or OAuth) on the Function URL with HTTPS only. Document this in the deploy guide.
4. Add a `cargo pmcp validate-manifest` subcommand that checks the manifest URL is HTTPS, the endpoint is reachable, and the tools/list response is valid.

```rust
// In manifest generation
if !mcp_url.starts_with("https://") {
    anyhow::bail!(
        "ChatGPT Apps require HTTPS. Got: {}. \
         Configure HTTPS on your deployment target before generating the manifest.",
        mcp_url
    );
}
```

**Warning signs:**
- Manifest generation accepts `http://` URLs without warning
- No `cargo pmcp validate-manifest` or equivalent verification step
- CloudFormation template does not enforce HTTPS-only on Function URL
- The deploy guide says "deploy, then register" without explicitly noting the HTTPS requirement

**Phase to address:**
Phase 3 (publishing flow). Must be part of the initial manifest generation implementation, not a post-ship fix.

---

### Pitfall 6: iframe Preview Bridge and WASM Bridge Have Divergent APIs — Widget Authors Must Target Two Contracts

**What goes wrong:**
The mcp-preview tool injects a `window.mcpBridge` via the preview server's parent page JavaScript (chess `preview.html`), which is a completely different code path from the `inject_bridge` call in `ChatGptAdapter`. When a widget is developed using `mcp-preview`, it runs against the preview bridge (which is a mock implemented in `preview.html`'s JavaScript). When the widget is deployed to ChatGPT, it runs against the `ChatGptAdapter`-injected bridge which wraps `window.openai`. If these two bridges have different function signatures or different return value shapes, the widget works in preview but fails in production.

Concrete divergence already exists: the preview bridge in chess `preview.html` returns raw game state objects from `callTool()`, while the ChatGPT `window.openai.callTool()` wraps tool results in `{ content: [...], _meta: ... }` envelopes (matching the MCP `CallToolResult` type). A widget that does `const state = await mcpBridge.callTool('chess_new_game', {})` and treats `state` as the game state directly will work in preview (mock returns state directly) but fail in ChatGPT (result is `{ content: [{ type: 'text', text: '...' }] }`).

**Why it happens:**
The preview bridge and the production bridge are written independently and there is no shared contract definition. The preview bridge was written to make the chess example easy to develop, not to accurately simulate the ChatGPT runtime. Since the chess example works in preview, nobody notices the divergence until running in actual ChatGPT.

**How to avoid:**
Define a single `McpBridgeContract` interface (as a TypeScript type definition or a well-documented JavaScript module) that both the preview bridge and the production bridge must conform to:

```typescript
// bridge-contract.d.ts
interface McpBridgeCallResult {
    content: Array<{ type: string; text?: string; mimeType?: string }>;
    isError?: boolean;
    _meta?: Record<string, unknown>;
}

interface McpBridge {
    callTool(name: string, args: Record<string, unknown>): Promise<McpBridgeCallResult>;
    getState(): Record<string, unknown>;
    setState(state: Record<string, unknown>): void;
    // ... all methods
}
```

The preview server's mock bridge must return results in `McpBridgeCallResult` format, not raw values. The shared bridge library (authoring DX feature) enforces this contract by providing a `parseBridgeResult<T>(result: McpBridgeCallResult): T` helper that widget authors use instead of direct property access.

The Playwright E2E tests should run against both the mock bridge (via `mcp-preview`) and a real ChatGPT-like bridge to catch divergence.

**Warning signs:**
- `preview.html` bridge returns a different structure than the ChatGPT SDK bridge would return for the same tool call
- Widget code does `const result = await mcpBridge.callTool(...)` and then `result.someField` directly without checking `result.content[0].text`
- No TypeScript types or interface definition for the bridge contract
- E2E tests only run against `mcp-preview`, never against a production-equivalent bridge

**Phase to address:**
Phase 1 (mcp-preview completion) to define the contract. Phase 2 (WASM bridge) to implement it. Phase 3 (authoring DX) to provide the shared bridge helper library that enforces it.

---

### Pitfall 7: WASM Module Size Balloons When Including Full pmcp Crate Without Feature Pruning

**What goes wrong:**
The WASM client uses `pmcp = { path = "../..", default-features = false, features = ["wasm"] }`. The `pmcp` crate is large (~32K Rust LOC at v1.2 with tasks, workflow, auth, storage backends, and MCP Apps adapters). Even with `default-features = false`, feature flags that are implicitly enabled by transitive dependencies (e.g., `serde`, `tokio` features pulled in by async-trait) can bloat the WASM binary significantly. The `console_error_panic_hook` and `tracing-wasm` dependencies are also included unconditionally, adding debug overhead even in release builds.

The existing `wasm-pack` configuration in `Cargo.toml` sets `wasm-opt = false`, which explicitly disables the wasm-opt optimizer that typically reduces WASM binary size by 15-20%. For a browser test client that needs to download quickly, a 5MB unoptimized WASM binary is a bad developer experience (each reload of the preview page downloads 5MB).

**Why it happens:**
`wasm-opt = false` was set to work around build system issues (wasm-opt requires a separate binary installation on some CI systems). It is a build convenience that becomes a permanent liability. Feature flag analysis for WASM is tedious — `cargo bloat --target wasm32-unknown-unknown` is not in the standard toolkit.

**How to avoid:**
1. Enable `wasm-opt` in release builds. Add it as a CI dependency or use the `wasm-opt` crate feature in `wasm-pack` which bundles a pre-compiled wasm-opt binary:
   ```toml
   [package.metadata.wasm-pack]
   wasm-opt = ["-Oz", "--enable-mutable-globals"]
   ```
2. Add a Cargo profile for WASM builds that enables size optimization:
   ```toml
   [profile.release]
   opt-level = "z"
   lto = true
   codegen-units = 1
   ```
3. Gate `tracing-wasm` and `console_error_panic_hook` behind a `debug` feature flag so release WASM builds exclude them.
4. Add a `just wasm-size` recipe that runs `wasm-pack build --release` and reports binary size. Set a budget (e.g., 2MB uncompressed) and fail CI if exceeded.

**Warning signs:**
- `wasm-opt = false` remains in `Cargo.toml` with no comment explaining why
- WASM binary in `pkg/` is larger than 3MB
- `cargo bloat` has never been run against the WASM target
- `tracing-wasm` is initialized unconditionally in `WasmClient::new()` even in release builds

**Phase to address:**
Phase 2 (WASM bridge). Address before the WASM bridge adds more dependencies for bridge injection and postMessage handling.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Preview bridge mocks tool calls in JavaScript | Fast widget iteration without server | Bridge contract drifts from production; widget works in preview, fails in ChatGPT | Only acceptable until shared bridge library defines the contract (Phase 3) |
| `window.parent.postMessage({...}, '*')` wildcard target origin | No need to know parent origin at bridge init time | Security vulnerability; CVE-class bug if widget handles sensitive data; may be silently dropped by ChatGPT runtime | Never acceptable in production code; use origin handshake pattern |
| Hardcoded request IDs in WASM HTTP client | Simplest possible implementation | Concurrent calls corrupt responses; runtime panics on rapid sequential calls | Acceptable only during initial prototype before any concurrent usage |
| `cargo pmcp deploy` skips HTTPS validation | Faster dev iteration with HTTP | Manifest is silently rejected by ChatGPT; developer has no actionable error | Never acceptable in the deploy command output |
| Inline HTML strings in `include_str!()` | Works today; files already exist | Blocks file-based widget authoring DX; hot-reload requires server restart | Only until file-based authoring is implemented (Phase 3) |
| `html.replace("</head>", ...)` (replaces all occurrences) | Simple one-liner | Injects bridge multiple times into HTML with template tags or comments | Acceptable only for trivial test HTML; use `replacen(..., 1)` immediately |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| ChatGPT Skybridge | Testing widget with `mcp-preview` mock and shipping to ChatGPT assuming bridge APIs match | Define bridge contract TypeScript types; test against real `window.openai` API signatures in unit tests before shipping |
| ChatGPT Skybridge | Using `window.mcpBridge.callTool()` results directly as typed objects | Tool results are wrapped in `{ content: [...] }` — use `parseBridgeResult<T>()` helper or check `result.content[0].text` |
| ChatGPT Skybridge | Sending postMessage to `window.parent` with `'*'` | ChatGPT injects `window.openai.callTool()` — use the OpenAI API directly; no postMessage needed for Skybridge widgets |
| mcp-preview WebSocket | Widget calls bridge, expects synchronous response | WebSocket bridge is async; always `await` bridge calls; never use bridge in synchronous render code |
| CORS in mcp-preview | Preview server uses `CorsLayer::new().allow_origin(Any)` — this is the current code | For preview, `Any` origin is correct. For production MCP server, restrict to your domain. Never use `allow_credentials(true)` with `Any` origin |
| Axum static assets | Embedded assets via `rust-embed` require recompile to update | For development, serve assets from the filesystem using `tower-http ServeDir`; use `rust-embed` only for release builds |
| ChatGPT manifest | Manifest uses HTTP URL from deploy output | Validate URL is HTTPS before generating manifest; fail fast with clear error |
| wasm-bindgen `JsValue` | Passing Rust structs across the WASM boundary by value consumes them | Prefer `JsValue` returns from `serde_wasm_bindgen::to_value()`; mark consumed values clearly in API docs |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| McpProxy re-initializes on every tool list | Each tool list request takes 2 RTTs instead of 1; servers with session state reset on each call | Initialize once and cache session ID in `OnceCell` | From the first deployment where the target server uses `Mcp-Session-Id` |
| WASM binary downloaded on every preview page reload | Preview page feels slow even on localhost; large binary clogs dev server | Enable wasm-opt; add content-hash to WASM file URL for caching | Immediately during widget development iteration |
| Chess game state included in every tool call argument | Network payload per tool call grows with move history (8x8 board × 64 squares × move history) | This is intentional for stateless design — document that widgets should trim state; provide a `trimGameState()` helper | At ~200 moves (game history becomes larger than the board state) |
| Playwright E2E tests spawn full browser for each test | CI widget test suite takes 5+ minutes | Share browser context across tests; only reload the page for each test | When widget test count exceeds 10 tests |
| WASM module instantiation in every preview iframe reload | Repeated 500ms+ instantiation delay on each widget test cycle | Cache initialized WASM module in a web worker or use `WebAssembly.compileStreaming` with HTTP cache | After first day of widget development when the reload friction becomes noticeable |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| postMessage with wildcard `'*'` target origin in production bridge | Any page embedding the widget can intercept tool call arguments including user data; CVE-class vulnerability (similar to CVE-2024-49038, CVSS 9.3) | Use origin handshake: parent passes allowed origin on first message; widget validates `event.origin` on every message |
| No `event.origin` check in message listener | Malicious page can inject fake tool call responses, causing widget to execute attacker-controlled logic | Always check `event.origin === trustedOrigin` before processing postMessage events |
| Exposing `widget_accessible: true` tools without session scoping | A widget in one ChatGPT conversation can call tools that affect state in another conversation | Scope tool-accessible state by `widgetSessionId` from `WidgetResponseMeta`; validate it on every tool call |
| Rendering user-provided HTML in widget iframe without Content Security Policy | Stored XSS if widget content includes user-generated HTML | Set `Content-Security-Policy` header on the preview server; never inject user content directly into HTML without sanitization |
| Trusting `toolInput` from `window.openai.toolInput` without server-side validation | Widget state can be tampered (the client controls what is displayed but the server must validate) | Always re-validate game state or tool inputs server-side on `tools/call`; never trust widget-provided state as authoritative |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Widget shows blank screen when bridge is not ready at load time | Developer stares at blank preview with no indication of what went wrong | Check for bridge readiness with a timeout and show a clear error: "Bridge not available — is the preview server running?" |
| Tool call errors are swallowed in the bridge `catch` block | Widget appears to hang; developer cannot diagnose failures | Surface errors visibly in the widget and log to both browser console and the mcp-preview dev panel simultaneously |
| `cargo pmcp new --mcp-apps` scaffold generates widget that uses bridge before DOM is ready | Widget fails with "cannot read property of null" errors that are hard to trace | Scaffold wraps all bridge usage in `DOMContentLoaded` or `mcpBridgeReady` event listener; document this in scaffold comments |
| Preview server requires manual restart to pick up HTML file changes | Developer edits widget HTML, sees no change, edits again, still no change — until restart | mcp-preview should watch `widget/` directory and broadcast reload notification via WebSocket; already has a `/ws` handler |
| `cargo pmcp preview` opens browser before server is ready | Browser shows connection refused; developer refreshes manually | The current 800ms sleep before `open::that()` is fragile — replace with health-check polling on `/api/config` endpoint |

## "Looks Done But Isn't" Checklist

- [ ] **McpProxy session management:** Often the proxy re-initializes on each tool list call — verify that `initialize` is called exactly once per `McpProxy` instance and that `Mcp-Session-Id` is forwarded on subsequent requests
- [ ] **postMessage target origin:** Often ships with `'*'` wildcard — verify that production bridge code specifies an explicit target origin, not `'*'`
- [ ] **postMessage message listener:** Often lacks `event.origin` validation — verify the `message` event handler checks `event.origin` matches the trusted parent origin
- [ ] **WASM request IDs:** Often hardcoded — verify that `call_tool`, `list_tools`, and `connect` each use an atomically incrementing request ID, not `1i64`, `2i64`, `3i64`
- [ ] **Bridge contract parity:** Often diverges between preview mock and production bridge — verify that `callTool()` returns `{ content: [...] }` envelope in both preview and production implementations
- [ ] **HTTPS validation in manifest generation:** Often accepts any URL — verify that `cargo pmcp deploy` with MCP Apps rejects HTTP URLs and emits a clear error before generating the manifest
- [ ] **`html.replace` injects bridge once:** Often replaces all occurrences — verify `replacen("</head>", ..., 1)` is used, and test with HTML that contains `</head>` inside a `<script>` string
- [ ] **wasm-opt enabled in release:** Often still `wasm-opt = false` — verify that `pkg/*.wasm` for release builds is less than 2MB uncompressed
- [ ] **CORS credentials + Any origin:** Often misconfigured together — verify that `allow_credentials(true)` is never combined with `allow_origin(Any)` in Axum routes
- [ ] **Bridge readiness event:** Often missing — verify widgets listen for `mcpBridgeReady` event before calling any bridge methods, and test what happens when the event never fires

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| McpProxy re-initializes (session drift) | LOW | Add `OnceCell<String>` for session ID; forward `Mcp-Session-Id` header; one afternoon of work |
| postMessage wildcard origin ships to production | HIGH | Requires coordinated update of preview bridge contract, adapter bridge scripts, and widget documentation; existing deployed widgets must be re-published |
| WASM hardcoded request IDs cause concurrent corruption | MEDIUM | Add `AtomicI64` counter to `WasmClient`; fix is mechanical but requires thorough regression testing of all tool call paths |
| Bridge contract divergence discovered post-ship | HIGH | Must deprecate old bridge API, publish new contract as TypeScript types, update all widget examples, write migration guide |
| HTTPS validation missing from manifest generation | LOW | Add URL validation to `cargo pmcp deploy` manifest step; one-line fix, immediate re-release of cargo-pmcp |
| WASM binary too large for production use | MEDIUM | Enable wasm-opt, add size optimization Cargo profile, re-run wasm-pack build; may require feature flag audit to remove unused dependencies |
| `html.replace` double-injects bridge | LOW | Change to `html.replacen("</head>", ..., 1)`; fix is one line; verify with test cases covering template tags and comments |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| McpProxy session re-initialization | Phase 1: mcp-preview completion | Integration test: connect to stateful PMCP server, call `list_tools` twice, verify exactly one `initialize` was sent |
| postMessage wildcard target origin | Phase 1: mcp-preview completion (preview bridge), Phase 2: WASM bridge | Security audit: grep for `postMessage({`, verify no `'*'` target origin in production bridge scripts |
| WASM hardcoded request IDs | Phase 2: WASM bridge | Concurrent test: call `list_tools` and `call_tool` in parallel from JavaScript, verify both responses are correctly matched |
| Bridge inject double-fires on template HTML | Phase 1: mcp-preview completion | Unit test: inject bridge into HTML with `</head>` inside `<script>` string; verify bridge script appears exactly once |
| Bridge contract divergence | Phase 1 (define contract), Phase 2 (enforce in WASM bridge), Phase 3 (shared library) | E2E test: run chess widget against both mock bridge and production-equivalent bridge, assert same state transitions |
| ChatGPT manifest HTTPS validation | Phase 3: publishing flow | Integration test: call manifest generation with HTTP URL, verify non-zero exit code and descriptive error message |
| WASM binary size bloat | Phase 2: WASM bridge | CI check: `wasm-pack build --release` produces pkg/*.wasm under 2MB; fails build if exceeded |
| Browser before server ready (open timing) | Phase 1: mcp-preview completion | Manual test: `cargo pmcp preview` with fast server; verify browser opens to a loaded page, not connection refused |

## Sources

- [OpenAI Apps SDK Reference](https://developers.openai.com/apps-sdk/reference/)
- [MCP Apps compatibility in ChatGPT](https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt/)
- [OpenAI Apps SDK Security & Privacy](https://developers.openai.com/apps-sdk/guides/security-privacy)
- [How to crash your software with Rust and wasm-bindgen — Ross Gardiner (2025-01-20)](https://www.rossng.eu/posts/2025-01-20-wasm-bindgen-pitfalls/)
- [Pitfalls of wasm-bindgen part 2: vec parameters — Ross Gardiner (2025-02-22)](https://www.rossng.eu/posts/2025-02-22-wasm-bindgen-vec-parameters/)
- [wasm-bindgen Issue #2486: Recursive use of an object detected](https://github.com/wasm-bindgen/wasm-bindgen/issues/2486)
- [PostMessage security — Microsoft MSRC Blog (2025-08)](https://www.microsoft.com/en-us/msrc/blog/2025/08/postmessaged-and-compromised)
- [PostMessage target origin wildcard vulnerabilities — Payatu](https://payatu.com/blog/postmessage-vulnerabilities/)
- [Sandboxed iframes: allow-scripts + allow-same-origin security issue — Mozilla Discourse](https://discourse.mozilla.org/t/can-someone-explain-the-issue-behind-the-rule-sandboxed-iframes-with-attributes-allow-scripts-and-allow-same-origin-are-not-allowed-for-security-reasons/110651)
- [iframe Security Risks — Qrvey (2026)](https://qrvey.com/blog/iframe-security/)
- [Shrinking .wasm Code Size — Rust and WebAssembly Book](https://rustwasm.github.io/book/reference/code-size.html)
- [wasm-pack build command reference](https://rustwasm.github.io/docs/wasm-pack/commands/build.html)
- [Cross-window communication — javascript.info](https://javascript.info/cross-window-communication)
- [OpenAI ChatGPT Apps Developer Mode announcement (September 2025) — InfoQ](https://www.infoq.com/news/2025/10/chat-gpt-mcp/)
- Codebase analysis: `crates/mcp-preview/src/proxy.rs`, `examples/wasm-client/src/lib.rs`, `src/types/mcp_apps.rs`, `src/server/mcp_apps/adapter.rs`, `crates/mcp-preview/src/server.rs`, `examples/mcp-apps-chess/preview.html`

---
*Pitfalls research for: MCP Apps developer tooling (widget preview, WASM bridge, publishing, authoring DX) added to PMCP SDK*
*Researched: 2026-02-24*
