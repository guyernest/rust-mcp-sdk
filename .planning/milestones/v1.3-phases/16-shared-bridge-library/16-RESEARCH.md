# Phase 16: Shared Bridge Library - Research

**Researched:** 2026-02-25
**Domain:** TypeScript/JavaScript bridge library, MCP Apps protocol alignment, Rust asset serving
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Align with the official MCP Apps postMessage/JSON-RPC protocol (`ui/initialize`, `tools/call`, etc.) -- not a custom `window.mcpBridge` API
- Preview server serves the `@modelcontextprotocol/ext-apps` JS library (or equivalent) at a stable URL for convenience, but widgets can also bring their own bundled copy
- Focus on the subset needed for interactive views (chess board, maps) -- not the full MCP Apps surface
- Early days for MCP Apps -- stick to the official spec, don't over-extend
- Host provides the postMessage interface uniformly across all modes -- widget code is identical whether running in preview, WASM standalone, or production (Claude Desktop, VS Code, etc.)
- WASM standalone mode becomes a "mini host" implementation: it creates the iframe, handles postMessage, and proxies tool calls through the WASM MCP client
- Graceful degradation with console warnings when a host doesn't support a particular capability -- widget should not crash
- Widget-runtime.js is an ES module (`<script type="module">`)
- Built from TypeScript source -- compiled to JS as part of the build process (enterprise/security focus -- type-safe source of truth)
- `cargo pmcp app` -- new subcommand scaffolds a widget/app project with the bridge library, TypeScript types, and best practices baked in
- Align type definitions with `@modelcontextprotocol/ext-apps` type signatures for consistent widget author experience
- Type only the methods we actually implement -- no phantom types for unimplemented capabilities

### Claude's Discretion
- Whether to migrate existing chess/map examples to the official protocol in this phase or separately
- Preview server's approach to hosting the iframe (AppBridge pattern vs inline injection)
- Bundling strategy (single file vs modular imports)
- URL path for serving the bridge library
- TypeScript source directory location within the repo

### Deferred Ideas (OUT OF SCOPE)
- Full MCP Apps protocol coverage (context updates, streaming tool inputs, permissions negotiation) -- future phase once the base bridge is stable
- Publishing widget-runtime as a standalone npm package -- consider after the API stabilizes
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DEVX-03 | Shared bridge library (`widget-runtime.js`) eliminates copy-pasted bridge code across widgets | Core deliverable: TypeScript source compiled to single ES module, served by preview server at stable URL, replaces inline bridge injection in `wrapWidgetHtmlProxy()` and `wrapWidgetHtmlWasm()` |
| DEVX-05 | Bridge API TypeScript type definitions (`widget-runtime.d.ts`) ship with bridge library | TypeScript source with strict types aligned to `@modelcontextprotocol/ext-apps` API surface; `tsup` build produces `.d.ts` alongside `.mjs` output |
</phase_requirements>

## Summary

Phase 16 extracts the duplicated bridge code from three locations -- the preview server's proxy bridge injection (`wrapWidgetHtmlProxy`), the WASM bridge injection (`wrapWidgetHtmlWasm`), and the standalone `widget-runtime.js` IIFE -- into a single TypeScript-sourced ES module library aligned with the official MCP Apps protocol. The existing `packages/widget-runtime/` directory already contains a comprehensive `@pmcp/widget-runtime` package with types, runtime class, React hooks, and utilities. However, the existing code uses a `window.mcpBridge` API pattern rather than the official MCP Apps `postMessage`/JSON-RPC protocol. This phase must reconcile these two approaches.

The official `@modelcontextprotocol/ext-apps` SDK (v1.1.2) uses a `App` class that communicates via `PostMessageTransport` with JSON-RPC 2.0 messages. Its key methods are `callServerTool()`, `sendMessage()`, `sendLog()`, `openLink()`, with lifecycle callbacks `ontoolinput`, `ontoolresult`, `ontoolcancelled`, `onhostcontextchanged`, and `onteardown`. The initialization flow is: (1) create `App` instance, (2) register handlers, (3) call `app.connect()` which performs a `ui/initialize` handshake via postMessage. The user's decision is to align with this protocol for the subset needed for interactive views.

