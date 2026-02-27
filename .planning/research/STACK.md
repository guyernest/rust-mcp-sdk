# Stack Research: MCP Apps Developer Experience (v1.3)

**Domain:** MCP Apps developer tooling — preview server, WASM test client, publishing, scaffolding
**Researched:** 2026-02-24
**Confidence:** HIGH (mcp-preview, WASM, Playwright), MEDIUM (MCP Apps spec, ChatGPT manifest format)
**Mode:** Subsequent milestone — only NEW stack for v1.3, not re-researching v1.0/v1.1/v1.2 foundations

## Executive Assessment

**Five distinct areas need stack attention, most of which require zero new Rust crates.** The existing `mcp-preview` crate already has Axum 0.7, WebSocket, reqwest, rust-embed, and serde. The `wasm-client` already has wasm-bindgen 0.2 and web-sys. The `cargo-pmcp` CLI already has minijinja-style templating needs covered by its existing deps.

The real work is:
1. **mcp-preview widget bridge**: The iframe MCP bridge JavaScript is ~90% complete in `assets/index.html`. What's missing is the `ui://` resource URI resolution pattern per the MCP Apps spec v2026-01-26 — tools now declare `_meta.ui.resourceUri` pointing to `ui://` resources, not inline HTML. The current proxy assumes inline HTML in tool responses; it must also support fetching resources by `ui://` URI.
2. **WASM widget bridge**: The WASM client (wasm-bindgen 0.2.100+) needs `web-sys` features for `MessageChannel`, `HtmlIframeElement`, and `Window.postMessage` to inject the bridge into an iframe. These are features on the existing `web-sys` dep, not new crates.
3. **ChatGPT manifest generation**: No `manifest.json` file — OpenAI uses MCP server discovery via `_meta.ui.resourceUri` on tool annotations. The "manifest" is the MCP `tools/list` response with `ui` metadata annotations. Generation is pure `serde_json` templating, zero new deps.
4. **Demo landing pages**: HTML generation for stakeholder demos. Use `minijinja` `"2"` (already the right choice for cargo-pmcp's scaffolding use case — minimal deps, Jinja2 syntax, no proc macros). This IS a new dep for `mcp-preview` but `cargo-pmcp` needs it for `new --mcp-apps`.
5. **`cargo pmcp new --mcp-apps` scaffolding**: Template expansion with variable substitution. Use `minijinja` `"2"`. cargo-pmcp already has the CLI infrastructure; this is template files + minijinja dep.

**Axum version mismatch is the one technical risk**: `mcp-preview` uses `axum = "0.7"` but the root `pmcp` crate uses `axum = "0.8.5"`. The route syntax changed (`/:param` → `/{param}`, `/*path` → `/{*path}`). `mcp-preview` must upgrade to `axum = "0.8"` to avoid dependency duplication and the path syntax already in `server.rs` uses the new `{*path}` style — so the code is written for 0.8 already but Cargo.toml says 0.7. Fix: bump `mcp-preview/Cargo.toml` to `axum = "0.8"`.

---

## Recommended Stack Additions

### mcp-preview Crate — Changes Required

| Technology | Current | Target | Change | Why |
|------------|---------|--------|--------|-----|
| `axum` | `"0.7"` | `"0.8"` | Version bump | Aligns with root `pmcp` crate. Route syntax `/{*path}` already used in server.rs matches 0.8 syntax. Avoids duplicate compilation. |
| `tower-http` | `"0.6"` | `"0.6"` | No change | 0.6 is the version compatible with axum 0.8. Already correct. |
| `minijinja` | (absent) | `"2"` | New dep | Needed for demo landing page HTML generation (configurable templates for stakeholder demos with server URL, tool list, branding). |

**mcp-preview adds one dep: `minijinja = "2"`.**

### cargo-pmcp Crate — Changes Required

| Technology | Current | Target | Change | Why |
|------------|---------|--------|--------|-----|
| `minijinja` | (absent) | `"2"` | New dep | `cargo pmcp new --mcp-apps` scaffolding: expand template files with `{{project_name}}`, `{{server_url}}`, `{{tool_name}}` variables. minijinja has no proc macros, minimal compile footprint vs `tera`. |

**cargo-pmcp adds one dep: `minijinja = "2"`.**

### wasm-client (examples/wasm-client) — Changes Required

| Technology | Current Version | Change | Why |
|------------|----------------|--------|-----|
| `wasm-bindgen` | `"0.2"` | Pin to `"0.2.100"` or keep `"0.2"` | Existing dep. Latest is 0.2.108 (Jan 2026). The `"0.2"` semver range already pulls the latest compatible version. No change needed. |
| `web-sys` features | `["console", "WebSocket"]` | Add: `"MessageChannel"`, `"MessagePort"`, `"HtmlIframeElement"`, `"Window"`, `"EventTarget"` | Bridge injection into iframe requires postMessage API. These are all `web-sys` feature flags, NOT new crates. |

**wasm-client changes: web-sys feature additions only. No new crates.**

### Build Tooling — New Requirements

| Tool | Version | Purpose | Where Used |
|------|---------|---------|------------|
| `wasm-pack` | latest (0.13.x) | Build WASM client to `pkg/` output for embedding | CI + developer local build. Already used implicitly but should be pinned in `just` tasks. |
| `@playwright/test` | `^1.50.0` | E2E widget testing | Already in `tests/playwright/package.json`. Correct version. |

**No new Rust toolchain crates. `wasm-pack` is already the standard WASM build tool for this stack.**

---

## Detailed Recommendations by Feature Area

### 1. mcp-preview Widget Iframe Rendering

**Gap**: The current `handleToolResponse()` in `index.html` looks for inline HTML in `content[].text` with MIME type `text/html`. The MCP Apps spec (2026-01-26) defines tools as declaring `_meta.ui.resourceUri: "ui://..."` — the host fetches the resource separately, then injects the tool result as context into the already-rendered widget.

**Stack implication**: The proxy (`proxy.rs`) needs a `read_resource(uri: &str)` method alongside `call_tool()`. The existing `reqwest::Client` handles this — zero new deps. The resource URI `ui://my-widget` maps to a `resources/read` JSON-RPC call.

**Bridge JavaScript in `index.html`**: The `wrapWidgetHtml()` function is functional but uses `window.parent` direct access which breaks with `sandbox` iframe attributes. The correct pattern is `postMessage`/`addEventListener("message")` for cross-origin iframe communication. This is a JavaScript-only change in the embedded HTML, not a Rust dep change.

**Recommended**: Keep the current approach (srcdoc iframe with injected bridge script) for the preview server since it runs same-origin. The postMessage pattern is needed only in the WASM in-browser test client where the iframe is from a different origin.

### 2. WASM-Based In-Browser Test Client

**Current state**: `examples/wasm-client/src/lib.rs` exports `WasmClient` with `connect()`, `list_tools()`, `call_tool()`. It has no bridge injection capability.

**What's needed**: A `WasmAppsClient` that:
1. Creates an `<iframe>` element (needs `web-sys::HtmlIframeElement` feature)
2. Sets `srcdoc` to the widget HTML from `read_resource()`
3. Injects bridge via `contentWindow.postMessage` (needs `web-sys::Window`, `web-sys::MessageEvent`)
4. Listens for `mcpBridge.callTool()` calls from the iframe via `addEventListener("message")` (needs `web-sys::EventTarget`)
5. Forwards to the MCP server via existing `WasmClient` methods

**Required web-sys features** (add to `examples/wasm-client/Cargo.toml`):
```toml
web-sys = { version = "0.3", features = [
  "console",
  "WebSocket",
  # New for bridge:
  "MessageChannel",
  "MessagePort",
  "MessageEvent",
  "HtmlIframeElement",
  "HtmlElement",
  "Window",
  "EventTarget",
  "Document",
  "Element",
] }
```

Note: `Document` and `Window` are already in the root `pmcp` crate's WASM deps (in `Cargo.toml` line 103). The wasm-client is a separate crate excluded from the workspace, so it must declare them explicitly.

### 3. ChatGPT Manifest Generation

**What "manifest" means in practice**: OpenAI does not use a `manifest.json` file. The ChatGPT Apps SDK (as of Nov 2025) registers MCP Apps by:
1. Providing an MCP server URL in the OpenAI platform dashboard
2. The server responding to `tools/list` with tools that include `_meta.ui.resourceUri` annotations
3. ChatGPT fetching `ui://` resources to prefetch widget templates

**Generation strategy**: A `cargo pmcp manifest` command (or auto-generation during `deploy`) outputs a JSON summary document for developer reference — it is NOT submitted to OpenAI. The actual "registration" is the live MCP server.

**Stack**: `serde_json` (already in `cargo-pmcp`). No new deps.

**What to generate**: A human-readable JSON that documents the MCP server's tool-to-widget mapping, suitable for copy-pasting into the OpenAI platform dashboard submission form. Structure:

```json
{
  "server_url": "https://...",
  "tools": [
    {
      "name": "chess_board",
      "description": "Interactive chess game",
      "ui": { "resourceUri": "ui://chess-board" }
    }
  ]
}
```

### 4. Demo Landing Page Generation

**Purpose**: Auto-generated standalone HTML page that non-technical stakeholders can open in a browser to see a widget demo with a mock bridge (no live MCP server required).

**Stack**: `minijinja = "2"` in `mcp-preview`. The template expands to a self-contained HTML file (inline CSS, inline JS, inline widget HTML).

**Why minijinja over tera**:
- `tera` pulls in `regex`, `globbing`, `serde_json` deep integration — heavier compile cost
- `minijinja` version 2 is single-file, zero proc macros, ~2s compile overhead
- Jinja2 syntax is broadly familiar
- `minijinja` is used by the same author (mitsuhiko) who wrote Flask/Jinja2; the API is production-quality
- Latest stable: 2.15.1 (MEDIUM confidence — web search result, no direct crates.io verification)

**Why not handlebars**: handlebars-rs lacks filters and the template complexity for generating a full HTML page with conditional sections warrants Jinja2's `{% if %}` / `{% for %}` support.

### 5. `cargo pmcp new --mcp-apps` Scaffolding

**Current state**: `cargo-pmcp` has the CLI infra (clap, walkdir, colored). The `new` subcommand likely exists for basic projects. The `--mcp-apps` flag generates a project with:
- `src/main.rs` (MCP server with mcp-apps feature, example tool)
- `widgets/board.html` (starter widget HTML with bridge boilerplate)
- `Cargo.toml` (with `pmcp` and `mcp-apps` feature)
- `README.md`

**Stack**: `minijinja = "2"`. Templates are embedded via `include_str!()` (already a Rust builtin — zero deps). Variables: `{{project_name}}`, `{{tool_name}}`, `{{server_port}}`.

**Why not cargo-generate**: `cargo-generate` is a standalone external tool; users shouldn't need to install it separately. The scaffolding is simple enough (5 files, 3 variables) that `minijinja` + embedded templates are the right approach. cargo-generate's Liquid/Rhai complexity is overkill.

---

## Existing Dependencies — No Changes Needed

These are already in the relevant crates and do NOT need modification:

### mcp-preview (already present)
| Dep | Version | Used For | Status |
|-----|---------|---------|--------|
| `axum` | `"0.7"` → bump to `"0.8"` | HTTP server, WebSocket | Bump required (see above) |
| `tokio` | `"1"` | Async runtime | Unchanged |
| `tower-http` | `"0.6"` | CORS, static file serving | Unchanged |
| `rust-embed` | `"8"` | Embedded static assets (`assets/index.html`) | Unchanged |
| `mime_guess` | `"2"` | MIME type detection for asset serving | Unchanged |
| `serde` / `serde_json` | `"1"` | Config, proxy serialization | Unchanged |
| `reqwest` | `"0.12"` | HTTP proxy to MCP server | Unchanged — also handles `resources/read` for `ui://` URIs |
| `uuid` | `"1"` | Request ID generation | Unchanged |
| `tracing` | `"0.1"` | Logging | Unchanged |
| `anyhow` | `"1"` | Error handling | Unchanged |
| `futures` | `"0.3"` | WebSocket stream handling | Unchanged |

### wasm-client (already present)
| Dep | Version | Used For | Status |
|-----|---------|---------|--------|
| `pmcp` | `path = "../.."` | MCP protocol types + transports | Unchanged |
| `wasm-bindgen` | `"0.2"` | JS/Rust interop | Unchanged (pulls 0.2.108) |
| `wasm-bindgen-futures` | `"0.4"` | Async bridge | Unchanged |
| `serde` / `serde_json` | `"1"` | Type serialization | Unchanged |
| `serde-wasm-bindgen` | `"0.6"` | JsValue ↔ Rust conversion | Unchanged |
| `web-sys` | `"0.3"` | Browser APIs | Feature additions only (see above) |
| `console_error_panic_hook` | `"0.1"` | Panic messages in browser console | Unchanged |
| `tracing-wasm` | `"0.2"` | Tracing in browser | Unchanged |

### cargo-pmcp (already present)
| Dep | Version | Used For | Status |
|-----|---------|---------|--------|
| `clap` | `"4"` | CLI parsing | Unchanged — `--mcp-apps` flag added to existing `new` subcommand |
| `serde_json` | `"1"` | Manifest JSON generation | Unchanged |
| `reqwest` | `"0.12"` | Publishing API calls | Unchanged |
| `zip` | `"7.0"` | Landing page deployment archives | Unchanged |
| `colored` | `"3"` | Scaffolding output formatting | Unchanged |
| `indicatif` | `"0.18"` | Progress bars during build/deploy | Unchanged |

---

## Updated Cargo.toml Snippets

### `crates/mcp-preview/Cargo.toml` — Changes

```toml
[dependencies]
# CHANGE: bump from "0.7" to "0.8" (route syntax in server.rs already uses 0.8 style)
axum = { version = "0.8", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "fs"] }

# Embedded assets
rust-embed = "8"
mime_guess = "2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# HTTP client for MCP proxy
reqwest = { version = "0.12", features = ["json"] }

# NEW: Template engine for demo landing page generation
minijinja = "2"

# Utilities
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
anyhow = "1"
futures = "0.3"
```

### `examples/wasm-client/Cargo.toml` — Changes

```toml
[dependencies]
pmcp = { path = "../..", default-features = false, features = ["wasm"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde-wasm-bindgen = "0.6"
# CHANGE: add web-sys features for bridge injection
web-sys = { version = "0.3", features = [
  "console",
  "WebSocket",
  "MessageChannel",
  "MessagePort",
  "MessageEvent",
  "HtmlIframeElement",
  "HtmlElement",
  "Window",
  "EventTarget",
  "Document",
  "Element",
] }
console_error_panic_hook = "0.1"
tracing-wasm = "0.2"
```

### `cargo-pmcp/Cargo.toml` — Changes

Add to `[dependencies]`:
```toml
# NEW: Template engine for --mcp-apps scaffolding
minijinja = "2"
```

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Template engine (scaffolding) | `minijinja "2"` | `tera "1"` | tera pulls regex + globbing; larger compile footprint. minijinja is purpose-built for this exact use case (Jinja2 templates in Rust CLI tools). |
| Template engine (scaffolding) | `minijinja "2"` | `handlebars "6"` | handlebars has no filter support or block-level conditionals — insufficient for HTML generation with conditional sections. |
| Template engine (scaffolding) | `minijinja "2"` | `askama` | askama is compile-time, not runtime. Scaffolding templates need to be embedded strings expanded at runtime. |
| Scaffolding tool | Embedded templates + minijinja | `cargo-generate` | External tool; users must install separately. Our templates are simple (5 files, 3 variables). |
| Bridge communication | srcdoc + same-origin access | postMessage cross-origin | srcdoc iframes are same-origin with their parent; direct `window.parent` access works. postMessage is the right choice only for the WASM test client where iframes are cross-origin. |
| WASM packaging | `wasm-pack` | `trunk` | trunk is a full bundler (webpack-like) designed for full WASM SPAs. wasm-pack produces a `pkg/` directory we embed in the preview server or serve standalone. trunk adds unnecessary complexity. |
| Manifest format | serde_json generation from tool metadata | Separate manifest.json | OpenAI does not use a manifest.json; registration is through the platform dashboard pointing at the live MCP server URL. A generated JSON is documentation only. |
| Axum version | `"0.8"` | Keep `"0.7"` | The root `pmcp` crate already depends on `axum = "0.8.5"`. Two incompatible axum versions in one workspace would compile both, bloating the binary. |

---

## What NOT to Add

| Technology | Why Not | What to Use Instead |
|------------|---------|---------------------|
| `trunk` | Full SPA bundler — overkill for embedding a WASM client in a preview server. Requires webpack config, `index.html` entrypoint conventions, npm-style setup. | `wasm-pack build --target web` produces standalone ES module. |
| `leptos` or `yew` | Frontend frameworks. The preview UI is intentionally vanilla JS (single HTML file, no build step). Adding a framework would require a JS build pipeline. | Plain JavaScript in `assets/index.html`. Already implemented. |
| `axum-extra` | Optional axum extensions — none needed for this feature set. | Core `axum = "0.8"` with `ws` feature. |
| `tower-serve-static` | Wrapper around tower-http for embedded files. `rust-embed` + a custom handler (already implemented in `handlers/assets.rs`) does the same thing without an extra dep. | Existing `rust-embed` + `handlers::assets`. |
| `include_dir` | Alternative to `rust-embed`. No advantage here — `rust-embed` is already present and provides the same compile-time embedding. | `rust-embed = "8"` (already in mcp-preview). |
| `cargo-generate` (as a lib) | Using cargo-generate as a library dep brings its full dependency tree (liquid template, git2, etc.) for a 5-file template. | `minijinja` for template expansion, `include_str!()` for embedding. |
| `serde_dynamo` / DynamoDB / Redis | Out of scope for this milestone. Storage backends are v1.2 work. | Not applicable to MCP Apps DX. |
| `oauth2` (new in mcp-preview) | Auth for the preview server is unnecessary — it runs localhost only. cargo-pmcp already has oauth2 for publishing flows. | No auth in preview server. |

---

## Version Compatibility

| Package A | Version | Compatible With | Notes |
|-----------|---------|-----------------|-------|
| `axum` | `"0.8"` | `tower-http = "0.6"` | tower-http 0.6 is the correct companion to axum 0.8. Already in mcp-preview. |
| `axum` | `"0.8"` | `tokio-tungstenite = "0.26+"` | axum 0.8 upgraded tokio-tungstenite to 0.26. The `ws` feature in axum 0.8 handles this. |
| `wasm-bindgen` | `"0.2"` | `web-sys = "0.3"` | These are co-released; the `"0.2"` and `"0.3"` semver ranges always track together. No pinning needed. |
| `minijinja` | `"2"` | `serde = "1"` | minijinja 2.x uses serde for value serialization. Compatible with existing serde 1.x. |
| `minijinja` | `"2"` | `minijinja = "1"` (if present elsewhere) | They are separate major versions. If any dep transitively pulls minijinja 1, Cargo resolves both. Both are small; duplication is acceptable but avoid if possible. |
| `pmcp` root | `axum = "0.8.5"` | `mcp-preview axum = "0.8"` | After the bump, both use the same semver range. Cargo deduplicates to a single axum 0.8.x compile. |

---

## MCP Apps Spec Impact on Architecture (Not Stack)

The ext-apps specification (2026-01-26, now production-stable) defines:
- Tools declare `_meta.ui.resourceUri: "ui://widget-name"`
- Hosts call `resources/read` with that URI to get the HTML bundle
- Communication is postMessage JSON-RPC between iframe and host

**Stack consequence**: The `McpProxy` in `mcp-preview/src/proxy.rs` needs a `read_resource(uri: &str) -> Result<String>` method. This uses the existing `reqwest::Client` to call `resources/read` on the MCP server. **Zero new deps.** The existing `send_request()` private method handles this already.

The preview server's WebSocket handler already supports `CallTool` messages. It needs a companion `ReadResource` message type. **Zero new deps** — just additional match arms in `handlers/websocket.rs`.

---

## Playwright E2E Stack

The Playwright setup is already correct:

```json
{
  "@playwright/test": "^1.50.0",
  "@types/node": "^22.0.0",
  "typescript": "^5.7.0"
}
```

The `serve.js` static file server, `playwright.config.ts`, `fixtures/mock-mcp-bridge.ts`, and `tests/chess-widget.spec.ts` are already written. What's missing is the widget HTML files being served at the expected paths (`/chess/board.html`, `/map/explorer.html`). The `serve.js` serves from `examples/` — the chess example's `preview.html` must be placed at the path the tests expect, or the test paths updated to match the examples' structure.

**No new npm packages needed.** `@playwright/test 1.50.0` is current as of early 2026.

---

## Build Pipeline for wasm-pack

The WASM test client needs a build step before embedding:

```bash
# In examples/wasm-client/
wasm-pack build --target web --out-dir pkg

# The mcp-preview server then serves pkg/ assets
# OR: pkg/ is embedded via rust-embed at compile time
```

**Recommended**: Add to `justfile` at workspace root:

```just
build-wasm:
    cd examples/wasm-client && wasm-pack build --target web --out-dir pkg
```

The `mcp-preview` crate can then `include!` the built WASM via `rust-embed` pointing at the build output path (requires `BUILD_WASM=1` to run `build-wasm` before cargo build). This is standard practice; see examples in the wasm-pack documentation.

**wasm-pack version**: Use whatever `cargo install wasm-pack` installs (0.13.x as of early 2026). No Cargo.toml dep — it's a standalone tool.

---

## Sources

- [MCP Apps ext-apps spec 2026-01-26](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx) — `ui://` resource URI scheme, `_meta.ui.resourceUri` tool annotations, postMessage protocol (MEDIUM confidence — web search, structure described but not directly fetched)
- [OpenAI Apps SDK docs](https://developers.openai.com/apps-sdk/) — No manifest.json; registration via platform dashboard + live MCP server (MEDIUM confidence)
- [minijinja on crates.io](https://crates.io/crates/minijinja) — version 2.15.1 as of web search (MEDIUM confidence — web search result)
- [wasm-bindgen on crates.io](https://crates.io/crates/wasm-bindgen) — latest 0.2.108 as of Jan 2026 (MEDIUM confidence — web search result)
- [Announcing axum 0.8.0](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) — Breaking changes: path syntax `/{param}`, WebSocket Message uses Bytes (HIGH confidence — official tokio blog)
- [web-sys docs](https://docs.rs/web-sys/latest/web_sys/) — MessageChannel, MessagePort, HtmlIframeElement, Window features available (HIGH confidence — official docs)
- [Playwright npm package](https://www.npmjs.com/package/playwright) — version 1.50.0 is current in early 2026 (MEDIUM confidence — web search)
- Codebase analysis: `crates/mcp-preview/Cargo.toml` — existing axum 0.7, current deps
- Codebase analysis: `crates/mcp-preview/src/server.rs` — route uses `/{*path}` (axum 0.8 syntax, confirming the version mismatch)
- Codebase analysis: `crates/mcp-preview/assets/index.html` — bridge JavaScript implementation status (~90% complete)
- Codebase analysis: `examples/wasm-client/Cargo.toml` — existing web-sys features
- Codebase analysis: `cargo-pmcp/Cargo.toml` — existing deps, no templating engine present
- Codebase analysis: `tests/playwright/package.json`, `playwright.config.ts` — Playwright already set up
- Project doc: `.planning/PROJECT.md` — v1.3 milestone scope

---
*Stack research for: MCP Apps Developer Experience (v1.3 milestone)*
*Researched: 2026-02-24*
*Key findings:*
*1. THREE new deps total: `minijinja "2"` in mcp-preview, `minijinja "2"` in cargo-pmcp, web-sys feature additions in wasm-client.*
*2. ONE critical fix: bump mcp-preview axum from "0.7" to "0.8" (server.rs already uses 0.8 route syntax).*
*3. ChatGPT "manifest" is not a file — it's the tools/list response with _meta.ui annotations.*
*4. Playwright E2E stack is already complete; missing piece is widget HTML files at expected paths.*
*5. MCP Apps spec went production-stable 2026-01-26; PMCP's existing type support is aligned.*
