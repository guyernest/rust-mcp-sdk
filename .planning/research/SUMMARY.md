# Research Summary: MCP Apps Developer Experience (v1.3)

**Project:** PMCP SDK -- MCP Apps Developer Experience
**Domain:** MCP Apps developer tooling (widget preview, WASM bridge, publishing, authoring DX, E2E testing)
**Researched:** 2026-02-24
**Overall confidence:** HIGH

## Executive Summary

The v1.3 milestone transforms the PMCP SDK's MCP Apps story from "types and adapters exist" to "a developer can author, preview, test, and publish an MCP App entirely from the Rust toolchain." The infrastructure is 60-80% built: mcp-preview has an Axum server with tool list/call proxy, asset serving, and a full preview UI; the WASM client has dual-transport connect/list/call; chess and map examples have working server code and widget HTML; Playwright scaffolding has 10 tests written with a mock bridge fixture. The remaining work is integration -- wiring the pieces together so that widget iframes actually render with a working bridge, the WASM client can be used as an in-browser bridge alternative, and the publishing pipeline generates ChatGPT-compatible manifests.

The recommended approach is bridge-first: fix the mcp-preview bridge injection (replace broken `window.parent.previewRuntime` cross-origin access with postMessage or same-origin `srcdoc` proxy, add `resources/read` proxy), then layer WASM bridge support, shared bridge library, authoring scaffolding, and publishing on top. The dependency chain is strict -- nothing downstream works until the preview bridge renders widgets with live tool call routing. Stack changes are minimal: bump mcp-preview's axum from 0.7 to 0.8 (code already uses 0.8 syntax), add `minijinja "2"` to mcp-preview and cargo-pmcp for template generation, and expand web-sys features in the WASM client. Three new deps total across the workspace.

The key risks are: (1) bridge contract divergence between the preview mock and ChatGPT's Skybridge runtime causing widgets to work in dev but fail in production -- mitigated by defining a shared `McpBridgeContract` TypeScript interface and a canonical `widget-runtime.js`; (2) postMessage wildcard origin (`'*'`) in the bridge code creating a CVE-class security vulnerability -- mitigated by implementing origin handshake on all bridge paths; (3) WASM client hardcoded request IDs causing concurrent call corruption -- mitigated by adding an atomic counter before exposing WASM to widget authors.

## Key Findings

### Recommended Stack

Three new dependencies total. No experimental or unstable libraries. The stack is deliberately conservative.

**Core changes:**
- `axum "0.8"` in mcp-preview: Version bump (from 0.7) to align with root pmcp crate. Route syntax in `server.rs` already uses 0.8 `/{*path}` patterns. Eliminates duplicate axum compilation.
- `minijinja "2"` in mcp-preview + cargo-pmcp: Template engine for demo landing pages and `--mcp-apps` scaffolding. Single-file, zero proc macros, Jinja2 syntax. Chosen over tera (heavier deps), handlebars (no filters), askama (compile-time only).
- `web-sys` feature additions in wasm-client: `MessageChannel`, `MessagePort`, `MessageEvent`, `HtmlIframeElement`, `Window`, `EventTarget`, `Document`, `Element`. Feature flags on the existing dep, not new crates.

**Build tooling:**
- `wasm-pack` (0.13.x) for WASM client builds. Already the standard tool; needs `just` recipe.
- Playwright `@playwright/test ^1.50.0` already in `tests/playwright/package.json`. No npm changes needed.

**Explicitly not adding:** trunk (overkill), leptos/yew (framework complexity), cargo-generate (external tool dependency), axum-extra (unnecessary).

### Expected Features

**Must have (table stakes -- milestone fails without these):**
- Widget iframe renders from resource URI via `resources/read` proxy (mcp-preview)
- `window.mcpBridge.callTool()` routes to real MCP server through postMessage bridge
- MCP session initialization happens once, not per-request (McpProxy fix)
- Chess and map examples compile and run (`cargo build --features mcp-apps`)
- Playwright chess widget tests pass (10 tests against real widget HTML selectors)
- `cargo pmcp new --mcp-apps` scaffolding generates a working MCP App project

**Should have (differentiators):**
- Shared bridge library (`widget-runtime.js`) eliminating copy-paste across widgets
- ChatGPT manifest generation (`cargo pmcp manifest`)
- WASM in-browser test client as alternative bridge mode in preview
- Demo landing page generation (`cargo pmcp landing`)
- Bridge API TypeScript type definitions (`widget-runtime.d.ts`)
- Playwright test report with screenshots

