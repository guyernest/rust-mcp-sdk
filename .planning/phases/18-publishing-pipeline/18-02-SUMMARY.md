---
phase: 18-publishing-pipeline
plan: 02
subsystem: cli
tags: [html-generation, mock-bridge, iframe-srcdoc, landing-page, scaffold]

# Dependency graph
requires:
  - phase: 18-publishing-pipeline/18-01
    provides: detect_project, generate_manifest, write_manifest, AppCommand enum with New and Manifest
provides:
  - Landing page HTML generation with mock bridge (publishing::landing)
  - `cargo pmcp app landing` command for standalone demo pages
  - `cargo pmcp app build --url <URL>` command for combined manifest + landing output
  - Mock data scaffold (mock-data/hello.json) in `cargo pmcp app new`
affects: [19-widget-authoring-dx]

# Tech tracking
tech-stack:
  added: []
  patterns: [iframe-srcdoc-embedding, mock-bridge-injection, escape-for-srcdoc]

key-files:
  created:
    - cargo-pmcp/src/publishing/landing.rs
  modified:
    - cargo-pmcp/src/publishing/mod.rs
    - cargo-pmcp/src/commands/app.rs
    - cargo-pmcp/src/templates/mcp_app.rs

key-decisions:
  - "Mock bridge uses type=module script with window.mcpBridge matching live bridge API"
  - "srcdoc escaping only escapes & and \" (minimum for attribute context; < and > are valid in srcdoc HTML)"
  - "load_mock_data takes explicit Path parameter (matching detect_project pattern for testability)"
  - "Build command calls detect_project once and shares result for both manifest and landing generation"

patterns-established:
  - "iframe srcdoc embedding: escape HTML for attribute context, inject mock bridge before </head>"
  - "Mock data convention: mock-data/*.json keyed by tool name for offline widget demos"

requirements-completed: [PUBL-02]

# Metrics
duration: 4min
completed: 2026-02-26
---

# Phase 18 Plan 02: Landing Page & Build Command Summary

**Self-contained landing page generation with mock bridge injection and combined build command for manifest + landing output**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-26T17:48:30Z
- **Completed:** 2026-02-26T17:52:56Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Landing page generator produces single self-contained HTML with widget in iframe via srcdoc, mock bridge returning hardcoded JSON, and product-showcase styling
- `cargo pmcp app landing` command wired into CLI for standalone demo page generation
- `cargo pmcp app build --url <URL>` command produces both manifest.json and landing.html in one invocation
- Scaffold template updated to include mock-data/hello.json for immediate landing page support
- 16 new tests added (13 in landing.rs, 3 in mcp_app.rs)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement landing page generation with mock bridge** - `3f6b9a0` (feat)
2. **Task 2: Wire landing and build commands into AppCommand CLI and update scaffold template** - `30446e1` (feat)

## Files Created/Modified
- `cargo-pmcp/src/publishing/landing.rs` - Landing page HTML generation with mock bridge, srcdoc escaping, and product-showcase styling
- `cargo-pmcp/src/publishing/mod.rs` - Added landing module declaration
- `cargo-pmcp/src/commands/app.rs` - Added Landing and Build command variants with create_landing and build_all handlers
- `cargo-pmcp/src/templates/mcp_app.rs` - Added mock-data/hello.json to scaffold template

## Decisions Made
- Mock bridge uses `type=module` script matching the live bridge API surface (callTool, getState, setState, theme, locale, displayMode)
- srcdoc escaping only replaces `&` and `"` -- `<` and `>` are valid inside srcdoc and the browser parses them as HTML
- load_mock_data takes explicit `&Path` parameter following the detect_project pattern established in 18-01 for testability
- Build command shares a single detect_project call for both manifest and landing generation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Applied cargo fmt formatting corrections**
- **Found during:** Task 2 (after build verification)
- **Issue:** Some lines exceeded rustfmt width preferences
- **Fix:** Ran `cargo fmt -p cargo-pmcp` to auto-format
- **Files modified:** cargo-pmcp/src/publishing/landing.rs, cargo-pmcp/src/commands/app.rs
- **Verification:** `cargo fmt -p cargo-pmcp --check` passes
- **Committed in:** 30446e1 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Formatting-only auto-fix, no scope change.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Publishing pipeline complete (both manifest generation and landing page)
- Ready for phase 19 (widget authoring DX) or milestone wrap-up
- All `cargo pmcp app` subcommands (new, manifest, landing, build) are functional

---
*Phase: 18-publishing-pipeline*
*Completed: 2026-02-26*
