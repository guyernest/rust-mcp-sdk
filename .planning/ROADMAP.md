# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- ✅ **v1.2 Pluggable Storage Backends** — Phases 9-13 (shipped 2026-02-24)
- **v1.3 MCP Apps Developer Experience** — Phases 14-19 (in progress)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.2 Pluggable Storage Backends (Phases 9-13) — SHIPPED 2026-02-24</summary>

- [x] Phase 9: Storage Abstraction Layer (2/2 plans) — completed 2026-02-24
- [x] Phase 10: InMemory Backend Refactor (2/2 plans) — completed 2026-02-24
- [x] Phase 11: DynamoDB Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 12: Redis Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 13: Feature Flag Verification (1/1 plans) — completed 2026-02-24

See: `.planning/milestones/v1.2-ROADMAP.md` for full phase details

</details>

### v1.3 MCP Apps Developer Experience (In Progress)

**Milestone Goal:** Polish MCP Apps into a production-ready developer experience -- from authoring through preview to publishing -- making it easy for developers to build, test, demo, and publish MCP Apps with rich UI widgets.

- [ ] **Phase 14: Preview Bridge Infrastructure** - Widget iframe rendering with working MCP bridge proxy in mcp-preview
- [ ] **Phase 15: WASM Widget Bridge** - In-browser WASM MCP client as alternative bridge mode for widget testing
- [ ] **Phase 16: Shared Bridge Library** - Canonical widget-runtime.js eliminating bridge code duplication across widgets
- [ ] **Phase 17: Widget Authoring DX and Scaffolding** - File-based widgets, scaffolding template, and developer ergonomics
- [ ] **Phase 18: Publishing Pipeline** - ChatGPT manifest generation and standalone demo landing pages
- [ ] **Phase 19: Ship Examples and Playwright E2E** - Chess and map examples finalized with passing Playwright test suites

## Phase Details

### Phase 14: Preview Bridge Infrastructure
**Goal**: Developer can run `cargo pmcp preview`, see their widget rendered in an iframe, and click UI elements that fire real MCP tool calls through the bridge proxy
**Depends on**: Nothing (first phase of v1.3; builds on existing mcp-preview crate)
**Requirements**: PREV-01, PREV-02, PREV-03, PREV-04, PREV-05, PREV-06, PREV-07
**Success Criteria** (what must be TRUE):
  1. Developer runs `cargo pmcp preview` and sees their widget HTML rendered inside an iframe in the preview UI
  2. Widget JavaScript calling `window.mcpBridge.callTool("tool_name", args)` receives a response from the real MCP server
  3. Refreshing the preview page or making multiple tool calls reuses the same MCP session (no re-initialization per request)
  4. DevTools panel in the preview UI shows a live log entry each time a bridge call is made, including tool name and response
  5. When the MCP server exposes multiple UI resources, a picker in the preview sidebar lets the developer switch between them
**Plans**: 2 plans

Plans:
- [ ] 14-01-PLAN.md — Backend: session-persistent MCP proxy with resource list/read methods and API routes
- [ ] 14-02-PLAN.md — Frontend: resource picker, auto-load widget, enhanced DevTools, connection status, reconnect

### Phase 15: WASM Widget Bridge
**Goal**: Developer can toggle to a WASM bridge mode in preview where an in-browser MCP client connects directly to the server, eliminating the proxy middleman
**Depends on**: Phase 14 (bridge protocol and preview UI must exist)
**Requirements**: WASM-01, WASM-02, WASM-03, WASM-04, WASM-05
**Success Criteria** (what must be TRUE):
  1. WASM MCP client loads in the preview iframe context and successfully connects to the local MCP server
  2. Widget code calling `window.mcpBridge.callTool()` works identically whether using the proxy bridge or the WASM bridge
  3. A standalone `widget-runtime.js` file bundles the WASM client and exposes it as a drop-in `window.mcpBridge` polyfill usable outside the preview context
**Plans**: TBD

Plans:
- [ ] 15-01: TBD
- [ ] 15-02: TBD