**Primary recommendation:** Restructure `packages/widget-runtime/` to export an `App`-compatible class that uses `PostMessageTransport` for production hosts and can fall back to direct function calls in preview mode. Build with `tsup` to produce a single-file ES module. Serve the compiled output from the preview server at `/assets/widget-runtime.js`. Ship `.d.ts` alongside.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| TypeScript | ^5.0 | Type-safe source for bridge library | Already in workspace `packages/widget-runtime/tsconfig.json`; enterprise requirement per CONTEXT.md |
| tsup | ^8.0 | Bundle TS to single-file ESM + CJS + DTS | Already configured in `packages/widget-runtime/package.json`; zero-config bundler built on esbuild |
| @modelcontextprotocol/ext-apps | ^1.1.2 | Reference types for MCP Apps protocol alignment | Official SDK; type signatures are the alignment target per CONTEXT.md |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| vitest | ^1.0 | Unit testing the bridge library | Already configured; run with `npm test` in `packages/widget-runtime/` |
| rust_embed | (existing) | Embed compiled JS into Rust binary | Already used in `crates/mcp-preview/src/assets.rs` for asset serving |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tsup | Vite library mode | tsup is simpler for library output; Vite better for app bundles. tsup is already configured. |
| Single-file bundle | Modular ESM imports | Single file is simpler for `<script type="module" src="...">` inclusion; modular imports require a bundler on the widget side. Single file wins for the `cargo pmcp app` scaffolding use case. |
| Re-exporting @modelcontextprotocol/ext-apps | Wrapping it | Re-exporting the official SDK directly would add ~50KB+ and Node.js dependencies. Better to align types/API surface but implement our own lightweight version. |

**Installation:**
```bash
# Already installed in packages/widget-runtime/
cd packages/widget-runtime && npm install
npm run build  # produces dist/index.mjs, dist/index.js, dist/index.d.ts
```

## Architecture Patterns

### Recommended Project Structure
```
packages/widget-runtime/
  src/
    index.ts              # Public exports
    app.ts                # App class (MCP Apps protocol aligned)
    transport.ts          # PostMessageTransport for iframe<->host
    types.ts              # Types aligned with @modelcontextprotocol/ext-apps
    host-adapter.ts       # Host-side bridge for preview server
    utils.ts              # Utilities (detectHost, etc.)
    hooks.ts              # React hooks (tree-shakeable)
  dist/
    index.mjs             # ES module bundle (served by preview server)
    index.js              # CJS bundle (for Node.js tooling)
    index.d.ts            # Type declarations
    index.d.mts           # ES module type declarations
  tsconfig.json
  package.json

crates/mcp-preview/
  assets/
    widget-runtime.js     # Symlink or copy of dist/index.mjs
  src/
    handlers/assets.rs    # Serves /assets/* including widget-runtime.js

cargo-pmcp/
  src/
    commands/
      app.rs              # New: `cargo pmcp app` scaffolding subcommand
```

### Pattern 1: MCP Apps Protocol-Aligned App Class
**What:** Widget-side `App` class that uses postMessage JSON-RPC for host communication
**When to use:** All widgets -- this is the canonical bridge
**Example:**
```typescript
// Source: @modelcontextprotocol/ext-apps App class pattern
import { App } from './widget-runtime.js';

const app = new App({ name: 'Chess', version: '1.0.0' });

app.ontoolinput = (params) => {
  // Receive tool arguments from host
  renderBoard(params.arguments);
};

app.ontoolresult = (result) => {
  // Receive tool execution results
  updateDisplay(result);
};

await app.connect();

// Call a tool on the server
const result = await app.callServerTool({
  name: 'chess_move',
  arguments: { move: 'e2e4', state: currentState }
});
```

### Pattern 2: Preview Server as Mini Host
**What:** Preview server wraps widget HTML in an iframe and acts as the "host" side of postMessage
**When to use:** The preview server (`cargo pmcp preview`)
**Example:**
```typescript
// Host-side (injected by preview server into the outer page)
// Creates iframe, sets up postMessage transport, proxies tool calls to MCP server
import { AppBridge } from './widget-runtime.js';

const bridge = new AppBridge({
  iframe: document.getElementById('widget-frame'),
  toolCallHandler: async (name, args) => {
    // Proxy to MCP server via preview API
    const response = await fetch('/api/tools/call', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, arguments: args })
    });
    return response.json();
  }
});

bridge.initialize();
```

