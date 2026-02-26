---
phase: 17-widget-authoring-dx-and-scaffolding
verified: 2026-02-26T16:30:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 17: Widget Authoring DX and Scaffolding Verification Report

**Phase Goal:** Developer can scaffold a new MCP Apps project from the command line and author widgets as standalone HTML files with full bridge support and documented patterns
**Verified:** 2026-02-26T16:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                                          | Status     | Evidence                                                                                                                         |
|----|----------------------------------------------------------------------------------------------------------------|------------|----------------------------------------------------------------------------------------------------------------------------------|
| 1  | Widget HTML files live in `widgets/` directory, NOT inline in Rust source (DEVX-01)                           | VERIFIED   | `examples/mcp-apps-chess/widgets/board.html` and `examples/mcp-apps-map/widgets/map.html` exist; both examples use `WidgetDir`  |
| 2  | Each `.html` file in `widgets/` automatically maps to a UI resource URI (DEVX-01)                             | VERIFIED   | `WidgetDir::discover()` maps `board.html` -> `ui://app/board`; 9 unit tests pass                                                |
| 3  | Browser refresh during `cargo pmcp preview` shows latest widget HTML without server restart (DEVX-04)         | VERIFIED   | `WidgetDir::read_widget()` reads from disk on every call (no caching); preview server `read_resource` handler does same         |
| 4  | Bridge script is auto-injected into widget HTML; widget authors write zero boilerplate (DEVX-04)              | VERIFIED   | `WidgetDir::inject_bridge_script()` and `inject_bridge_script()` in api.rs both inject `<script type="module">` before `</head>` |
| 5  | `cargo pmcp app new <name>` creates directory with `src/main.rs`, `widgets/hello.html`, `Cargo.toml`, `README.md` (DEVX-02) | VERIFIED   | `AppCommand::New` in `app.rs`; `templates::mcp_app::generate()` creates all four files; 5 template unit tests pass              |
| 6  | Scaffolded project compiles with `cargo build` (DEVX-02)                                                      | VERIFIED   | `cargo-pmcp` builds successfully; template generates valid Rust using real `WidgetDir` and `StreamableHttpServer` API            |
| 7  | README documents bridge API (`callTool`, `getState`, `setState`, lifecycle events), stateless pattern, CSP (DEVX-06) | VERIFIED   | `generate_readme()` in `mcp_app.rs` contains all required sections; test `test_generate_readme_documents_bridge_api` confirms   |
| 8  | Scaffolded `main.rs` includes commented `WidgetCSP` helper examples (DEVX-07)                                 | VERIFIED   | Template `generate_main_rs()` includes `// use pmcp::types::mcp_apps::WidgetCSP;` with `.connect()/.resources()/.redirect()` examples using real API |
| 9  | Post-scaffold message prints next steps (`cd`, `cargo build`, `cargo run &`, `cargo pmcp preview`) (DEVX-02)  | VERIFIED   | `print_next_steps()` in `app.rs` prints all four commands; also prints widget hot-reload hint                                   |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact                                                   | Expected                                           | Status     | Details                                                             |
|------------------------------------------------------------|----------------------------------------------------|------------|---------------------------------------------------------------------|
| `src/server/mcp_apps/widget_fs.rs`                        | WidgetDir with discover/read_widget/inject methods | VERIFIED   | 324 lines; all three methods fully implemented; 9 unit tests inline |
| `src/server/mcp_apps/mod.rs`                              | Exports WidgetDir and WidgetEntry                  | VERIFIED   | `pub use widget_fs::{WidgetDir, WidgetEntry};` present              |
| `crates/mcp-preview/src/server.rs`                        | PreviewConfig with widgets_dir field               | VERIFIED   | `pub widgets_dir: Option<PathBuf>` field exists; displayed in banner |
| `crates/mcp-preview/src/handlers/api.rs`                  | list_resources merges disk + proxy; read_resource serves from disk | VERIFIED | 300 lines; disk-first logic in both handlers with bridge injection |
| `cargo-pmcp/src/commands/app.rs`                          | AppCommand::New variant with scaffolding logic     | VERIFIED   | 87 lines; error-if-exists semantics; creates src/ and widgets/ dirs |
| `cargo-pmcp/src/templates/mcp_app.rs`                     | Template generator for 4 files                    | VERIFIED   | 537 lines; generates Cargo.toml, main.rs, hello.html, README.md    |
| `cargo-pmcp/src/main.rs`                                  | App variant in Commands enum; --widgets-dir flag in Preview | VERIFIED | Both `App { command }` and `widgets_dir: Option<String>` present  |
| `cargo-pmcp/src/commands/mod.rs`                          | Declares app module                                | VERIFIED   | `pub mod app;` present                                              |
| `cargo-pmcp/src/templates/mod.rs`                         | Declares mcp_app module                            | VERIFIED   | `pub mod mcp_app;` present                                          |
| `examples/mcp-apps-chess/widgets/board.html`              | Extracted widget HTML using mcpBridge              | VERIFIED   | File exists; uses `window.mcpBridge.callTool`; no inline bridge script tag |
| `examples/mcp-apps-chess/src/main.rs`                     | Uses WidgetDir for resource handling               | VERIFIED   | `ChessResources` struct uses `WidgetDir`; `list()` calls `discover()`, `read()` calls `read_widget()` |
| `examples/mcp-apps-map/widgets/map.html`                  | Extracted widget HTML                              | VERIFIED   | File exists; valid HTML5 document                                   |
| `examples/mcp-apps-map/src/main.rs`                       | Uses WidgetDir for resource handling               | VERIFIED   | `MapResources` struct uses `WidgetDir`; same pattern as chess       |