**Defer to v1.4+:**
- WASM bridge polyfill (bundled `widget-runtime.js` with WASM client inside)
- Integration Playwright test (preview server + real MCP server)
- Auto-reload on server restart via WebSocket
- Multiple widget resources panel in preview UI
- Manifest validation against ChatGPT schema
- Widget accessibility tests (axe-core)

### Architecture Approach

The architecture is a six-component system with strict dependency flow: `McpProxy` (HTTP-to-JSON-RPC translation with session persistence) feeds `BridgeInjector` (pure HTML transform that inserts preview bridge JS), which feeds `handlers::widget` (the new `/widget-proxy` endpoint). The shared bridge library (`mcp-bridge-js`) defines the contract all bridge implementations must satisfy. The WASM `WidgetRenderer` implements the same contract through postMessage. `cargo-pmcp` commands consume the preview infrastructure for scaffolding, manifests, and demos.

**Major components:**
1. `McpProxy` (modified) -- HTTP proxy to MCP server with session persistence, `resources/read` + `resources/list`, atomic request IDs
2. `BridgeInjector` (new) -- Pure function: HTML in, HTML+bridge-script out. Injects `window.mcpBridge` that routes `callTool()` to preview server's `/api/tools/call`
3. `handlers::widget` (new) -- `/widget-proxy?uri=<resource-uri>` endpoint. Fetches resource HTML, runs through BridgeInjector, serves with iframe-friendly headers
4. `mcp-bridge-js` / `widget-runtime.js` (new) -- Shared JS bridge library. Detects environment (ChatGPT/preview/standalone). Single source of truth for bridge API contract
5. `WasmClient::WidgetRenderer` (new) -- Creates iframe, injects bridge via postMessage, routes tool calls through WASM HTTP transport
6. `cargo-pmcp` commands (modified/new) -- `new --mcp-apps`, `manifest`, `landing` commands

### Critical Pitfalls

Research identified 7 critical pitfalls. These are the top 5 that must be addressed during implementation:

1. **McpProxy re-initializes on every request (no session stickiness)** -- `list_tools()` calls `initialize()` before every request. Breaks stateful MCP servers, wastes RTTs. Fix: `OnceCell<String>` for session ID, forward `Mcp-Session-Id` header. Address in Phase 1.

2. **postMessage wildcard origin `'*'` in bridge code (CVE-class vulnerability)** -- Both `ChatGptAdapter::inject_bridge()` and the preview bridge send postMessage with `'*'` target and accept messages without `event.origin` validation. Fix: origin handshake protocol. Address in Phase 1.

3. **WASM client hardcoded request IDs (concurrent call corruption)** -- `call_tool()` uses `3i64`, `list_tools()` uses `2i64`. Concurrent widget calls collide. Fix: `AtomicI64` counter. Address in Phase 2.

4. **Bridge contract divergence between preview mock and production** -- Preview returns raw game state; ChatGPT wraps in `{ content: [...] }` envelope. Widgets work in dev, fail in production. Fix: define shared `McpBridgeContract` interface, enforce in both bridges. Address across Phases 1-3.

5. **`html.replace("</head>", ...)` double-injects on template HTML** -- `str::replace` replaces ALL occurrences. HTML with `</head>` in scripts/comments gets bridge injected multiple times. Fix: use `replacen(..., 1)` or position-based insertion. Address in Phase 1.

## Implications for Roadmap

Based on combined research, suggested phase structure (6 phases, ~13 days):

### Phase 1: Preview Bridge Infrastructure
**Rationale:** Everything downstream depends on widgets actually rendering with a working bridge in mcp-preview. This is the single blocking dependency for the entire milestone.
**Delivers:** Working widget preview -- developer runs `cargo pmcp preview`, sees widget, clicks buttons, tools fire.
**Addresses features:** Widget iframe from resource URI, `callTool()` bridge routing, MCP session init, resource fetching, DevTools updates, connection status.
**Avoids pitfalls:** McpProxy re-initialization (Pitfall 1), postMessage wildcard origin (Pitfall 2), HTML injection double-fire (Pitfall 4), bridge contract divergence (Pitfall 6 -- defines the contract).
**Stack changes:** Bump axum 0.7 to 0.8 in mcp-preview.
**Estimated effort:** 3 days.