### Pattern 3: WASM Standalone as Mini Host
**What:** The WASM standalone runtime acts as a host, creating an iframe and handling postMessage
**When to use:** Widgets running with `widget-runtime.js` + WASM client outside any chat host
**Example:**
```typescript
// widget-runtime.js in standalone/WASM mode creates an AppBridge
// that uses the WASM MCP client instead of fetch proxy
import { AppBridge } from './widget-runtime.js';

const wasmClient = await initWasmClient(mcpServerUrl);

const bridge = new AppBridge({
  iframe: widgetIframe,
  toolCallHandler: async (name, args) => {
    const result = await wasmClient.call_tool(name, args);
    return { content: result.content, isError: result.isError };
  }
});
```

### Anti-Patterns to Avoid
- **Inline bridge injection:** The current approach of injecting bridge code via `srcdoc` string concatenation in `wrapWidgetHtmlProxy()` and `wrapWidgetHtmlWasm()` duplicates 100+ lines of JavaScript. Replace with a `<script type="module" src="/assets/widget-runtime.js">` tag.
- **`window.mcpBridge` global:** The current custom API (`window.mcpBridge.callTool()`) is not aligned with the MCP Apps spec. Widgets should use the `App` class from the bridge library instead.
- **IIFE bundle format:** The current `widget-runtime.js` is a self-executing IIFE. ES module format is the locked decision.
- **Building the full official SDK into the bundle:** The `@modelcontextprotocol/ext-apps` package has Node.js dependencies. Align API surface and types, but implement a lightweight version.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| postMessage serialization | Custom message format | JSON-RPC 2.0 with correlation IDs | Spec-compliant; hosts expect this format |
| TypeScript compilation | Manual tsc + file concat | tsup (already configured) | Handles bundling, DTS generation, source maps in one step |
| Asset embedding in Rust | Manual include_bytes!() | rust_embed (already used) | Handles MIME types, caching headers automatically |
| JSON-RPC request/response matching | Manual ID tracking | Shared `PendingRequests` map pattern from ext-apps | Race conditions, timeout handling, cancellation are solved problems |

**Key insight:** The hard part is not the bridge code itself -- it is maintaining API contract compatibility across three modes (preview proxy, WASM standalone, production host). The official MCP Apps protocol solves this by making postMessage the universal transport. All modes speak the same protocol; only the host-side implementation differs.

## Common Pitfalls

### Pitfall 1: srcdoc CSP Blocking Module Imports
**What goes wrong:** When a widget is loaded via `iframe.srcdoc`, `<script type="module" src="/assets/widget-runtime.js">` fails because `srcdoc` has a `null` origin that blocks cross-origin module imports.
**Why it happens:** srcdoc iframes have `null` origin by default; ES module requests are subject to CORS.
**How to avoid:** Use `iframe.src` pointing to a URL served by the preview server (e.g., `/widget?uri=...`) instead of `srcdoc`. Or inline the module source in a `<script type="module">` block with the code directly (not via `src`). The preview server already serves assets at `/assets/*`, so using `iframe.src` with a generated page is the cleanest approach.
**Warning signs:** "Failed to fetch module" errors in the browser console.

### Pitfall 2: postMessage Origin Validation
**What goes wrong:** Using `'*'` as the target origin in `postMessage()` creates a security vulnerability where any window can intercept messages.
**Why it happens:** During development, using `'*'` is convenient but it leaks into production.
**How to avoid:** The host (preview server) should set a specific origin when sending messages. The widget's `PostMessageTransport` should validate `event.origin` against the expected host origin. Note: STATE.md already flags this as a known concern: "postMessage wildcard origin ('*') in bridge code is a CVE-class vulnerability."
**Warning signs:** Security audit flags, messages intercepted by third-party scripts.

### Pitfall 3: Build Artifact Staleness
**What goes wrong:** The preview server embeds `widget-runtime.js` at compile time via `rust_embed`. If the TypeScript is rebuilt but the Rust binary is not recompiled, the embedded JS is stale.
**Why it happens:** Two-stage build: TypeScript -> JS, then Rust embeds JS.
**How to avoid:** Add a build step to the `Makefile`/`justfile` that runs `npm run build` in `packages/widget-runtime/` before `cargo build` for `mcp-preview`. Document this in the developer guide. Consider a `build.rs` script in `mcp-preview` that runs the TypeScript build automatically.
**Warning signs:** Widget behavior doesn't match source code changes after editing TypeScript.

