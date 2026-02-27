# Phase 17: Widget Authoring DX and Scaffolding - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Developer can scaffold a new MCP Apps project from the command line (`cargo pmcp app new <name>`) and author widgets as standalone HTML files in a `widgets/` directory with automatic bridge injection, file-based discovery, and live reload on browser refresh. Documented patterns and CSP helpers are included.

</domain>

<decisions>
## Implementation Decisions

### Scaffolding Template
- Minimal project layout: `src/main.rs`, `widgets/hello.html`, `Cargo.toml`, `README.md`
- Starter widget demonstrates a "Hello World" with `callTool` — teaches the bridge pattern with a working example
- `main.rs` uses the builder pattern (`ServerBuilder::new().tool_typed_sync(...)`) matching existing chess/map examples
- `Cargo.toml` pins pmcp to published crates.io version (`pmcp = "1.10"`) — works for end users, not a workspace path dependency
- README explains bridge API, stateless widget pattern, and CSP configuration
- `main.rs` includes commented `WidgetCSP` helper examples

### Widget File Conventions
- Server scans `widgets/` directory at startup — each `.html` file becomes a UI resource automatically
- Filename maps directly to MCP resource URI: `widgets/board.html` → `ui://app/board`
- Widgets are single self-contained HTML files (CSS/JS inline) — matches MCP Apps spec
- Server auto-injects bridge script tag into widget HTML before serving — zero boilerplate for widget authors

### Hot Reload Behavior
- Server reads widget files from disk on each HTTP request — browser refresh shows latest HTML, no file watcher needed
- When a widget file has a syntax error or is missing, show a styled error message inline in the iframe with filename and error details
- Verbose logging in dev mode: log each widget file read with path and size to help developers confirm changes are picked up

### CLI Subcommand Design
- Invocation: `cargo pmcp app new <name>` — subcommand under `app` namespace, leaves room for future `app build`, `app test`
- One-shot generation, no interactive prompts — flags for any customization
- After scaffolding: print next steps (`cd my-app`, `cargo pmcp preview`, `open http://localhost:8765`) like `cargo init`
- Creates new directory (`./my-app/`) — error if directory already exists, matching `cargo new` semantics

### Claude's Discretion
- Dev mode vs production mode distinction (whether to add a flag for embedded assets vs disk reads)
- Exact error styling for inline widget errors
- Logging format and verbosity flag design

</decisions>

<specifics>
## Specific Ideas

- CLI should feel like `cargo new` / `cargo init` — familiar to Rust developers
- Post-scaffold message should include the exact commands to get to a running preview
- Widget auto-discovery means adding a new widget is just dropping a `.html` file — no registration step

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 17-widget-authoring-dx-and-scaffolding*
*Context gathered: 2026-02-26*