### Phase 2: WASM Widget Bridge
**Rationale:** Once the bridge protocol is validated in Phase 1, the WASM client implements the same protocol as an in-browser alternative. This is a differentiator (only Rust SDK with browser-native MCP client) but not blocking.
**Delivers:** WASM client can render widgets in iframes with postMessage bridge. Side-by-side comparison mode in preview (proxy vs WASM).
**Addresses features:** WASM client loads in preview context, bridge adapter (WASM -> mcpBridge shape), CORS handling, connection URL configuration.
**Avoids pitfalls:** Hardcoded request IDs (Pitfall 3), WASM binary size bloat (Pitfall 7).
**Stack changes:** web-sys feature additions in wasm-client.
**Estimated effort:** 2 days.

### Phase 3: Shared Bridge Library
**Rationale:** After two bridge implementations (proxy in Phase 1, WASM in Phase 2), the API contract is proven. Extract into a shared library to eliminate copy-paste and enforce the contract.
**Delivers:** `widget-runtime.js` served by preview at `/widget-runtime.js`. TypeScript `.d.ts` types. Chess and map examples updated to use it.
**Addresses features:** Shared bridge library, bridge API type definitions, environment detection (ChatGPT/preview/standalone).
**Avoids pitfalls:** Divergent bridge APIs (Pitfall 6).
**Stack changes:** None (plain JavaScript/TypeScript, no Rust deps).
**Estimated effort:** 2 days.

### Phase 4: Widget Authoring DX + Scaffolding
**Rationale:** With the bridge library from Phase 3, scaffolding can reference it. The authoring DX features make widget development pleasant.
**Delivers:** `cargo pmcp new --mcp-apps` scaffolding, `pmcp::widget!()` macro helper, `widgets/` directory convention in examples.
**Addresses features:** File-based widget authoring, scaffolding template, shared bridge in scaffolded `preview.html`, widget CSP configuration helper, stateless widget pattern in template.
**Uses stack:** `minijinja "2"` in cargo-pmcp for template expansion.
**Estimated effort:** 2 days.

### Phase 5: Publishing Pipeline
**Rationale:** Publishing depends on working widgets (Phase 1), file-based structure (Phase 4), and bridge library (Phase 3). Implements the deployment-to-ChatGPT workflow.
**Delivers:** `cargo pmcp manifest` for ChatGPT manifest generation, `cargo pmcp landing` for demo pages, HTTPS URL validation, `text/html+skybridge` MIME verification.
**Addresses features:** ChatGPT manifest generation, demo landing page, deploy `--mcp-apps` flag, submission checklist.
**Avoids pitfalls:** HTTPS silent rejection (Pitfall 5).
**Uses stack:** `minijinja "2"` in mcp-preview for landing page templates, `serde_json` for manifest generation.
**Estimated effort:** 2 days.

### Phase 6: Ship Examples + Playwright E2E
**Rationale:** Integration validation. Examples exercise every piece of the toolchain. Playwright tests prove it all works end-to-end. Must come last because it depends on all previous phases.
**Delivers:** Chess and map examples in final form, Playwright test suite green in CI, map widget tests, example READMEs.
**Addresses features:** Chess/map examples compile and run, Playwright test server wired to widget directories, widget HTML data attributes match test selectors, screenshot reports.
**Estimated effort:** 2 days.

### Phase Ordering Rationale

- **Phase 1 before everything:** The preview bridge is the load-bearing wall. WASM bridge (Phase 2), shared library (Phase 3), scaffolding (Phase 4), publishing (Phase 5), and tests (Phase 6) all depend on the bridge protocol working.
- **Phase 2 before Phase 3:** Building two independent bridge implementations (proxy and WASM) before extracting the shared library ensures the abstraction is correct. Extract after proving, not before.
- **Phase 4 after Phase 3:** Scaffolding templates reference `widget-runtime.js`. The template is wrong if the bridge library does not exist yet.
- **Phase 5 after Phase 4:** Manifest generation reads `pmcp.toml [mcp-apps]` which is scaffolded in Phase 4. Demo landing page uses the file-based widget structure from Phase 4.
- **Phase 6 last:** E2E tests validate the complete pipeline. They cannot be written until all components exist. The chess/map examples serve as integration fixtures.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** The `resources/read` proxy call and postMessage origin handshake pattern need careful specification. The bridge injection point (`</head>` vs fallback) needs test cases for edge-case HTML. Research recommended.
- **Phase 2:** WASM module loading in iframe `srcdoc` context is novel (limited prior art). The `&mut self` / concurrent-call problem in wasm-bindgen needs design attention. Research recommended.
- **Phase 5:** ChatGPT App Directory submission process is opaque (human review). Manifest format may evolve. MEDIUM confidence. Research recommended.

