---
phase: 17-widget-authoring-dx-and-scaffolding
plan: 02
subsystem: cli
tags: [scaffolding, cli, templates, widget-authoring, dx, cargo-pmcp]

# Dependency graph
requires:
  - phase: 17-widget-authoring-dx-and-scaffolding
    plan: 01
    provides: WidgetDir filesystem discovery, bridge auto-injection, hot-reload pattern
provides:
  - cargo pmcp app new <name> scaffolding command
  - MCP Apps project template (Cargo.toml, main.rs, hello.html, README.md)
  - Bridge API documentation in generated README
  - Commented WidgetCSP configuration examples in generated main.rs
affects: [widget-authoring-workflow, developer-onboarding]

# Tech tracking
tech-stack:
  added: []
  patterns: [cli-scaffolding, project-template-generation, post-scaffold-next-steps]

key-files:
  created:
    - cargo-pmcp/src/commands/app.rs
    - cargo-pmcp/src/templates/mcp_app.rs
  modified:
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/mod.rs
    - cargo-pmcp/src/templates/mod.rs

key-decisions:
  - "WidgetCSP commented examples use actual API (.connect/.resources/.redirect) not plan's suggested (.default_src/.script_src) which does not exist"
  - "Template main.rs imports WidgetCSP from pmcp::types::mcp_apps (correct path), not pmcp::server::mcp_apps"
  - "Used 'ok' prefix for status messages instead of Unicode checkmarks for terminal compatibility"

patterns-established:
  - "App subcommand namespace: cargo pmcp app {verb} leaves room for future app build, app test"
  - "One-shot scaffolding: no interactive prompts, error-if-exists matching cargo new semantics"

requirements-completed: [DEVX-02, DEVX-06, DEVX-07]

# Metrics
duration: 4min
completed: 2026-02-26
---

# Phase 17 Plan 02: CLI Scaffolding Command and Project Templates Summary

**`cargo pmcp app new` scaffolding command with WidgetDir-based server template, bridge-pattern starter widget, and documented bridge API/CSP/stateless patterns**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-26T16:18:23Z
- **Completed:** 2026-02-26T16:22:06Z
- **Tasks:** 1
- **Files modified:** 5

## Accomplishments
- `cargo pmcp app new <name>` creates a complete MCP Apps project with server code, starter widget, and documentation
- Generated main.rs uses WidgetDir for file-based widget discovery with ServerBuilder pattern matching chess/map examples
- Generated hello.html demonstrates mcpBridge.callTool pattern without inline bridge script (server auto-injects)
- Generated README documents bridge API (callTool, getState, setState, lifecycle events), stateless widget pattern, and CSP configuration with WidgetCSP examples

## Task Commits

Each task was committed atomically:

1. **Task A: App subcommand and scaffolding template** - `2f20ae3` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/app.rs` - App subcommand with New variant, directory-exists error handling, post-scaffold next steps
- `cargo-pmcp/src/templates/mcp_app.rs` - Template generator for Cargo.toml, main.rs, hello.html, README.md with 5 unit tests
- `cargo-pmcp/src/main.rs` - Added App variant to Commands enum and execute match arm
- `cargo-pmcp/src/commands/mod.rs` - Added app module declaration
- `cargo-pmcp/src/templates/mod.rs` - Added mcp_app module declaration

## Decisions Made
- Used actual WidgetCSP API methods (.connect, .resources, .redirect) in commented examples instead of plan's suggested .default_src/.script_src which do not exist in the codebase (Rule 1 - correcting inaccurate API reference)
- Template imports WidgetCSP from `pmcp::types::mcp_apps` (the correct module path) rather than `pmcp::server::mcp_apps` as the plan suggested
- Used "ok" prefix for status messages matching the existing crate's output conventions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected WidgetCSP API in commented examples**
- **Found during:** Task A (template generation)
- **Issue:** Plan specified `.default_src()`, `.script_src()`, `.style_src()`, `.connect_src()` methods which do not exist on WidgetCSP. Actual API uses `.connect()`, `.resources()`, `.redirect()`, `.frame()`.
- **Fix:** Used the real WidgetCSP API in both main.rs commented examples and README documentation
- **Files modified:** cargo-pmcp/src/templates/mcp_app.rs
- **Verification:** Examples match actual WidgetCSP implementation in src/types/mcp_apps.rs
- **Committed in:** 2f20ae3

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential correctness fix. Generated code references real API methods that users can uncomment and use.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 17 is now complete: file-based widget system (17-01) and CLI scaffolding (17-02) both shipped
- Developers can scaffold a new MCP Apps project and author widgets with zero boilerplate
- Preview workflow documented in generated README: `cargo run` then `cargo pmcp preview --url http://localhost:3000 --open`

## Self-Check: PASSED

- cargo-pmcp/src/commands/app.rs: FOUND
- cargo-pmcp/src/templates/mcp_app.rs: FOUND
- Task commit 2f20ae3: FOUND in git log
- Build: cargo build -p cargo-pmcp succeeds
- Tests: 48/48 pass (5 new mcp_app template tests)
- Clippy: zero warnings in new files
- Fmt: cargo fmt --check passes

---
*Phase: 17-widget-authoring-dx-and-scaffolding*
*Completed: 2026-02-26*