### Pitfall 4: Breaking the `window.mcpBridge` Contract During Migration
**What goes wrong:** Existing chess and map widgets use `window.mcpBridge.callTool()`. If the shared library removes this API without updating widgets, they break.
**Why it happens:** Protocol migration without backward compatibility.
**How to avoid:** The shared library should provide a compatibility shim that exposes `window.mcpBridge` backed by the new `App` class internally. Or: migrate widgets in this phase (Claude's discretion item). Recommendation: include a lightweight `window.mcpBridge` compatibility layer that logs deprecation warnings and delegates to the `App` class.
**Warning signs:** Chess/map examples stop working after the bridge library change.

### Pitfall 5: React Peer Dependency Bloat
**What goes wrong:** Including React hooks in the main bundle adds React as a required dependency for all widgets, even vanilla JS ones.
**Why it happens:** React hooks import from 'react' at module scope.
**How to avoid:** Keep hooks in a separate entry point (`widget-runtime/react`) or use dynamic imports. The existing code already marks React as tree-shakeable but `import { useState } from 'react'` at the top of `hooks.ts` means React must be present. Use a separate tsup entry point: `tsup src/index.ts src/hooks.ts --format esm --dts`.
**Warning signs:** "Cannot find module 'react'" errors in vanilla JS widgets.

## Code Examples

### Example 1: Widget Using the Shared Bridge Library
```html
<!-- Widget HTML (served by MCP server as a ui:// resource) -->
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <script type="module">
    import { App } from '/assets/widget-runtime.js';

    const app = new App({ name: 'ChessWidget', version: '1.0.0' });

    app.ontoolresult = (result) => {
      if (result.structuredContent) {
        updateBoard(result.structuredContent);
      }
    };

    await app.connect();

    // Make a move
    document.getElementById('board').addEventListener('click', async (e) => {
      const result = await app.callServerTool({
        name: 'chess_move',
        arguments: { move: selectedMove, state: gameState }
      });
      if (!result.isError) {
        gameState = result.structuredContent;
        renderBoard(gameState);
      }
    });
  </script>
</head>
<body>
  <div id="board"></div>
</body>
</html>
```

### Example 2: Preview Server Bridge Injection (Simplified)
```javascript
// Instead of 100+ lines of inline bridge code,
// the preview server wraps widget HTML with:
const wrappedHtml = `
<!DOCTYPE html>
<html>
<head>
  <script type="module" src="/assets/widget-runtime.js"></script>
</head>
<body>
${widgetHtml}
</body>
</html>`;

// The preview server's outer page creates an AppBridge
// that handles postMessage and proxies to MCP server
```

### Example 3: TypeScript Type Definitions
```typescript
// widget-runtime.d.ts (subset of @modelcontextprotocol/ext-apps types)

export interface CallToolParams {
  name: string;
  arguments?: Record<string, unknown>;
}

export interface CallToolResult {
  content?: Array<{ type: string; text?: string; mimeType?: string }>;
  structuredContent?: unknown;
  isError?: boolean;
}

export interface HostContext {
  theme?: 'light' | 'dark';
  locale?: string;
  timezone?: string;
  displayMode?: 'inline' | 'pip' | 'fullscreen';
  containerSize?: { width: number; height: number };
}

export class App {
  constructor(info: { name: string; version: string });

  connect(): Promise<void>;

  callServerTool(params: CallToolParams): Promise<CallToolResult>;
  sendMessage(params: { role: string; content: unknown[] }): Promise<{ isError?: boolean }>;
  openLink(params: { url: string }): Promise<{ isError?: boolean }>;
  sendLog(params: { level: string; data: unknown }): Promise<void>;

  getHostContext(): HostContext | undefined;

  set ontoolinput(cb: (params: { arguments?: Record<string, unknown> }) => void);
  set ontoolresult(cb: (result: CallToolResult) => void);
  set ontoolcancelled(cb: (params: { reason?: string }) => void);
  set onhostcontextchanged(cb: (ctx: HostContext) => void);
  set onteardown(cb: () => {} | Promise<{}>);
}
```

### Example 4: `cargo pmcp app` Scaffold Output
```
my-chess-app/
  widgets/
    board.html            # Widget HTML with <script type="module"> import
  src/
    main.rs               # MCP server with chess tools + ui:// resource
  preview.html            # Standalone preview page (optional)
  Cargo.toml
  README.md               # Bridge API docs, stateless pattern, CSP config
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `window.mcpBridge.callTool()` custom API | `App.callServerTool()` via postMessage JSON-RPC | MCP Apps spec 2026-01-26 | Widget code portable across all MCP hosts |
| Inline bridge JS in `srcdoc` | External `<script type="module" src="...">` | This phase | Eliminates duplication, enables caching |
| Three separate bridge implementations | One shared TypeScript library | This phase | Single source of truth for types and behavior |
| ChatGPT-specific `window.openai` API | MCP Apps protocol (`ui/initialize`, `tools/call`) | MCP Apps spec 2026-01-26 | Host-agnostic; works in Claude, ChatGPT, VS Code, Goose |

**Deprecated/outdated:**
- `window.mcpBridge` custom API: Being replaced by MCP Apps protocol. Provide backward-compat shim during transition.
- `widget-runtime.js` IIFE format: Replaced by ES module format per CONTEXT.md decision.
- `window.openai` compatibility alias: Still needed for ChatGPT Apps backward compatibility, but the primary API should be the `App` class.

## Open Questions

1. **iframe.src vs iframe.srcdoc for widget loading**
   - What we know: `srcdoc` blocks ES module imports from external URLs due to `null` origin. The current preview uses `srcdoc`.
   - What's unclear: Whether switching to `iframe.src` with a server-rendered page breaks any existing behavior (e.g., the DevTools panel's console capture).
   - Recommendation: Switch to `iframe.src` pointing to a preview server route (e.g., `/widget?uri=...`) that injects the `<script>` tag and serves the widget HTML. This is the cleanest approach and matches how production hosts work. If `srcdoc` must be kept for this phase, inline the bridge as a `<script type="module">` with a `data:` URL import map or embed the full module source inline.

2. **Migrating chess/map examples in this phase or deferring**
   - What we know: Both examples currently use `window.mcpBridge.callTool()`. The CONTEXT.md marks this as Claude's discretion.
   - What's unclear: Whether partial migration creates confusion or whether deferring to Phase 19 (Ship) is cleaner.
   - Recommendation: Include a `window.mcpBridge` backward-compatibility shim in the bridge library so existing widgets keep working without changes. Defer full migration to Phase 19 when all Ship requirements are addressed. This keeps Phase 16 scope tight.

3. **Build orchestration: TS build before Rust build**
   - What we know: `rust_embed` embeds the `assets/` folder at compile time. If we want the compiled JS in the embedded assets, we need a pre-build step.
   - What's unclear: Whether to use a `build.rs` in `mcp-preview` or a `Makefile`/`justfile` target.
   - Recommendation: Use a `justfile` target (`just build-widget-runtime`) that runs `npm run build` in `packages/widget-runtime/` and copies `dist/index.mjs` to `crates/mcp-preview/assets/widget-runtime.js`. Add this as a dependency of the `just build` target. This aligns with the user's `justfile` preference from `~/.claude/CLAUDE.md`.

4. **Two entry points: vanilla vs React**
   - What we know: The current `hooks.ts` imports from `react` which makes the entire bundle require React.
   - What's unclear: Whether to split into two entry points now or later.
   - Recommendation: Split now. `tsup src/index.ts src/react.ts --format esm --dts`. The main `widget-runtime.js` has zero dependencies. React hooks available via `widget-runtime/react.js`.

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `packages/widget-runtime/src/*.ts` -- existing TypeScript source with types, runtime, hooks, utils
- Codebase inspection: `crates/mcp-preview/assets/widget-runtime.js` -- existing WASM bridge IIFE (Phase 15 output)
- Codebase inspection: `crates/mcp-preview/assets/index.html` -- preview server with inline proxy/WASM bridge injection
- GitHub: `modelcontextprotocol/ext-apps/src/app.ts` -- official App class source code (v1.1.2)
- GitHub: `modelcontextprotocol/ext-apps/examples/basic-server-vanillajs/src/mcp-app.ts` -- official vanilla JS usage example

### Secondary (MEDIUM confidence)
- [MCP Apps specification](https://modelcontextprotocol.io/docs/extensions/apps) -- protocol overview and architecture
- [MCP Apps blog post](http://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/) -- supported clients and ecosystem status
- [ext-apps GitHub README](https://github.com/modelcontextprotocol/ext-apps) -- SDK packages, examples, agent skills

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- tsup, TypeScript, rust_embed all already in use in the codebase; no new dependencies needed
- Architecture: HIGH -- official MCP Apps protocol is well-documented with source code available; the migration path from `window.mcpBridge` to `App` class is clear
- Pitfalls: HIGH -- identified from direct codebase analysis of current bridge injection approach and known issues in STATE.md

**Research date:** 2026-02-25
**Valid until:** 2026-03-25 (MCP Apps spec is stable at v1.1.2; no breaking changes expected in 30 days)