Phases with standard patterns (skip research-phase):
- **Phase 3:** Shared JavaScript library with environment detection is a well-documented pattern. Standard.
- **Phase 4:** CLI scaffolding with template expansion is standard Rust CLI practice. cargo-pmcp already has the infrastructure. Standard.
- **Phase 6:** Playwright E2E with mock bridge is standard. Tests are already written. Standard.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Only 3 new deps, all mature. Axum 0.8 bump is well-documented. minijinja is production-quality. web-sys features are official. |
| Features | HIGH | Five feature areas well-defined. Table stakes vs differentiators clearly separated. Anti-features identified (no React templates, no HMR, no CDN hosting). Existing codebase is 60-80% complete. |
| Architecture | HIGH | Based on direct codebase analysis of all existing components. Component boundaries, data flows, and build order derived from actual code, not hypothetical design. |
| Pitfalls | HIGH | 7 critical pitfalls identified from codebase analysis and domain research. All have concrete code-level prevention strategies. The postMessage security pitfall is backed by a real CVE (CVE-2024-49038). |

**Overall confidence:** HIGH

### Gaps to Address

- **ChatGPT App Directory submission process:** The submission is human-reviewed with opaque acceptance criteria. Manifest format is documented but the review process is not. Validate manifest structure against real submissions when Phase 5 is planned.
- **WASM module loading in srcdoc iframes:** No established pattern for loading WASM modules inside `srcdoc`-based iframes. May need blob URL or `importmap` approach. Prototype during Phase 2 planning.
- **MCP Apps spec evolution:** The ext-apps specification went production-stable 2026-01-26 but is still young. If the spec changes `_meta.ui.resourceUri` semantics or adds new host requirements, the bridge and manifest code may need updates. Monitor the spec repo.
- **Bridge contract completeness:** The research identifies `callTool`, `getState`, `setState` as the core bridge API. The full contract (including `listTools`, `readResource`, `onStateChange`, lifecycle events) needs specification during Phase 1 planning.
- **Axum 0.8 WebSocket breaking change:** Axum 0.8 changed `Message` to use `Bytes` instead of `Vec<u8>`. The existing WebSocket handler in mcp-preview may need updates. Verify during Phase 1 implementation.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/mcp-preview/` -- server.rs, proxy.rs, handlers/, assets/index.html (bridge implementation ~90% complete, cross-origin bug identified)
- Codebase analysis: `examples/wasm-client/src/lib.rs` -- WASM client capabilities, hardcoded ID bug, missing bridge injection
- Codebase analysis: `examples/mcp-apps-chess/` and `mcp-apps-map/` -- complete server code, widget HTML, preview.html mock bridge
- Codebase analysis: `cargo-pmcp/src/commands/` -- preview.rs, new.rs (no --mcp-apps template exists)
- Codebase analysis: `src/types/mcp_apps.rs` -- all MCP Apps types present and working
- Codebase analysis: `tests/playwright/` -- 10 chess tests written, mock-mcp-bridge.ts fixture, config scaffolded
- [Axum 0.8.0 announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) -- path syntax changes, WebSocket Bytes migration
- [web-sys docs](https://docs.rs/web-sys/latest/web_sys/) -- feature flags for MessageChannel, HtmlIframeElement, Window

### Secondary (MEDIUM confidence)
- [MCP Apps ext-apps spec 2026-01-26](https://github.com/modelcontextprotocol/ext-apps) -- `ui://` resource URI scheme, postMessage protocol, production-stable
- [MCP Apps Blog 2026-01-26](http://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/) -- host support (Claude, VS Code, Goose, Postman, MCPJam)
- [OpenAI Apps SDK](https://developers.openai.com/apps-sdk/) -- ChatGPT deployment requirements, HTTPS mandatory, no manifest.json file
- [ChatGPT Apps Developer Mode](https://help.openai.com/en/articles/12584461-developer-mode-apps-and-full-mcp-connectors-in-chatgpt-beta) -- App Directory submission process
- [minijinja on crates.io](https://crates.io/crates/minijinja) -- version 2.15.1
- [PostMessage security -- Microsoft MSRC](https://www.microsoft.com/en-us/msrc/blog/2025/08/postmessaged-and-compromised) -- CVE-2024-49038 reference
- [wasm-bindgen pitfalls](https://www.rossng.eu/posts/2025-01-20-wasm-bindgen-pitfalls/) -- recursive object use, async aliasing

### Tertiary (LOW confidence)
- [MCP Apps playground](https://github.com/digitarald/mcp-apps-playground) -- community widget authoring patterns
- [SEP-1865 PR](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865) -- original proposal background

---
*Research completed: 2026-02-24*
*Ready for roadmap: yes*