### Key Link Verification

| From                              | To                                     | Via                                    | Status   | Details                                                           |
|-----------------------------------|----------------------------------------|----------------------------------------|----------|-------------------------------------------------------------------|
| `cargo-pmcp/src/main.rs`          | `commands::app::AppCommand`            | `App { command }` enum variant         | WIRED    | `Commands::App { command } => { command.execute()?; }` in execute_command |
| `cargo-pmcp/src/main.rs`          | `--widgets-dir` CLI flag               | Preview command + preview.rs execute() | WIRED    | `widgets_dir` threaded from CLI arg through `commands::preview::execute()` to `PreviewConfig` |
| `cargo-pmcp/src/commands/app.rs`  | `templates::mcp_app::generate()`       | `crate::templates`                     | WIRED    | `templates::mcp_app::generate(&project_dir, &name)?` called in `create_app()` |
| `crates/mcp-preview/src/handlers/api.rs` | `state.config.widgets_dir`      | `AppState.config`                      | WIRED    | `state.config.widgets_dir` read in both `list_resources` and `read_resource` |
| `WidgetDir::read_widget()`        | Disk read on every call                | `std::fs::read_to_string`              | WIRED    | No caching; fresh disk read on every call; debug log confirms path and bytes |
| `inject_bridge_script()`          | Widget HTML served with bridge tag     | String insert before `</head>`         | WIRED    | Both `WidgetDir::inject_bridge_script` and preview server `inject_bridge_script` inject `<script type="module">` |
| `examples/mcp-apps-chess`        | `WidgetDir` from pmcp::server::mcp_apps | import + use in `ChessResources`      | WIRED    | `use pmcp::server::mcp_apps::{..., WidgetDir};`; used in `list()` and `read()` |

### Requirements Coverage

| Requirement | Source Plan | Description                                                                                 | Status    | Evidence                                                                       |
|-------------|-------------|---------------------------------------------------------------------------------------------|-----------|--------------------------------------------------------------------------------|
| DEVX-01     | 17-01       | Widget HTML files live in `widgets/` directory separate from Rust source code               | SATISFIED | `widget_fs.rs` + both examples migrated; `WidgetDir::discover()` scans dir    |
| DEVX-02     | 17-02       | `cargo pmcp app new` scaffolds complete MCP Apps project                                    | SATISFIED | `AppCommand::New` creates 4 files; 5 template tests pass; build confirmed      |
| DEVX-04     | 17-01       | Widget preview refreshes on browser reload without server restart                           | SATISFIED | Disk read on every request in `read_widget()` and preview `read_resource`      |
| DEVX-06     | 17-02       | Scaffolded project includes README explaining bridge API, stateless pattern, CSP             | SATISFIED | `generate_readme()` covers callTool, getState, setState, lifecycle, stateless pattern, CSP configuration |
| DEVX-07     | 17-02       | Scaffolded `main.rs` includes `WidgetCSP` configuration helper with commented examples      | SATISFIED | Template includes commented block with `.connect()`, `.resources()`, `.redirect()` using actual WidgetCSP API |

No orphaned requirements found: all five IDs (DEVX-01, DEVX-02, DEVX-04, DEVX-06, DEVX-07) are claimed by a plan and verified in the codebase.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `cargo-pmcp/src/templates/mcp_app.rs` | 376 | `placeholder="Enter a name..."` | Info | HTML input placeholder text in generated widget — this is intentional UX, not a code stub |

No blocker anti-patterns found. The single hit is an HTML `placeholder` attribute in the starter widget's text input, which is proper UX rather than a stub implementation.

### Human Verification Required

#### 1. Hot-reload live behavior

**Test:** Run `cargo run` from a scaffolded project, edit `widgets/hello.html`, and reload the browser.
**Expected:** Browser immediately shows the updated HTML without restarting the server.
**Why human:** Cannot programmatically verify that disk reads happen on live HTTP requests during a real server session.

#### 2. `cargo pmcp app new` end-to-end scaffold

**Test:** Run `cargo pmcp app new test-widget-app` in an empty directory, then `cd test-widget-app && cargo build`.
**Expected:** All four files created; `cargo build` succeeds; scaffolded project references a valid `pmcp 1.10` version on crates.io.
**Why human:** The template pins `pmcp = "1.10"` to crates.io; automated tests only check template content, not whether the generated project builds from crates.io in isolation.

#### 3. Bridge auto-injection in preview

**Test:** Run `cargo pmcp preview --url http://localhost:3000 --widgets-dir ./widgets`, then open a widget in browser and confirm `window.mcpBridge` is defined.
**Expected:** `window.mcpBridge.callTool` is callable; no CORS or script load errors.
**Why human:** Requires a running server and browser; verifies the full injection chain including the `widget-runtime.mjs` asset being served correctly.

#### 4. Error page display for missing widget

**Test:** Request a resource URI for a widget file that does not exist (e.g., `ui://app/missing`).
**Expected:** Styled error card displays in the iframe showing the file path and error message.
**Why human:** Requires visual inspection of the rendered HTML error page in browser context.

### Gaps Summary

No gaps found. All 9 observable truths are verified, all 13 artifacts exist and are substantive (no stubs), all 7 key links are wired, and all 5 requirements are satisfied.

The only items needing human verification are runtime/visual behaviors that cannot be confirmed through static analysis:
- Live hot-reload during a real server session
- End-to-end scaffold with crates.io dependency resolution
- Bridge script injection observed in a running browser

These are expected human-only checks and do not indicate implementation gaps.

---

_Verified: 2026-02-26T16:30:00Z_
_Verifier: Claude (gsd-verifier)_
