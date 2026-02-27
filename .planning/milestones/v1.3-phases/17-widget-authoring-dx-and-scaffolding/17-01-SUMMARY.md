---
phase: 17-widget-authoring-dx-and-scaffolding
plan: 01
subsystem: ui
tags: [widget, hot-reload, filesystem, preview-server, dx]

# Dependency graph
requires:
  - phase: 16-shared-bridge-library
    provides: widget-runtime.js bridge script, preview server infrastructure
provides:
  - WidgetDir filesystem discovery module for scanning widgets/ directories
  - Hot-reload widget serving (disk read on every request, no caching)
  - Bridge script auto-injection into widget HTML
  - Preview server widgets_dir configuration for file-based authoring
  - --widgets-dir CLI flag for cargo pmcp preview
affects: [17-02-scaffolding, widget-authoring, preview-server]

# Tech tracking
tech-stack:
  added: []
  patterns: [file-based-widget-authoring, hot-reload-via-disk-read, bridge-auto-injection]

key-files:
  created:
    - src/server/mcp_apps/widget_fs.rs
    - examples/mcp-apps-chess/widgets/board.html
    - examples/mcp-apps-map/widgets/map.html
  modified:
    - src/server/mcp_apps/mod.rs
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/handlers/api.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/preview.rs
    - examples/mcp-apps-chess/src/main.rs
    - examples/mcp-apps-map/src/main.rs

key-decisions:
  - "WidgetDir reads from disk on every call (no caching) for zero-config hot-reload"
  - "Bridge script injected as type=module before </head> or after <body> open tag"
  - "Preview server implements its own inject_bridge_script (mirrors WidgetDir) since mcp-preview crate does not depend on pmcp"
  - "Widget URI convention: widgets/board.html maps to ui://app/board"
  - "Examples use CARGO_MANIFEST_DIR to resolve widgets/ path at compile time"

patterns-established:
  - "File-based widget authoring: HTML in widgets/ directory, auto-discovered as MCP resources"
  - "Hot-reload pattern: read from disk every request, no file watchers needed"
  - "Bridge auto-injection: server inserts script tag so widget authors write zero boilerplate"

requirements-completed: [DEVX-01, DEVX-04]

# Metrics
duration: 8min
completed: 2026-02-26
---

# Phase 17 Plan 01: File-Based Widget System and Hot Reload Summary

**WidgetDir filesystem discovery with hot-reload disk reads, bridge auto-injection, and preview server integration for zero-boilerplate widget authoring**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-26T16:06:40Z
- **Completed:** 2026-02-26T16:15:19Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- WidgetDir module scans widgets/ directories, maps .html files to ui://app/{name} URIs, reads fresh HTML on every call for hot-reload
- Preview server serves file-based widgets with automatic bridge script injection; --widgets-dir CLI flag threads through to PreviewConfig
- Chess and map examples migrated from inline include_str! to WidgetDir-based resource handling with widgets/ directories

## Task Commits

Each task was committed atomically:

1. **Task A: Widget filesystem discovery module** - `70ddcd8` (feat)
2. **Task B: Preview server hot-reload integration and example migration** - `75a6aa0` (feat)

## Files Created/Modified
- `src/server/mcp_apps/widget_fs.rs` - WidgetDir struct with discover(), read_widget(), inject_bridge_script() methods and 9 unit tests
- `src/server/mcp_apps/mod.rs` - Export WidgetDir and WidgetEntry from mcp_apps module
- `crates/mcp-preview/src/server.rs` - Added widgets_dir to PreviewConfig, display in server banner
- `crates/mcp-preview/src/handlers/api.rs` - list_resources merges disk widgets with proxy resources; read_resource reads from disk for ui://app/* URIs
- `cargo-pmcp/src/main.rs` - Added --widgets-dir CLI flag to Preview command
- `cargo-pmcp/src/commands/preview.rs` - Thread widgets_dir through to PreviewConfig
- `examples/mcp-apps-chess/widgets/board.html` - Extracted chess widget HTML to standalone file
- `examples/mcp-apps-chess/src/main.rs` - Migrated to WidgetDir for resource handling
- `examples/mcp-apps-map/widgets/map.html` - Extracted map widget HTML to standalone file
- `examples/mcp-apps-map/src/main.rs` - Migrated to WidgetDir for resource handling

## Decisions Made
- WidgetDir reads from disk on every call with no caching -- simplest hot-reload approach, no file watchers needed
- Bridge script injected as `<script type="module">` before `</head>` (or after `<body>` if no head)
- Preview server implements its own inject_bridge_script matching WidgetDir logic since mcp-preview does not depend on pmcp crate
- Widget URI convention: `widgets/board.html` maps to `ui://app/board`
- Examples use `env!("CARGO_MANIFEST_DIR")` to resolve widgets/ directory path at compile time

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WidgetDir module ready for use by scaffolding templates in Plan 17-02
- Preview server hot-reload infrastructure ready for widget authoring workflow
- Both example apps demonstrate the file-based widget pattern

## Self-Check: PASSED

- All created files verified on disk
- Both task commits (70ddcd8, 75a6aa0) verified in git log
- Build verified: mcp-preview, mcp-apps-chess, mcp-apps-map all compile
- Clippy: zero warnings across all modified crates
- Tests: 9/9 widget_fs unit tests pass

---
*Phase: 17-widget-authoring-dx-and-scaffolding*
*Completed: 2026-02-26*