### Phase 16: Shared Bridge Library
**Goal**: A single canonical bridge library eliminates duplicated JavaScript across widgets and guarantees API consistency between preview, WASM, and production bridge modes
**Depends on**: Phase 14 and Phase 15 (both bridge implementations must exist to extract a proven shared contract)
**Requirements**: DEVX-03, DEVX-05
**Success Criteria** (what must be TRUE):
  1. Preview server serves `widget-runtime.js` at a stable URL and widgets reference it via a single `<script>` tag instead of inline bridge code
  2. TypeScript type definitions (`widget-runtime.d.ts`) ship alongside the bridge library, providing autocomplete for `callTool`, `getState`, `setState`, and lifecycle events
**Plans**: TBD

Plans:
- [ ] 16-01: TBD

### Phase 17: Widget Authoring DX and Scaffolding
**Goal**: Developer can scaffold a new MCP Apps project from the command line and author widgets as standalone HTML files with full bridge support and documented patterns
**Depends on**: Phase 16 (scaffolded templates reference widget-runtime.js)
**Requirements**: DEVX-01, DEVX-02, DEVX-04, DEVX-06, DEVX-07
**Success Criteria** (what must be TRUE):
  1. Running `cargo pmcp new --mcp-apps my-app` generates a compilable project with server code, a `widgets/` directory containing a starter widget, and a working preview configuration
  2. Widget HTML files live in a `widgets/` directory separate from Rust source and are loaded at runtime (not embedded as inline strings in Rust code)
  3. Reloading the browser while `cargo pmcp preview` is running shows the latest widget HTML without requiring a server restart
  4. Scaffolded project README explains the bridge API, stateless widget pattern, and CSP configuration; scaffolded `main.rs` includes commented `WidgetCSP` helper examples
**Plans**: TBD

Plans:
- [ ] 17-01: TBD
- [ ] 17-02: TBD

### Phase 18: Publishing Pipeline
**Goal**: Developer can generate deployment artifacts for ChatGPT App Directory submission and shareable demo pages from their MCP Apps project
**Depends on**: Phase 17 (manifest reads project structure established by scaffolding; landing page uses file-based widget layout)
**Requirements**: PUBL-01, PUBL-02
**Success Criteria** (what must be TRUE):
  1. Running `cargo pmcp manifest` produces a ChatGPT-compatible JSON file containing server URL, tool-to-widget mapping, and required metadata fields
  2. Running `cargo pmcp landing` generates a standalone HTML page that renders the widget with a mock bridge, viewable without a running MCP server
**Plans**: TBD

Plans:
- [ ] 18-01: TBD

### Phase 19: Ship Examples and Playwright E2E
**Goal**: Chess and map MCP Apps examples compile, run, and pass automated end-to-end Playwright tests proving the complete widget pipeline works
**Depends on**: Phase 14, 15, 16, 17, 18 (examples exercise the full toolchain; tests validate everything)
**Requirements**: SHIP-01, SHIP-02, SHIP-03, SHIP-04, SHIP-05
**Success Criteria** (what must be TRUE):
  1. `cargo build --features mcp-apps` compiles both chess and map example apps without errors
  2. Running each example serves widgets that render correctly in a browser at the expected paths
  3. Playwright test server serves widget files and all chess widget Playwright tests pass
  4. Map widget Playwright tests are written and passing
**Plans**: TBD

Plans:
- [ ] 19-01: TBD
- [ ] 19-02: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Handoff + Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction | v1.2 | 2/2 | Complete | 2026-02-24 |
| 10. InMemory Refactor | v1.2 | 2/2 | Complete | 2026-02-24 |
| 11. DynamoDB Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 12. Redis Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 13. Feature Flags | v1.2 | 1/1 | Complete | 2026-02-24 |
| 14. Preview Bridge | 1/2 | In Progress|  | - |
| 15. WASM Bridge | v1.3 | 0/? | Not started | - |
| 16. Shared Bridge Lib | v1.3 | 0/? | Not started | - |
| 17. Authoring DX | v1.3 | 0/? | Not started | - |
| 18. Publishing | v1.3 | 0/? | Not started | - |
| 19. Ship + E2E | v1.3 | 0/? | Not started | - |
