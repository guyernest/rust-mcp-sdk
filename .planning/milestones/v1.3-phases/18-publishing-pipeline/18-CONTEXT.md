# Phase 18: Publishing Pipeline - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Developer can generate deployment artifacts for ChatGPT App Directory submission and shareable demo pages from their MCP Apps project. Two CLI commands produce a ChatGPT-compatible manifest JSON and a standalone landing page HTML. A combined `app build` command generates both.

</domain>

<decisions>
## Implementation Decisions

### Manifest format
- Follow ChatGPT ai-plugin.json format — name, description, auth, API endpoint, logo URL fields
- Server URL specified via required `--url` CLI flag (e.g., `cargo pmcp app manifest --url https://my-app.example.com`)
- Tool-to-widget mappings auto-discovered from `widgets/` directory — each `.html` file becomes a widget entry mapped to a tool of the same name (convention over config)
- App name from `[package].name`, description from `[package].description` in Cargo.toml; logo via optional `--logo` flag or `[package.metadata.pmcp].logo`

### Landing page behavior
- Widget rendered in a framed showcase — centered widget iframe with app name, description header, and subtle frame (product demo page feel)
- Mock bridge uses hardcoded sample responses generated at build time from `mock-data/` directory
- Developer creates `mock-data/tool-name.json` files with sample responses; landing generator reads and embeds them
- Output is a single self-contained HTML file — CSS, JS, widget HTML, mock data all inlined. One file to share, email, or host anywhere.

### CLI flag design
- Commands live under `cargo pmcp app` namespace: `app manifest`, `app landing`, `app build`
- `app build` is combined command generating both manifest + landing in one invocation; individual commands still available separately
- Default output to `dist/` directory (`dist/manifest.json`, `dist/landing.html`) — like a build output
- Overwrite existing output files silently — generated artifacts are disposable build output, matching cargo build semantics

### Project detection
- Detect MCP Apps project by checking Cargo.toml for pmcp dependency with mcp-apps feature; error with "Not an MCP Apps project. Run `cargo pmcp app new` first." if missing
- Widget discovery always looks in `./widgets/` relative to Cargo.toml — hardcoded path matching Phase 17 convention
- Empty or missing `widgets/` directory produces error: "No widgets found in widgets/. Add .html files or run `cargo pmcp app new` to scaffold a project."
- `mock-data/` directory required for landing page generation — error if missing: "No mock data found. Create mock-data/tool-name.json for each tool."

### Claude's Discretion
- Exact ChatGPT manifest schema version and field validation
- Landing page CSS styling and responsive behavior
- Mock bridge JavaScript implementation details
- How widget HTML is inlined into the landing page (iframe vs direct embed)
- dist/ directory creation behavior (auto-create if missing)

</decisions>

<specifics>
## Specific Ideas

- `cargo pmcp app build` as the "ship it" command — one command to get everything ready for distribution
- Landing page should feel like a product showcase you'd send to a colleague: "here's what this MCP app does"
- Mock data approach means landing pages always show realistic content, never empty/broken states
- Single-file landing page is the key UX win — drag and drop to any static host, attach to an email, open from filesystem

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 18-publishing-pipeline*
*Context gathered: 2026-02-26*
